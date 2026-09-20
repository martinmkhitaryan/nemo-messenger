//! UniFFI surface for the Compose shell (ADR-0028). No keys cross this boundary
//! except identifiers, fingerprints, and the one-time revocation mnemonic.

uniffi::setup_scaffolding!("nemo");

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use nemo_core::{
    decode, encode, encode_text, mailbox, AppBody, AppHeader, AppMessage, CoreError, Group,
    HomeSession, HostAccept, HostGroup, HttpHome, Installation, PendingJoin, Vault,
};
use nemo_wire::cbor::{self, Value};
use nemo_wire::envelope::{MessageType, TtlBucket};
use nemo_wire::ids::{self, KEY_LEN};
use nemo_wire::{parse_identity_id, ContactCard, GroupInvite};

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

/// Decrypted 1:1 or group text for the shell. Never includes ratchet or MLS keys.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct DisplayRow {
    pub conv_id: String,
    pub conv_seq: u64,
    pub text: String,
    pub sent_at: u64,
}

/// Hosted MLS group the shell can list. No signing keys.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct GroupRow {
    pub group_id: String,
    pub nickname: String,
    pub member_count: u64,
}

const GROUP_INVITE_PREFIX: &str = "nemo-g:1:";
const JOIN_REQUEST_PREFIX: &str = "nemo-j:1:";

struct LiveGroup {
    host: HostGroup,
    mls: Group,
    nickname: String,
}

struct PendingEntry {
    pending: PendingJoin,
    host: HostAccept,
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
    groups: Vec<LiveGroup>,
    pending: HashMap<[u8; KEY_LEN], PendingEntry>,
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
            vault.save_home(&session.install, &session.snapshot())?;
            vault.save_groups(&session.install, inner.groups.iter().map(|g| &g.mls))?;
        }
        None => return Err(FfiError::Core("busy".into())),
    }
    Ok(())
}

fn empty_inner(
    state: Option<ClientState>,
    vault: Option<Vault>,
    mnemonic: Option<String>,
) -> Inner {
    Inner {
        state,
        vault,
        revocation_mnemonic: mnemonic,
        inbox: Vec::new(),
        nicknames: HashMap::new(),
        next_seq: HashMap::new(),
        groups: Vec::new(),
        pending: HashMap::new(),
    }
}

fn load_live_groups(vault: &Vault, install: &Installation, hosts: &[HostGroup]) -> Vec<LiveGroup> {
    let mls = vault.load_groups(install).unwrap_or_default();
    hosts
        .iter()
        .copied()
        .zip(mls)
        .map(|(host, mls)| LiveGroup {
            host,
            mls,
            nickname: String::new(),
        })
        .collect()
}

fn find_group_mut<'a>(
    groups: &'a mut [LiveGroup],
    group_id_hex: &str,
) -> Result<&'a mut LiveGroup, FfiError> {
    let id = parse_identity_id(group_id_hex)?;
    groups
        .iter_mut()
        .find(|g| g.host.group_id == id)
        .ok_or_else(|| FfiError::Core("unknown group".into()))
}

fn join_request_uri(
    group_id: [u8; KEY_LEN],
    pending_id: [u8; KEY_LEN],
    key_package: &[u8],
    signing_pk: [u8; KEY_LEN],
    cap: [u8; KEY_LEN],
    hpke: [u8; KEY_LEN],
    credential_id: [u8; KEY_LEN],
) -> String {
    let bytes = cbor::encode(&Value::Map(vec![
        (0, Value::Bytes(group_id.to_vec())),
        (1, Value::Bytes(pending_id.to_vec())),
        (2, Value::Bytes(key_package.to_vec())),
        (3, Value::Bytes(signing_pk.to_vec())),
        (4, Value::Bytes(cap.to_vec())),
        (5, Value::Bytes(hpke.to_vec())),
        (6, Value::Bytes(credential_id.to_vec())),
    ]));
    format!("{JOIN_REQUEST_PREFIX}{}", ids::to_hex(&bytes))
}

