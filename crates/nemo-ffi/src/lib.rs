//! UniFFI surface for the Compose shell (ADR-0028). No keys cross this boundary
//! except identifiers, fingerprints, and the one-time revocation mnemonic.

uniffi::setup_scaffolding!("nemo");

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use nemo_core::{
    decode, encode, encode_text, AppBody, AppHeader, AppMessage, CoreError, HomeSession, HttpHome,
    Installation, Vault,
};
use nemo_wire::envelope::TtlBucket;
use nemo_wire::{ids, parse_identity_id, ContactCard};

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum FfiError {
    #[error("{0}")]
    Core(String),
}

impl From<nemo_core::CoreError> for FfiError {
    fn from(err: nemo_core::CoreError) -> Self {
        Self::Core(err.to_string())
    }
}

impl From<nemo_wire::WireError> for FfiError {
    fn from(err: nemo_wire::WireError) -> Self {
        Self::Core(err.to_string())
    }
}

/// Decrypted 1:1 text for the shell. Never includes ratchet or MLS keys.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct DisplayRow {
    pub conv_id: String,
    pub conv_seq: u64,
    pub text: String,
    pub sent_at: u64,
}

enum ClientState {
    Local(Installation),
    Registered(HomeSession<HttpHome>),
}

struct Inner {
    state: Option<ClientState>,
    vault: Option<Vault>,
    revocation_mnemonic: Option<String>,
    inbox: Vec<DisplayRow>,
    nicknames: HashMap<String, String>,
    next_seq: HashMap<String, u64>,
}

/// One installation. The shell must not persist ratchet or MLS keys.
#[derive(uniffi::Object)]
pub struct NemoClient {
    inner: Mutex<Inner>,
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
        Err(_) => runtime().block_on(fut),
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}

fn lock_err() -> FfiError {
    FfiError::Core("lock".into())
}

fn parse_card(card_or_uri: &str) -> Result<ContactCard, FfiError> {
    let uri = card_or_uri
        .split_once('#')
        .map(|(u, _)| u)
        .unwrap_or(card_or_uri);
    if uri.starts_with(nemo_wire::card::URI_PREFIX) {
        return Ok(ContactCard::from_uri(uri)?);
    }
    if let Ok(card) = ContactCard::from_uri(uri) {
        return Ok(card);
    }
    let bytes = ids::from_hex(uri)?;
    Ok(ContactCard::decode(&bytes)?)
}

fn parse_delivery_cap(card_or_uri: &str, card: &ContactCard) -> Result<[u8; 32], FfiError> {
    match card_or_uri.split_once('#') {
        Some((_, hex)) if !hex.is_empty() => Ok(ids::copy_fixed(&ids::from_hex(hex)?)?),
        _ => Ok(card.share_token),
    }
}

fn bump_seq(map: &mut HashMap<String, u64>, peer: &str) -> u64 {
    let e = map.entry(peer.to_owned()).or_insert(0);
    *e += 1;
    *e
}

fn persist(inner: &Inner) -> Result<(), FfiError> {
    let Some(vault) = inner.vault.as_ref() else {
        return Ok(());
    };
    match inner.state.as_ref() {
        Some(ClientState::Local(install)) => vault.save(install)?,
        Some(ClientState::Registered(session)) => {
            vault.save_home(&session.install, &session.snapshot())?
        }
        None => return Err(FfiError::Core("busy".into())),
    }
    Ok(())
}

impl Inner {
    fn install(&self) -> Result<&Installation, FfiError> {
        match self.state.as_ref() {
            Some(ClientState::Local(install)) => Ok(install),
            Some(ClientState::Registered(session)) => Ok(&session.install),
            None => Err(FfiError::Core("busy".into())),
        }
    }

    fn registered(&mut self) -> Result<&mut HomeSession<HttpHome>, FfiError> {
        match self.state.as_mut() {
            Some(ClientState::Registered(session)) => Ok(session),
            Some(ClientState::Local(_)) => Err(CoreError::NotRegistered.into()),
            None => Err(FfiError::Core("busy".into())),
        }
    }
}