struct JoinRequest {
    group_id: [u8; KEY_LEN],
    pending_id: [u8; KEY_LEN],
    key_package: Vec<u8>,
    signing_pk: [u8; KEY_LEN],
    cap: [u8; KEY_LEN],
    hpke: [u8; KEY_LEN],
    credential_id: [u8; KEY_LEN],
}

fn parse_join_request(uri: &str) -> Result<JoinRequest, FfiError> {
    let hex = uri
        .strip_prefix(JOIN_REQUEST_PREFIX)
        .ok_or_else(|| FfiError::Core("join request must start with nemo-j:1:".into()))?;
    let Value::Map(m) = cbor::decode(&ids::from_hex(hex)?)? else {
        return Err(FfiError::Core("join request".into()));
    };
    let read = |k: u64| -> Result<[u8; KEY_LEN], FfiError> {
        Ok(ids::copy_fixed(cbor::expect_bytes(cbor::map_get(&m, k)?)?)?)
    };
    Ok(JoinRequest {
        group_id: read(0)?,
        pending_id: read(1)?,
        key_package: cbor::expect_bytes(cbor::map_get(&m, 2)?)?.to_vec(),
        signing_pk: read(3)?,
        cap: read(4)?,
        hpke: read(5)?,
        credential_id: read(6)?,
    })
}

fn parse_group_invite(uri: &str) -> Result<GroupInvite, FfiError> {
    let hex = uri
        .strip_prefix(GROUP_INVITE_PREFIX)
        .ok_or_else(|| FfiError::Core("group invite must start with nemo-g:1:".into()))?;
    Ok(GroupInvite::decode(&ids::from_hex(hex)?)?)
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
            inner: Mutex::new(empty_inner(
                Some(ClientState::Local(install)),
                None,
                Some(export.mnemonic),
            )),
        }))
    }

    /// Create an identity and lock it in `dir` (ADR-0034).
    #[uniffi::constructor]
    pub fn create_at(dir: String, passphrase: String) -> Result<Arc<Self>, FfiError> {
        let (install, export) = Installation::create()?;
        let vault = Vault::create(&dir, &passphrase, &install)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(empty_inner(
                Some(ClientState::Local(install)),
                Some(vault),
                Some(export.mnemonic),
            )),
        }))
    }

    #[uniffi::constructor]
    pub fn open_at(dir: String, passphrase: String) -> Result<Arc<Self>, FfiError> {
        let (vault, install) = Vault::open(&dir, &passphrase)?;
        let (state, groups) = match vault.load_home()? {
            Some(home) if !home.home_base.is_empty() => {
                let hosts = home.groups.clone();
                let groups = load_live_groups(&vault, &install, &hosts);
                let transport = HttpHome::new(home.home_base.clone())?;
                (
                    ClientState::Registered(HomeSession::resume(transport, install, home)),
                    groups,
                )
            }
            _ => (ClientState::Local(install), Vec::new()),
        };
        let mut inner = empty_inner(Some(state), Some(vault), None);
        inner.groups = groups;
        Ok(Arc::new(Self {
            inner: Mutex::new(inner),
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
        let new_rows = {
            let Inner {
                state,
                groups,
                pending,
                inbox,
                next_seq,
                ..
            } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let rows = block_on(session.fetch_mailbox())?;
            if rows.is_empty() {
                Vec::new()
            } else {
                let contacts: Vec<_> = session.contacts.keys().copied().collect();
                let mut new_rows = Vec::new();
                let mut gossip = Vec::new();
                for row in &rows {
                    match block_on(session.install.decrypt_incoming(&row.inner, &contacts)) {
                        Ok((peer, plaintext)) => match decode(&plaintext) {
                            Ok(AppMessage {
                                header,
                                body: AppBody::Text { text },
                            }) => {
                                gossip.push(peer);
                                new_rows.push(push_text(
                                    inbox,
                                    next_seq,
                                    ids::to_hex(&peer),
                                    header.conv_seq,
                                    text,
                                    header.sent_at,
                                ));
                            }
                            Ok(AppMessage {
                                body: AppBody::Capability { contact_capability },
                                ..
                            }) => {
                                if let Some(contact) = session.contacts.get_mut(&peer) {
                                    contact.delivery_capability = contact_capability;
                                }
                            }
                            _ => {}
                        },
                        Err(CoreError::WrongMailboxType) => {}
                        Err(_) => {}
                    }
                }

                let mut joined = Vec::new();
                for row in &rows {
                    let Ok(body) = mailbox::expect_type(&row.inner, MessageType::MlsHandshake)
                    else {
                        continue;
                    };
                    if !PendingJoin::is_welcome(body) {
                        continue;
                    }
                    let keys: Vec<_> = pending.keys().copied().collect();
                    for gid in keys {
                        let Some(entry) = pending.remove(&gid) else {
                            continue;
                        };
                        match entry.pending.join(session.install.mls_provider(), body) {
                            Ok(mls) => {
                                joined.push((gid, mls, entry.host));
                                break;
                            }
                            Err(_) => {}
                        }
                    }
                }
                for (gid, mls, host) in joined {
                    let live = HostGroup {
                        group_id: gid,
                        cred: host.cred,
                    };
                    session.remember_group(live);
                    groups.push(LiveGroup {
                        host: live,
                        mls,
                        nickname: String::new(),
                    });
                }

                let provider = session.install.mls_provider();
                for row in &rows {
                    match row.inner.padded_message.type_ {
                        MessageType::MlsHandshake => {
                            for g in groups.iter_mut() {
                                let _ = g.mls.apply_handshake_from_mailbox(provider, &row.inner);
                            }
                        }
                        MessageType::MlsApp => {
                            for g in groups.iter_mut() {
                                if let Ok(plaintext) =
                                    g.mls.decrypt_from_mailbox(provider, &row.inner)
                                {
                                    if let Ok(AppMessage {
                                        header,
                                        body: AppBody::Text { text },
                                    }) = decode(&plaintext)
                                    {
                                        new_rows.push(push_text(
                                            inbox,
                                            next_seq,
                                            ids::to_hex(&g.host.group_id),
                                            header.conv_seq,
                                            text,
                                            header.sent_at,
                                        ));
                                    }
                                    break;
                                }
                            }
                        }
                        MessageType::RemoveBundle => {
                            for g in groups.iter_mut() {
                                let _ = g.mls.apply_remove_from_mailbox(provider, &row.inner);
                            }
                        }
                        _ => {}
                    }
                }

                block_on(session.ack())?;
                let now = now_unix();
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
                            let _ =
                                block_on(session.send_to(&peer, TtlBucket::DEFAULT, &bytes, now));
                        }
                    }
                }
                new_rows
            }
        };
        persist(&inner)?;
        Ok(new_rows)
    }

    pub fn create_group(&self, nickname: String) -> Result<String, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let group_id = {
            let Inner { state, groups, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let mls = session.install.create_group()?;
            let cap = block_on(session.mint_contact())?;
            let host = block_on(session.create_group(mls.group_signing_public(), cap))?;
            session.remember_group(host);
            let id = ids::to_hex(&host.group_id);
            groups.push(LiveGroup {
                host,
                mls,
                nickname,
            });
            id
        };
        persist(&inner)?;
        Ok(group_id)
    }

    pub fn mint_group_invite(&self, group_id_hex: String) -> Result<String, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let uri = {
            let Inner { state, groups, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let g = find_group_mut(groups, &group_id_hex)?;
            let invite = g
                .mls
                .sign_invite(g.host.group_id, Group::default_invite_ttl(), None)?;
            block_on(session.store_invite(g.host.group_id, &g.host.cred, &invite))?;
            format!("{GROUP_INVITE_PREFIX}{}", ids::to_hex(&invite.encode()))
        };
        persist(&inner)?;
        Ok(uri)
    }

    pub fn accept_group_invite(&self, invite_uri: String) -> Result<String, FfiError> {
        let invite = parse_group_invite(&invite_uri)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let uri = {
            let Inner { state, pending, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let acc = block_on(session.accept_invite(invite.group_id, invite.nonce))?;
            let pending_join = session.install.prepare_join(acc.cred.credential_id)?;
            let cap = block_on(session.mint_contact())?;
            let hpke = session.hpke_public();
            let signing_pk = pending_join.group_signing_public();
            let uri = join_request_uri(
                invite.group_id,
                acc.pending_id,
                &pending_join.key_package,
                signing_pk,
                cap,
                hpke,
                acc.cred.credential_id,
            );
            pending.insert(
                invite.group_id,
                PendingEntry {
                    pending: pending_join,
                    host: acc,
                },
            );
            uri
        };
        persist(&inner)?;
        Ok(uri)
    }

    pub fn admit_join(&self, join_request_uri: String) -> Result<String, FfiError> {
        let req = parse_join_request(&join_request_uri)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        {
            let Inner { state, groups, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let g = groups
                .iter_mut()
                .find(|g| g.host.group_id == req.group_id)
                .ok_or_else(|| FfiError::Core("unknown group".into()))?;
            let admit = g.mls.sign_admit(req.group_id, req.pending_id)?;
            let (commit, welcome) = g
                .mls
                .admit(session.install.mls_provider(), &req.key_package)?;
            block_on(session.admit(
                req.group_id,
                &g.host.cred,
                &admit,
                req.signing_pk,
                req.cap,
                req.hpke,
            ))?;
            block_on(session.group_append(
                req.group_id,
                &g.host.cred,
                MessageType::MlsHandshake,
                commit,
            ))?;
            let outer = Group::wrap_handshake(&req.hpke, req.cap, TtlBucket::DEFAULT, welcome)?;
            block_on(session.post_envelope(&outer))?;
        }
        persist(&inner)?;
        Ok(ids::to_hex(&req.credential_id))
    }

    pub fn send_group_text(
        &self,
        group_id_hex: String,
        text: String,
    ) -> Result<DisplayRow, FfiError> {
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        let now = now_unix();
        let row = {
            let seq = bump_seq(&mut inner.next_seq, &group_id_hex);
            let ptext = encode_text(seq, now, &text)?;
            let Inner { state, groups, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let g = find_group_mut(groups, &group_id_hex)?;
            let body = g.mls.encrypt(session.install.mls_provider(), &ptext)?;
            block_on(session.group_append(
                g.host.group_id,
                &g.host.cred,
                MessageType::MlsApp,
                body,
            ))?;
            DisplayRow {
                conv_id: group_id_hex,
                conv_seq: seq,
                text,
                sent_at: now,
            }
        };
        inner.inbox.push(row.clone());
        persist(&inner)?;
        Ok(row)
    }

    pub fn remove_group_member(
        &self,
        group_id_hex: String,
        credential_id_hex: String,
    ) -> Result<(), FfiError> {
        let cred = parse_identity_id(&credential_id_hex)?;
        let mut inner = self.inner.lock().map_err(|_| lock_err())?;
        {
            let Inner { state, groups, .. } = &mut *inner;
            let session = match state {
                Some(ClientState::Registered(session)) => session,
                Some(ClientState::Local(_)) => return Err(CoreError::NotRegistered.into()),
                None => return Err(FfiError::Core("busy".into())),
            };
            let g = find_group_mut(groups, &group_id_hex)?;
            let bundle = g.mls.remove(session.install.mls_provider(), cred)?;
            block_on(session.group_append(
                g.host.group_id,
                &g.host.cred,
                MessageType::RemoveBundle,
                bundle.encode()?,
            ))?;
        }
        persist(&inner)?;
        Ok(())
    }

    pub fn list_groups(&self) -> Result<Vec<GroupRow>, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner
            .groups
            .iter()
            .map(|g| GroupRow {
                group_id: ids::to_hex(&g.host.group_id),
                nickname: g.nickname.clone(),
                member_count: g.mls.member_count() as u64,
            })
            .collect())
    }

    pub fn group_member_ids(&self, group_id_hex: String) -> Result<Vec<String>, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        let id = parse_identity_id(&group_id_hex)?;
        let g = inner
            .groups
            .iter()
            .find(|g| g.host.group_id == id)
            .ok_or_else(|| FfiError::Core("unknown group".into()))?;
        Ok(g.mls
            .member_credential_ids()
            .into_iter()
            .map(|c| ids::to_hex(&c))
            .collect())
    }

    pub fn inbox(&self) -> Result<Vec<DisplayRow>, FfiError> {
        let inner = self.inner.lock().map_err(|_| lock_err())?;
        Ok(inner.inbox.clone())
    }
}