#[uniffi::export]
impl NemoClient {
    #[uniffi::constructor]
    pub fn create() -> Result<Arc<Self>, FfiError> {
        let (install, export) = Installation::create()?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Inner {
                state: Some(ClientState::Local(install)),
                vault: None,
                revocation_mnemonic: Some(export.mnemonic),
                inbox: Vec::new(),
                nicknames: HashMap::new(),
                next_seq: HashMap::new(),
            }),
        }))
    }

    /// Create an identity and lock it in `dir` (ADR-0034).
    #[uniffi::constructor]
    pub fn create_at(dir: String, passphrase: String) -> Result<Arc<Self>, FfiError> {
        let (install, export) = Installation::create()?;
        let vault = Vault::create(&dir, &passphrase, &install)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Inner {
                state: Some(ClientState::Local(install)),
                vault: Some(vault),
                revocation_mnemonic: Some(export.mnemonic),
                inbox: Vec::new(),
                nicknames: HashMap::new(),
                next_seq: HashMap::new(),
            }),
        }))
    }

    #[uniffi::constructor]
    pub fn open_at(dir: String, passphrase: String) -> Result<Arc<Self>, FfiError> {
        let (vault, install) = Vault::open(&dir, &passphrase)?;
        let state = match vault.load_home()? {
            Some(home) if !home.home_base.is_empty() => {
                let transport = HttpHome::new(home.home_base.clone())?;
                ClientState::Registered(HomeSession::resume(transport, install, home))
            }
            _ => ClientState::Local(install),
        };
        Ok(Arc::new(Self {
            inner: Mutex::new(Inner {
                state: Some(state),
                vault: Some(vault),
                revocation_mnemonic: None,
                inbox: Vec::new(),
                nicknames: HashMap::new(),
                next_seq: HashMap::new(),
            }),
        }))
    }

    pub fn identity_id_hex(&self) -> Result<String, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(ids::to_hex(&inner.install()?.identity_id()))
    }

    pub fn fingerprint(&self) -> Result<String, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner.install()?.fingerprint())
    }

    /// Shown once at identity creation. Never stored in the vault.
    pub fn take_revocation_mnemonic(&self) -> Result<Option<String>, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner.revocation_mnemonic.take())
    }

    pub fn save(&self) -> Result<(), FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        if inner.vault.is_none() {
            return Err(CoreError::VaultDetached.into());
        }
        persist(&inner)
    }

    pub fn register(&self, home_https_base: String) -> Result<(), FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let install = match inner.state.take() {
            Some(ClientState::Local(install)) => install,
            Some(ClientState::Registered(session)) => {
                inner.state = Some(ClientState::Registered(session));
                return Err(CoreError::AlreadyRegistered.into());
            }
            None => return Err(FfiError::Core("busy".into())),
        };
        let transport = HttpHome::new(&home_https_base)?;
        match block_on(async {
            let (mut session, _) = HomeSession::register(transport, install, now_unix()).await?;
            session.set_home_base(&home_https_base);
            session.restock_publish().await?;
            Ok::<_, CoreError>(session)
        }) {
            Ok(session) => {
                inner.state = Some(ClientState::Registered(session));
                persist(&inner)?;
                Ok(())
            }
            Err(err) => {
                inner.state = None;
                Err(err.into())
            }
        }
    }

    pub fn mint_share_uri(&self) -> Result<String, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let session = inner.registered()?;
        let card = block_on(session.mint_share_card(now_unix()))?;
        let cap = block_on(session.mint_contact())?;
        persist(&inner)?;
        Ok(format!("{}#{}", card.to_uri()?, ids::to_hex(&cap)))
    }

    pub fn add_contact(&self, card_or_uri: String, nickname: String) -> Result<String, FfiError> {
        let card = parse_card(&card_or_uri)?;
        let cap = parse_delivery_cap(&card_or_uri, &card)?;
        let peer = ids::to_hex(&card.identity_id());
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let session = inner.registered()?;
        block_on(session.add_contact(&card, cap, now_unix()))?;
        if !nickname.is_empty() {
            inner.nicknames.insert(peer.clone(), nickname);
        }
        persist(&inner)?;
        Ok(peer)
    }

    pub fn send_text(&self, peer_id_hex: String, text: String) -> Result<DisplayRow, FfiError> {
        let peer = parse_identity_id(&peer_id_hex)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        let seq = bump_seq(&mut inner.next_seq, &peer_id_hex);
        let ptext = encode_text(seq, now, &text)?;
        {
            let session = inner.registered()?;
            block_on(session.send_to(&peer, TtlBucket::DEFAULT, &ptext, now))?;
        }
        let row = DisplayRow {
            conv_id: peer_id_hex,
            conv_seq: seq,
            text,
            sent_at: now,
        };
        inner.inbox.push(row.clone());
        persist(&inner)?;
        Ok(row)
    }

    pub fn fetch_now(&self) -> Result<Vec<DisplayRow>, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let opened = {
            let session = inner.registered()?;
            block_on(session.ingest_mailbox())?
        };
        let mut new_rows = Vec::new();
        let mut gossip = Vec::new();
        for (peer, plaintext) in opened {
            let conv_id = ids::to_hex(&peer);
            match decode(&plaintext) {
                Ok(AppMessage {
                    header,
                    body: AppBody::Text { text },
                }) => {
                    gossip.push(peer);
                    let seq = *inner.next_seq.get(&conv_id).unwrap_or(&0);
                    if header.conv_seq > seq {
                        inner.next_seq.insert(conv_id.clone(), header.conv_seq);
                    }
                    let row = DisplayRow {
                        conv_id,
                        conv_seq: header.conv_seq,
                        text,
                        sent_at: header.sent_at,
                    };
                    inner.inbox.push(row.clone());
                    new_rows.push(row);
                }
                Ok(AppMessage {
                    body: AppBody::Capability { contact_capability },
                    ..
                }) => {
                    if let Some(ClientState::Registered(session)) = &mut inner.state {
                        if let Some(contact) = session.contacts.get_mut(&peer) {
                            contact.delivery_capability = contact_capability;
                        }
                    }
                }
                _ => {}
            }
        }
        let now = now_unix();
        if let Some(ClientState::Registered(session)) = &mut inner.state {
            for peer in gossip {
                if !session.contacts.contains_key(&peer) {
                    continue;
                }
                if let Ok(cap) = block_on(session.mint_contact()) {
                    if let Ok(bytes) = encode(&AppMessage {
                        header: AppHeader {
                            conv_seq: 0,
                            sent_at: now,
                            reply_to: None,
                        },
                        body: AppBody::Capability {
                            contact_capability: cap,
                        },
                    }) {
                        let _ = block_on(session.send_to(&peer, TtlBucket::DEFAULT, &bytes, now));
                    }
                }
            }
        }
        persist(&inner)?;
        Ok(new_rows)
    }

    pub fn inbox(&self) -> Result<Vec<DisplayRow>, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner.inbox.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_server::{router, AppState};
    use rand::RngCore;
    use std::fs;

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let mut n = [0u8; 8];
        rand::rngs::OsRng.fill_bytes(&mut n);
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", ids::to_hex(&n)));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn serve_home() -> String {
        let listener = runtime()
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let addr = listener.local_addr().unwrap();
        runtime().spawn(async move {
            axum::serve(listener, router(AppState::new()))
                .await
                .expect("serve");
        });
        format!("http://{addr}")
    }

    #[test]
    fn create_exports_ids_and_one_time_mnemonic() {
        let client = NemoClient::create().unwrap();
        let id = client.identity_id_hex().unwrap();
        assert_eq!(id.len(), 64);
        assert_eq!(client.fingerprint().unwrap().len(), 64 + 7);
        let mnemonic = client.take_revocation_mnemonic().unwrap();
        assert!(mnemonic.is_some());
        assert!(client.take_revocation_mnemonic().unwrap().is_none());
    }

    #[test]
    fn vault_create_open_same_identity() {
        let dir = temp_dir("nemo-ffi-vault");
        let client =
            NemoClient::create_at(dir.to_string_lossy().into_owned(), "correct horse".into())
                .unwrap();
        let id = client.identity_id_hex().unwrap();
        let mnemonic = client.take_revocation_mnemonic().unwrap();
        assert!(mnemonic.is_some());
        drop(client);
        let opened =
            NemoClient::open_at(dir.to_string_lossy().into_owned(), "correct horse".into())
                .unwrap();
        assert_eq!(opened.identity_id_hex().unwrap(), id);
        assert!(opened.take_revocation_mnemonic().unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn register_share_send_fetch_against_in_process_server() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-alice");
        let bob_dir = temp_dir("nemo-ffi-bob");
        let alice = NemoClient::create_at(
            alice_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
        )
        .unwrap();
        let bob = NemoClient::create_at(
            bob_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
        )
        .unwrap();
        alice.register(base.clone()).unwrap();
        bob.register(base).unwrap();

        let uri = alice.mint_share_uri().unwrap();
        assert!(uri.starts_with("nemo:1:"));
        let alice_id = alice.identity_id_hex().unwrap();
        let added = bob.add_contact(uri, "Alice".into()).unwrap();
        assert_eq!(added, alice_id);

        let sent = bob
            .send_text(alice_id.clone(), "hello from bob".into())
            .unwrap();
        assert_eq!(sent.text, "hello from bob");
        assert_eq!(sent.conv_id, alice_id);

        let rows = alice.fetch_now().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "hello from bob");
        assert_eq!(rows[0].conv_seq, sent.conv_seq);
        assert_eq!(alice.inbox().unwrap().len(), 1);

        drop(alice);
        drop(bob);
        let alice2 = NemoClient::open_at(
            alice_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
        )
        .unwrap();
        assert_eq!(alice2.identity_id_hex().unwrap(), alice_id);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
    }

    #[test]
    fn ffi_records_are_display_only() {
        let row = DisplayRow {
            conv_id: "ab".repeat(32),
            conv_seq: 1,
            text: "hi".into(),
            sent_at: 1,
        };
        assert_eq!(row.conv_id.len(), 64);
        let _ = row.text;
    }
}