fn push_text(
    inbox: &mut Vec<DisplayRow>,
    next_seq: &mut HashMap<String, u64>,
    conv_id: String,
    conv_seq: u64,
    text: String,
    sent_at: u64,
) -> DisplayRow {
    let seq = *next_seq.get(&conv_id).unwrap_or(&0);
    if conv_seq > seq {
        next_seq.insert(conv_id.clone(), conv_seq);
    }
    let row = DisplayRow {
        conv_id,
        conv_seq,
        text,
        sent_at,
    };
    inbox.push(row.clone());
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_server::{router, AppState};
    use rand::RngCore;
    use std::fs;
    use std::sync::Arc;

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

    fn client_at(dir: &std::path::Path) -> Arc<NemoClient> {
        NemoClient::create_at(dir.to_string_lossy().into_owned(), "correct horse".into()).unwrap()
    }

    #[test]
    fn group_invite_admit_restart_and_remove() {
        let base = serve_home();
        let alice_dir = temp_dir("nemo-ffi-ga");
        let bob_dir = temp_dir("nemo-ffi-gb");
        let carol_dir = temp_dir("nemo-ffi-gc");
        let alice = client_at(&alice_dir);
        let bob = client_at(&bob_dir);
        let carol = client_at(&carol_dir);
        alice.register(base.clone()).unwrap();
        bob.register(base.clone()).unwrap();
        carol.register(base).unwrap();

        let gid = alice.create_group("crew".into()).unwrap();
        let invite = alice.mint_group_invite(gid.clone()).unwrap();
        assert!(invite.starts_with("nemo-g:1:"));
        let join = bob.accept_group_invite(invite.clone()).unwrap();
        assert!(join.starts_with("nemo-j:1:"));
        let denied = carol.accept_group_invite(invite).unwrap_err();
        assert!(denied.to_string().to_lowercase().contains("denied"));

        let bob_cred = alice.admit_join(join).unwrap();
        let _ = bob.fetch_now().unwrap();
        assert_eq!(bob.list_groups().unwrap().len(), 1);
        assert_eq!(alice.list_groups().unwrap()[0].member_count, 2);

        let sent = alice
            .send_group_text(gid.clone(), "hello crew".into())
            .unwrap();
        let rows = bob.fetch_now().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "hello crew");
        assert_eq!(rows[0].conv_id, gid);
        assert_eq!(rows[0].conv_seq, sent.conv_seq);

        drop(alice);
        let alice2 = NemoClient::open_at(
            alice_dir.to_string_lossy().into_owned(),
            "correct horse".into(),
        )
        .unwrap();
        let groups = alice2.list_groups().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group_id, gid);
        alice2
            .send_group_text(gid.clone(), "after reopen".into())
            .unwrap();
        let again = bob.fetch_now().unwrap();
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].text, "after reopen");

        alice2.remove_group_member(gid.clone(), bob_cred).unwrap();
        let err = bob.send_group_text(gid, "still here".into()).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("denied"));

        drop(alice2);
        drop(bob);
        drop(carol);
        let _ = fs::remove_dir_all(&alice_dir);
        let _ = fs::remove_dir_all(&bob_dir);
        let _ = fs::remove_dir_all(&carol_dir);
    }
}
