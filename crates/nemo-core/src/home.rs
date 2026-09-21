//! Client-to-home HTTP (phase 8). The server crate is not a runtime dependency.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use nemo_wire::cbor::{self, Value};
use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope, TtlBucket};
use nemo_wire::ids::{self, IdentityId, ServerId, KEY_LEN};
use nemo_wire::prekey::SignedPrekey;
use nemo_wire::{
    ContactCard, DiscoveryRecord, GroupAdmit, GroupInvite, HomeServerBinding, RevocationStatement,
    ServerBundle,
};

use crate::call::TurnConfig;
use crate::discovery::{resolve_contact, ContactPin, Discovery, DISCOVERY_REFRESH_SECS};
use crate::error::{CoreError, Result};
use crate::identity::Installation;
use crate::privacy::{private_send_delay_ms, PrivacyMode};

pub const FETCH_LIMIT: u64 = 64;
const CARD_TTL_SECS: u64 = 30 * 24 * 3600;

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: &'static str,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait HomeTransport: Send + Sync {
    fn call(&self, req: HttpRequest) -> impl Future<Output = Result<HttpResponse>> + Send;
    /// HTTP origin for a group that still lives on another home. In-process
    /// transports stay on `self`.
    fn redirect(&self, origin: &str) -> Option<Self>
    where
        Self: Sized,
    {
        let _ = origin;
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostCred {
    pub credential_id: [u8; KEY_LEN],
    pub credential_secret: [u8; KEY_LEN],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostGroup {
    pub group_id: [u8; KEY_LEN],
    pub cred: HostCred,
    /// Group host HTTP origin. Empty means the current home (in-process tests).
    pub host_base: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostAccept {
    pub pending_id: [u8; KEY_LEN],
    pub cred: HostCred,
    pub invitee_binding: Option<[u8; KEY_LEN]>,
}

impl HostAccept {
    pub fn encode(&self) -> Vec<u8> {
        let mut pairs = vec![
            (0, Value::Bytes(self.pending_id.to_vec())),
            (1, Value::Bytes(self.cred.credential_id.to_vec())),
            (2, Value::Bytes(self.cred.credential_secret.to_vec())),
        ];
        if let Some(b) = self.invitee_binding {
            pairs.push((3, Value::Bytes(b.to_vec())));
        }
        cbor::encode(&Value::Map(pairs))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        Ok(Self {
            pending_id: ids::copy_fixed(
                cbor::expect_bytes(cbor::map_get(&m, 0).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?,
            )?,
            cred: HostCred {
                credential_id: ids::copy_fixed(
                    cbor::expect_bytes(cbor::map_get(&m, 1).map_err(|_| CoreError::VaultCorrupt)?)
                        .map_err(|_| CoreError::VaultCorrupt)?,
                )?,
                credential_secret: ids::copy_fixed(
                    cbor::expect_bytes(cbor::map_get(&m, 2).map_err(|_| CoreError::VaultCorrupt)?)
                        .map_err(|_| CoreError::VaultCorrupt)?,
                )?,
            },
            invitee_binding: match cbor::map_get_opt(&m, 3) {
                Some(v) => Some(ids::copy_fixed(
                    cbor::expect_bytes(v).map_err(|_| CoreError::VaultCorrupt)?,
                )?),
                None => None,
            },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnqueueResult {
    Local(u64),
    Queued,
}

#[derive(Clone, Debug)]
pub struct MailboxRow {
    pub seq: u64,
    pub inner: InnerEnvelope,
}

/// A 1:1 peer we can send to after invite-first discovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredContact {
    pub pin: ContactPin,
    pub delivery_capability: [u8; KEY_LEN],
    pub dest_hpke: [u8; KEY_LEN],
    pub last_discovery_unix: u64,
    pub revoked: bool,
    /// HTTP origin of the peer's current home. Empty means this session's transport.
    pub home_origin: String,
}

/// Durable home-server binding: survives vault reopen (I2).
#[derive(Clone, Debug)]
pub struct HomeState {
    pub bundle: ServerBundle,
    pub cursor: u64,
    pub contacts: HashMap<IdentityId, StoredContact>,
    pub groups: Vec<HostGroup>,
    /// HTTP origin for [`HttpHome`]; empty when the transport is in-process.
    pub home_base: String,
    pub privacy: PrivacyMode,
}

impl HomeState {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut contacts = Vec::new();
        let mut ids: Vec<_> = self.contacts.keys().copied().collect();
        ids.sort();
        for id in ids {
            let c = &self.contacts[&id];
            contacts.push(Value::Map(vec![
                (0, Value::Bytes(id.to_vec())),
                (1, Value::Bytes(c.pin.identity_public_key.to_vec())),
                (2, Value::Bytes(c.pin.revocation_public_key.to_vec())),
                (3, Value::Uint(c.pin.seq)),
                (4, Value::Bytes(c.delivery_capability.to_vec())),
                (5, Value::Bytes(c.dest_hpke.to_vec())),
                (6, Value::Uint(c.last_discovery_unix)),
                (7, Value::Uint(u64::from(c.revoked))),
                (8, Value::Text(c.home_origin.clone())),
            ]));
        }
        let groups = self
            .groups
            .iter()
            .map(|g| {
                Value::Array(vec![
                    Value::Bytes(g.group_id.to_vec()),
                    Value::Bytes(g.cred.credential_id.to_vec()),
                    Value::Bytes(g.cred.credential_secret.to_vec()),
                    Value::Text(g.host_base.clone()),
                ])
            })
            .collect();
        Ok(cbor::encode(&Value::Map(vec![
            (0, Value::Uint(1)),
            (1, Value::Bytes(self.bundle.encode())),
            (2, Value::Uint(self.cursor)),
            (3, Value::Array(contacts)),
            (4, Value::Array(groups)),
            (5, Value::Text(self.home_base.clone())),
            (6, Value::Uint(privacy_to_u64(self.privacy))),
        ])))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes).map_err(|_| CoreError::VaultCorrupt)? else {
            return Err(CoreError::VaultCorrupt);
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        if version != 1 {
            return Err(CoreError::VaultCorrupt);
        }
        let bundle = ServerBundle::decode(
            cbor::expect_bytes(cbor::map_get(&m, 1).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)?,
        )?;
        let cursor = cbor::expect_uint(cbor::map_get(&m, 2).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?;
        let mut contacts = HashMap::new();
        for item in cbor::expect_array(cbor::map_get(&m, 3).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?
        {
            let Value::Map(cm) = item else {
                return Err(CoreError::VaultCorrupt);
            };
            let id = ids::copy_fixed(
                cbor::expect_bytes(cbor::map_get(cm, 0).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?,
            )?;
            let pk = ids::copy_fixed(
                cbor::expect_bytes(cbor::map_get(cm, 1).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?,
            )?;
            let rpk = ids::copy_fixed(
                cbor::expect_bytes(cbor::map_get(cm, 2).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?,
            )?;
            let seq = cbor::expect_uint(cbor::map_get(cm, 3).map_err(|_| CoreError::VaultCorrupt)?)
                .map_err(|_| CoreError::VaultCorrupt)?;
            let cap = ids::copy_fixed(
                cbor::expect_bytes(cbor::map_get(cm, 4).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?,
            )?;
            let hpke = ids::copy_fixed(
                cbor::expect_bytes(cbor::map_get(cm, 5).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?,
            )?;
            let last =
                cbor::expect_uint(cbor::map_get(cm, 6).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?;
            let revoked =
                cbor::expect_uint(cbor::map_get(cm, 7).map_err(|_| CoreError::VaultCorrupt)?)
                    .map_err(|_| CoreError::VaultCorrupt)?
                    != 0;
            let home_origin = match cbor::map_get_opt(cm, 8) {
                Some(v) => cbor::expect_text(v)
                    .map_err(|_| CoreError::VaultCorrupt)?
                    .to_owned(),
                None => String::new(),
            };
            contacts.insert(
                id,
                StoredContact {
                    pin: ContactPin {
                        identity_id: id,
                        identity_public_key: pk,
                        revocation_public_key: rpk,
                        seq,
                    },
                    delivery_capability: cap,
                    dest_hpke: hpke,
                    last_discovery_unix: last,
                    revoked,
                    home_origin,
                },
            );
        }
        let mut groups = Vec::new();
        for item in cbor::expect_array(cbor::map_get(&m, 4).map_err(|_| CoreError::VaultCorrupt)?)
            .map_err(|_| CoreError::VaultCorrupt)?
        {
            let Value::Array(row) = item else {
                return Err(CoreError::VaultCorrupt);
            };
            if row.len() != 3 && row.len() != 4 {
                return Err(CoreError::VaultCorrupt);
            }
            groups.push(HostGroup {
                group_id: ids::copy_fixed(cbor::expect_bytes(&row[0])?)?,
                cred: HostCred {
                    credential_id: ids::copy_fixed(cbor::expect_bytes(&row[1])?)?,
                    credential_secret: ids::copy_fixed(cbor::expect_bytes(&row[2])?)?,
                },
                host_base: if row.len() == 4 {
                    cbor::expect_text(&row[3])
                        .map_err(|_| CoreError::VaultCorrupt)?
                        .to_owned()
                } else {
                    String::new()
                },
            });
        }
        Ok(Self {
            bundle,
            cursor,
            contacts,
            groups,
            home_base: match cbor::map_get_opt(&m, 5) {
                Some(v) => cbor::expect_text(v)
                    .map_err(|_| CoreError::VaultCorrupt)?
                    .to_owned(),
                None => String::new(),
            },
            privacy: match cbor::map_get_opt(&m, 6) {
                Some(v) => {
                    privacy_from_u64(cbor::expect_uint(v).map_err(|_| CoreError::VaultCorrupt)?)?
                }
                None => PrivacyMode::Normal,
            },
        })
    }
}

pub struct HomeSession<T> {
    transport: T,
    pub install: Installation,
    pub bundle: ServerBundle,
    pub cursor: u64,
    pub contacts: HashMap<IdentityId, StoredContact>,
    pub groups: Vec<HostGroup>,
    pub home_base: String,
    pub privacy: PrivacyMode,
    pending_outers: Mutex<Vec<OuterEnvelope>>,
}

impl<T: HomeTransport> HomeSession<T> {
    /// Fetch the host bundle, mint a card, register the mailbox.
    pub async fn register(
        transport: T,
        mut install: Installation,
        now_unix: u64,
    ) -> Result<(Self, ContactCard)> {
        let bundle = fetch_bundle(&transport).await?;
        let card = install.mint_card(
            bundle.server_hpke_public_key,
            &bundle.host,
            now_unix.saturating_add(CARD_TTL_SECS),
        )?;
        let res = transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/register".into(),
                headers: vec![],
                body: card.encode()?,
            })
            .await?;
        check_empty(&res)?;
        Ok((
            Self {
                transport,
                install,
                bundle,
                cursor: 0,
                contacts: HashMap::new(),
                groups: Vec::new(),
                home_base: String::new(),
                privacy: PrivacyMode::Normal,
                pending_outers: Mutex::new(Vec::new()),
            },
            card,
        ))
    }

    /// Same identity, new mailbox on `new_transport` (README §11). Contacts and
    /// groups stay; group **host** migration stays deferred.
    pub async fn rehome(
        &mut self,
        new_transport: T,
        new_base: &str,
        now_unix: u64,
    ) -> Result<ContactCard> {
        let bundle = fetch_bundle(&new_transport).await?;
        if bundle.server_id == self.bundle.server_id {
            return Err(CoreError::AlreadyRegistered);
        }
        let card = self.install.mint_card(
            bundle.server_hpke_public_key,
            &bundle.host,
            now_unix.saturating_add(CARD_TTL_SECS),
        )?;
        let res = new_transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/register".into(),
                headers: vec![],
                body: card.encode()?,
            })
            .await?;
        check_empty(&res)?;
        let old_base = self.home_base.clone();
        for g in &mut self.groups {
            if g.host_base.is_empty() {
                g.host_base = old_base.clone();
            }
        }
        let old = std::mem::replace(&mut self.transport, new_transport);
        self.bundle = bundle;
        self.cursor = 0;
        self.set_home_base(new_base);
        self.restock_publish().await?;
        let hpke = self.hpke_public();
        let groups = self.groups.clone();
        for g in &groups {
            if let Ok(cap) = self.mint_contact().await {
                let _ = Self::post_fanout(&old, g.group_id, &g.cred, cap, hpke).await;
            }
        }
        let _ = Self::post_binding(&old, &card).await;
        Ok(card)
    }

    pub fn resume(transport: T, install: Installation, state: HomeState) -> Self {
        Self {
            transport,
            install,
            bundle: state.bundle,
            cursor: state.cursor,
            contacts: state.contacts,
            groups: state.groups,
            home_base: state.home_base,
            privacy: state.privacy,
            pending_outers: Mutex::new(Vec::new()),
        }
    }

    pub fn snapshot(&self) -> HomeState {
        HomeState {
            bundle: self.bundle.clone(),
            cursor: self.cursor,
            contacts: self.contacts.clone(),
            groups: self.groups.clone(),
            home_base: self.home_base.clone(),
            privacy: self.privacy,
        }
    }

    pub fn set_privacy(&mut self, mode: PrivacyMode) -> Result<()> {
        match mode {
            PrivacyMode::Normal | PrivacyMode::Private => {
                self.privacy = mode;
                Ok(())
            }
            PrivacyMode::High => Err(CoreError::MaximumNotShipped),
        }
    }

    pub fn set_home_base(&mut self, base: impl Into<String>) {
        self.home_base = base.into().trim_end_matches('/').to_string();
    }

    fn origin_for_binding(&self, binding: &HomeServerBinding) -> String {
        peer_http_origin(
            &binding.host,
            binding.server_id,
            &self.bundle.host,
            self.bundle.server_id,
            &self.home_base,
        )
    }

    async fn call_on(&self, origin: &str, req: HttpRequest) -> Result<HttpResponse> {
        let origin = origin.trim_end_matches('/');
        if origin.is_empty() || origin == self.home_base {
            return self.transport.call(req).await;
        }
        match self.transport.redirect(origin) {
            Some(alt) => alt.call(req).await,
            None => Err(CoreError::Transport(format!("no transport for {origin}"))),
        }
    }

    pub async fn discovery_at(&self, origin: &str, identity_id: IdentityId) -> Result<Discovery> {
        let res = self
            .call_on(
                origin,
                HttpRequest {
                    method: "GET",
                    path: format!("/v1/discovery/{}", ids::to_hex(&identity_id)),
                    headers: vec![],
                    body: vec![],
                },
            )
            .await?;
        let body = check_body(&res)?;
        let rec = DiscoveryRecord::decode(body)?;
        Ok(Discovery {
            identity_public_key: rec.identity_public_key,
            revocation_public_key: rec.revocation_public_key,
            binding: rec.binding,
            revocation: rec.revocation,
        })
    }

    pub async fn fetch_prekey_at(
        &self,
        origin: &str,
        share_token: [u8; KEY_LEN],
    ) -> Result<SignedPrekey> {
        let res = self
            .call_on(
                origin,
                HttpRequest {
                    method: "GET",
                    path: "/v1/prekeys".into(),
                    headers: vec![("nemo-token".into(), ids::to_hex(&share_token))],
                    body: vec![],
                },
            )
            .await?;
        Ok(SignedPrekey::decode(check_body(&res)?)?)
    }

    /// Fetch the current binding, start PQXDH, and remember the delivery address.
    pub async fn add_contact(
        &mut self,
        card: &ContactCard,
        delivery_capability: [u8; KEY_LEN],
        now_unix: u64,
    ) -> Result<()> {
        let origin = self.origin_for_binding(&card.binding);
        let disc = self.discovery_at(&origin, card.identity_id()).await?;
        let binding = resolve_contact(card, &disc, now_unix)?;
        let live_origin = self.origin_for_binding(&binding);
        let prekey = self.fetch_prekey_at(&live_origin, card.share_token).await?;
        self.install.start_session(card, &prekey, now_unix).await?;
        let mut pin = ContactPin::from_card(card);
        pin.seq = binding.seq;
        self.contacts.insert(
            card.identity_id(),
            StoredContact {
                pin,
                delivery_capability,
                dest_hpke: binding.server_hpke_public_key,
                last_discovery_unix: now_unix,
                revoked: false,
                home_origin: live_origin,
            },
        );
        Ok(())
    }

    pub async fn refresh_contact(&mut self, peer: &IdentityId, now_unix: u64) -> Result<()> {
        let origin = self
            .contacts
            .get(peer)
            .map(|c| c.home_origin.clone())
            .unwrap_or_default();
        let disc = self.discovery_at(&origin, *peer).await?;
        disc.verify(now_unix)?;
        let own_host = self.bundle.host.clone();
        let own_server = self.bundle.server_id;
        let own_base = self.home_base.clone();
        let contact = self
            .contacts
            .get_mut(peer)
            .ok_or(CoreError::UnknownContact)?;
        if disc.revocation.is_some() {
            contact.revoked = true;
            return Err(CoreError::Revoked);
        }
        let binding = contact.pin.refresh(&disc, now_unix)?;
        contact.dest_hpke = binding.server_hpke_public_key;
        contact.home_origin = peer_http_origin(
            &binding.host,
            binding.server_id,
            &own_host,
            own_server,
            &own_base,
        );
        contact.last_discovery_unix = now_unix;
        Ok(())
    }

    /// Fetch discovery and refuse a valid revocation.
    ///
    /// Known contacts are checked on their pinned home. Unknown identities are
    /// checked on this home; `Denied` there is not revocation (they may live
    /// elsewhere).
    pub async fn check_discovery_not_revoked(
        &self,
        identity_id: IdentityId,
        now_unix: u64,
    ) -> Result<()> {
        let origin = self
            .contacts
            .get(&identity_id)
            .map(|c| c.home_origin.as_str())
            .filter(|s| !s.is_empty());
        match self.discovery_at(origin.unwrap_or(""), identity_id).await {
            Ok(disc) => {
                disc.verify(now_unix)?;
                disc.check_not_revoked()
            }
            Err(CoreError::Denied) if origin.is_none() => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Stale 1:1 pins (ADR-0004). Returns identities newly marked revoked this pass.
    pub async fn refresh_idle_discovery(&mut self, now_unix: u64) -> Vec<IdentityId> {
        let own = self.identity_id();
        let peers: Vec<_> = self.contacts.keys().copied().collect();
        let mut newly = Vec::new();
        for id in peers {
            if id == own {
                continue;
            }
            let Some(c) = self.contacts.get(&id) else {
                continue;
            };
            if c.revoked {
                continue;
            }
            let stale = now_unix.saturating_sub(c.last_discovery_unix) >= DISCOVERY_REFRESH_SECS;
            if !stale {
                continue;
            }
            match self.refresh_contact(&id, now_unix).await {
                Ok(()) => {}
                Err(CoreError::Revoked) => newly.push(id),
                Err(_) => {}
            }
        }
        newly
    }

    pub fn revoked_contact_ids(&self) -> Vec<IdentityId> {
        self.contacts
            .iter()
            .filter(|(_, c)| c.revoked)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Discovery revoke check for an identity that may not be a 1:1 contact.
    pub async fn identity_is_revoked(&self, identity_id: IdentityId, now_unix: u64) -> bool {
        matches!(
            self.check_discovery_not_revoked(identity_id, now_unix)
                .await,
            Err(CoreError::Revoked)
        )
    }

    /// Same stale window as 1:1 `send_to`. Contacts that are revoked fail closed.
    pub async fn refresh_mls_identities(
        &mut self,
        ids: &[IdentityId],
        now_unix: u64,
    ) -> Result<()> {
        let own = self.identity_id();
        for id in ids {
            if *id == own {
                continue;
            }
            if let Some(c) = self.contacts.get(id) {
                if c.revoked {
                    return Err(CoreError::Revoked);
                }
                let stale =
                    now_unix.saturating_sub(c.last_discovery_unix) >= DISCOVERY_REFRESH_SECS;
                if stale {
                    self.refresh_contact(id, now_unix).await?;
                }
            } else {
                self.check_discovery_not_revoked(*id, now_unix).await?;
            }
        }
        Ok(())
    }

    pub async fn issue_turn(&self) -> Result<TurnConfig> {
        let auth = self.owner_header(0, 1)?;
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/turn".into(),
                headers: vec![("nemo-owner".into(), auth)],
                body: vec![],
            })
            .await?;
        let m = expect_map(check_body(&res)?)?;
        Ok(TurnConfig {
            url: cbor::expect_text(cbor::map_get(&m, 0)?)?.to_owned(),
            username: cbor::expect_text(cbor::map_get(&m, 1)?)?.to_owned(),
            credential: cbor::expect_text(cbor::map_get(&m, 2)?)?.to_owned(),
            bind: std::env::var("NEMO_ICE_BIND").unwrap_or_else(|_| "127.0.0.1:0".into()),
        })
    }

    pub async fn send_to(
        &mut self,
        peer: &IdentityId,
        ttl_bucket: TtlBucket,
        plaintext: &[u8],
        now_unix: u64,
    ) -> Result<EnqueueResult> {
        let stale = match self.contacts.get(peer) {
            None => return Err(CoreError::UnknownContact),
            Some(c) if c.revoked => return Err(CoreError::Revoked),
            Some(c) => now_unix.saturating_sub(c.last_discovery_unix) >= DISCOVERY_REFRESH_SECS,
        };
        if stale {
            self.refresh_contact(peer, now_unix).await?;
        }
        let contact = self.contacts.get(peer).ok_or(CoreError::UnknownContact)?;
        if contact.revoked {
            return Err(CoreError::Revoked);
        }
        let dest_hpke = contact.dest_hpke;
        let cap = contact.delivery_capability;
        let outer = self
            .install
            .encrypt_to_mailbox(peer, &dest_hpke, cap, ttl_bucket, plaintext)
            .await?;
        self.post_envelope(&outer).await
    }

    /// Like [`send_to`] but never holds the Private batch window (calls, acks).
    pub async fn send_to_now(
        &mut self,
        peer: &IdentityId,
        ttl_bucket: TtlBucket,
        plaintext: &[u8],
        now_unix: u64,
    ) -> Result<EnqueueResult> {
        let stale = match self.contacts.get(peer) {
            None => return Err(CoreError::UnknownContact),
            Some(c) if c.revoked => return Err(CoreError::Revoked),
            Some(c) => now_unix.saturating_sub(c.last_discovery_unix) >= DISCOVERY_REFRESH_SECS,
        };
        if stale {
            self.refresh_contact(peer, now_unix).await?;
        }
        let contact = self.contacts.get(peer).ok_or(CoreError::UnknownContact)?;
        if contact.revoked {
            return Err(CoreError::Revoked);
        }
        let dest_hpke = contact.dest_hpke;
        let cap = contact.delivery_capability;
        let outer = self
            .install
            .encrypt_to_mailbox(peer, &dest_hpke, cap, ttl_bucket, plaintext)
            .await?;
        self.post_envelope_now(&outer).await
    }

    /// 1:1 file: same contact path as [`send_to`], padded to an A* DR envelope.
    pub async fn send_attachment(
        &mut self,
        peer: &IdentityId,
        ttl_bucket: TtlBucket,
        plaintext: &[u8],
        now_unix: u64,
    ) -> Result<EnqueueResult> {
        let stale = match self.contacts.get(peer) {
            None => return Err(CoreError::UnknownContact),
            Some(c) if c.revoked => return Err(CoreError::Revoked),
            Some(c) => now_unix.saturating_sub(c.last_discovery_unix) >= DISCOVERY_REFRESH_SECS,
        };
        if stale {
            self.refresh_contact(peer, now_unix).await?;
        }
        let contact = self.contacts.get(peer).ok_or(CoreError::UnknownContact)?;
        if contact.revoked {
            return Err(CoreError::Revoked);
        }
        let dest_hpke = contact.dest_hpke;
        let cap = contact.delivery_capability;
        let outer = self
            .install
            .encrypt_attachment_to_mailbox(peer, &dest_hpke, cap, ttl_bucket, plaintext)
            .await?;
        self.post_envelope(&outer).await
    }

    /// Publish one-time prekeys until the stock is above the restock floor.
    pub async fn restock_publish(&mut self) -> Result<usize> {
        let mut n = 0;
        while self.install.needs_restock() {
            self.publish_prekey().await?;
            n += 1;
        }
        Ok(n)
    }

    pub async fn submit_revocation(&self, stmt: &RevocationStatement) -> Result<()> {
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/revocation".into(),
                headers: vec![],
                body: stmt.encode(),
            })
            .await?;
        check_empty(&res)
    }

    pub fn hpke_public(&self) -> [u8; KEY_LEN] {
        self.bundle.server_hpke_public_key
    }

    pub fn identity_id(&self) -> IdentityId {
        self.install.identity_id()
    }

    pub async fn discovery(&self, identity_id: IdentityId) -> Result<Discovery> {
        self.discovery_at(&self.home_base, identity_id).await
    }

    pub async fn publish_prekey(&mut self) -> Result<()> {
        let blob = self.install.mint_prekey()?;
        let auth = self.owner_header(0, 1)?;
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/prekeys".into(),
                headers: vec![("nemo-owner".into(), auth)],
                body: blob.encode(),
            })
            .await?;
        check_empty(&res)
    }

    pub async fn fetch_prekey(&self, share_token: [u8; KEY_LEN]) -> Result<SignedPrekey> {
        self.fetch_prekey_at(&self.home_base, share_token).await
    }

    pub async fn mint_contact(&self) -> Result<[u8; KEY_LEN]> {
        self.mint_token("/v1/tokens/contact").await
    }

    pub async fn mint_share(&self) -> Result<[u8; KEY_LEN]> {
        self.mint_token("/v1/tokens/share").await
    }

    /// New `nemo:1:` card whose share token is live on this home (ADR-0007).
    pub async fn mint_share_card(&mut self, now_unix: u64) -> Result<ContactCard> {
        let share_token = self.mint_share().await?;
        self.install.card_with_share(
            self.bundle.server_hpke_public_key,
            &self.bundle.host,
            now_unix.saturating_add(CARD_TTL_SECS),
            share_token,
        )
    }

    /// Fetch, decrypt 1:1 rows against known contacts, ack.
    pub async fn ingest_mailbox(&mut self) -> Result<Vec<(IdentityId, Vec<u8>)>> {
        let rows = self.fetch_mailbox().await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let contacts: Vec<_> = self.contacts.keys().copied().collect();
        let mut opened = Vec::new();
        for row in &rows {
            match self.install.decrypt_incoming(&row.inner, &contacts).await {
                Ok(item) => opened.push(item),
                Err(CoreError::WrongMailboxType) => {}
                Err(_) => {}
            }
        }
        self.ack().await?;
        Ok(opened)
    }

    async fn mint_token(&self, path: &str) -> Result<[u8; KEY_LEN]> {
        let auth = self.owner_header(0, 1)?;
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: path.into(),
                headers: vec![("nemo-owner".into(), auth)],
                body: vec![],
            })
            .await?;
        let m = expect_map(check_body(&res)?)?;
        read_key(&m, 0)
    }

    pub async fn post_envelope(&self, outer: &OuterEnvelope) -> Result<EnqueueResult> {
        self.post_envelope_maybe_batch(outer, true).await
    }

    /// Call signaling skips Private batching so ringing is not delayed up to 5 s.
    pub async fn post_envelope_now(&self, outer: &OuterEnvelope) -> Result<EnqueueResult> {
        self.post_envelope_maybe_batch(outer, false).await
    }

    async fn post_envelope_maybe_batch(
        &self,
        outer: &OuterEnvelope,
        batch: bool,
    ) -> Result<EnqueueResult> {
        if self.privacy == PrivacyMode::High {
            return Err(CoreError::MaximumNotShipped);
        }
        if batch && self.privacy == PrivacyMode::Private {
            self.pending_outers
                .lock()
                .map_err(|_| CoreError::Transport("outbox lock".into()))?
                .push(outer.clone());
            let delay = std::time::Duration::from_millis(private_send_delay_ms());
            tokio::time::sleep(delay).await;
            return self.flush_private_outbox().await;
        }
        self.post_one_envelope(outer).await
    }

    async fn flush_private_outbox(&self) -> Result<EnqueueResult> {
        let batch = {
            let mut g = self
                .pending_outers
                .lock()
                .map_err(|_| CoreError::Transport("outbox lock".into()))?;
            std::mem::take(&mut *g)
        };
        if batch.is_empty() {
            return Ok(EnqueueResult::Queued);
        }
        let mut last = EnqueueResult::Queued;
        for outer in batch {
            last = self.post_one_envelope(&outer).await?;
        }
        Ok(last)
    }

    async fn post_one_envelope(&self, outer: &OuterEnvelope) -> Result<EnqueueResult> {
        let auth = self.owner_header(0, 1)?;
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/envelopes".into(),
                headers: vec![("nemo-owner".into(), auth)],
                body: outer.encode(),
            })
            .await?;
        let m = expect_map(check_body(&res)?)?;
        match cbor::expect_text(cbor::map_get(&m, 0)?)? {
            "ok" => Ok(EnqueueResult::Local(cbor::expect_uint(cbor::map_get(
                &m, 1,
            )?)?)),
            "queued" => Ok(EnqueueResult::Queued),
            _ => Err(CoreError::HomeHttp(res.status)),
        }
    }

    pub async fn fetch_mailbox(&mut self) -> Result<Vec<MailboxRow>> {
        let auth = self
            .install
            .mailbox_owner_auth(self.cursor, FETCH_LIMIT, now_unix())?;
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/mailbox/fetch".into(),
                headers: vec![],
                body: auth.encode(),
            })
            .await?;
        let Value::Array(items) = cbor::decode(check_body(&res)?)? else {
            return Err(CoreError::Wire(nemo_wire::WireError::Cbor(
                "fetch must be an array",
            )));
        };
        let mut rows = Vec::new();
        for item in items {
            let Value::Map(m) = item else {
                return Err(CoreError::Wire(nemo_wire::WireError::Cbor("fetch row")));
            };
            let seq = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
            let inner = InnerEnvelope::decode_padded(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?;
            rows.push(MailboxRow { seq, inner });
        }
        if let Some(last) = rows.last() {
            self.cursor = last.seq;
        }
        Ok(rows)
    }

    pub async fn ack(&self) -> Result<()> {
        let auth = self
            .install
            .mailbox_owner_auth(self.cursor, 1, now_unix())?;
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/mailbox/ack".into(),
                headers: vec![],
                body: auth.encode(),
            })
            .await?;
        check_empty(&res)
    }

    pub async fn create_group(
        &self,
        signing_pk: [u8; KEY_LEN],
        delivery_capability: [u8; KEY_LEN],
    ) -> Result<HostGroup> {
        let body = cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(signing_pk.to_vec())),
            (1, Value::Bytes(delivery_capability.to_vec())),
            (2, Value::Bytes(self.hpke_public().to_vec())),
        ]));
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/groups".into(),
                headers: vec![],
                body,
            })
            .await?;
        let m = expect_map(check_body(&res)?)?;
        Ok(HostGroup {
            group_id: read_key(&m, 0)?,
            cred: HostCred {
                credential_id: read_key(&m, 1)?,
                credential_secret: read_key(&m, 2)?,
            },
            host_base: self.home_base.clone(),
        })
    }

    pub async fn group_append(
        &self,
        group_id: [u8; KEY_LEN],
        cred: &HostCred,
        type_: MessageType,
        body: Vec<u8>,
    ) -> Result<u64> {
        let payload = cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(cred.credential_id.to_vec())),
            (1, Value::Bytes(cred.credential_secret.to_vec())),
            (2, Value::Uint(u64::from(type_ as u8))),
            (3, Value::Bytes(body)),
        ]));
        let res = self
            .call_group(
                group_id,
                HttpRequest {
                    method: "POST",
                    path: format!("/v1/groups/{}/append", ids::to_hex(&group_id)),
                    headers: vec![],
                    body: payload,
                },
            )
            .await?;
        let m = expect_map(check_body(&res)?)?;
        Ok(cbor::expect_uint(cbor::map_get(&m, 0)?)?)
    }

    pub async fn store_invite(
        &self,
        group_id: [u8; KEY_LEN],
        cred: &HostCred,
        invite: &GroupInvite,
    ) -> Result<()> {
        let body = cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(cred.credential_id.to_vec())),
            (1, Value::Bytes(cred.credential_secret.to_vec())),
            (2, Value::Bytes(invite.encode())),
        ]));
        let res = self
            .call_group(
                group_id,
                HttpRequest {
                    method: "POST",
                    path: format!("/v1/groups/{}/invites", ids::to_hex(&group_id)),
                    headers: vec![],
                    body,
                },
            )
            .await?;
        check_empty(&res)
    }

    pub async fn accept_invite(
        &self,
        group_id: [u8; KEY_LEN],
        nonce: [u8; KEY_LEN],
    ) -> Result<HostAccept> {
        let body = cbor::encode(&Value::Map(vec![(0, Value::Bytes(nonce.to_vec()))]));
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: format!("/v1/groups/{}/accept", ids::to_hex(&group_id)),
                headers: vec![],
                body,
            })
            .await?;
        let m = expect_map(check_body(&res)?)?;
        let invitee_binding = match cbor::map_get_opt(&m, 3) {
            Some(v) => Some(ids::copy_fixed(cbor::expect_bytes(v)?)?),
            None => None,
        };
        Ok(HostAccept {
            pending_id: read_key(&m, 0)?,
            cred: HostCred {
                credential_id: read_key(&m, 1)?,
                credential_secret: read_key(&m, 2)?,
            },
            invitee_binding,
        })
    }

    pub async fn admit(
        &self,
        group_id: [u8; KEY_LEN],
        admitter: &HostCred,
        admit: &GroupAdmit,
        joiner_signing_pk: [u8; KEY_LEN],
        joiner_capability: [u8; KEY_LEN],
        joiner_hpke: [u8; KEY_LEN],
    ) -> Result<[u8; KEY_LEN]> {
        let body = cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(admitter.credential_id.to_vec())),
            (1, Value::Bytes(admitter.credential_secret.to_vec())),
            (2, Value::Bytes(admit.encode())),
            (3, Value::Bytes(joiner_signing_pk.to_vec())),
            (4, Value::Bytes(joiner_capability.to_vec())),
            (5, Value::Bytes(joiner_hpke.to_vec())),
        ]));
        let res = self
            .call_group(
                group_id,
                HttpRequest {
                    method: "POST",
                    path: format!("/v1/groups/{}/admit", ids::to_hex(&group_id)),
                    headers: vec![],
                    body,
                },
            )
            .await?;
        let m = expect_map(check_body(&res)?)?;
        read_key(&m, 0)
    }

    pub async fn refresh_fanout(
        &self,
        group_id: [u8; KEY_LEN],
        cred: &HostCred,
        delivery_capability: [u8; KEY_LEN],
        home_hpke_public: [u8; KEY_LEN],
    ) -> Result<()> {
        let origin = self.group_origin(group_id);
        if let Some(alt) = self.transport.redirect(&origin) {
            Self::post_fanout(&alt, group_id, cred, delivery_capability, home_hpke_public).await
        } else {
            Self::post_fanout(
                &self.transport,
                group_id,
                cred,
                delivery_capability,
                home_hpke_public,
            )
            .await
        }
    }

    async fn post_fanout(
        transport: &T,
        group_id: [u8; KEY_LEN],
        cred: &HostCred,
        delivery_capability: [u8; KEY_LEN],
        home_hpke_public: [u8; KEY_LEN],
    ) -> Result<()> {
        let body = cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(cred.credential_id.to_vec())),
            (1, Value::Bytes(cred.credential_secret.to_vec())),
            (2, Value::Bytes(delivery_capability.to_vec())),
            (3, Value::Bytes(home_hpke_public.to_vec())),
        ]));
        let res = transport
            .call(HttpRequest {
                method: "POST",
                path: format!("/v1/groups/{}/fanout", ids::to_hex(&group_id)),
                headers: vec![],
                body,
            })
            .await?;
        check_empty(&res)
    }

    async fn post_binding(transport: &T, card: &ContactCard) -> Result<()> {
        let res = transport
            .call(HttpRequest {
                method: "POST",
                path: "/v1/binding".into(),
                headers: vec![],
                body: card.encode()?,
            })
            .await?;
        check_empty(&res)
    }

    pub async fn upload_file(
        &self,
        group_id: [u8; KEY_LEN],
        cred: &HostCred,
        fetch_token: [u8; KEY_LEN],
        body: Vec<u8>,
    ) -> Result<()> {
        let res = self
            .call_group(
                group_id,
                HttpRequest {
                    method: "POST",
                    path: format!(
                        "/v1/groups/{}/files/{}",
                        ids::to_hex(&group_id),
                        ids::to_hex(&fetch_token)
                    ),
                    headers: vec![
                        ("nemo-cred-id".into(), ids::to_hex(&cred.credential_id)),
                        (
                            "nemo-cred-secret".into(),
                            ids::to_hex(&cred.credential_secret),
                        ),
                    ],
                    body,
                },
            )
            .await?;
        check_empty(&res)
    }

    pub async fn fetch_file(
        &self,
        group_id: [u8; KEY_LEN],
        cred: &HostCred,
        fetch_token: [u8; KEY_LEN],
    ) -> Result<Vec<u8>> {
        let res = self
            .call_group(
                group_id,
                HttpRequest {
                    method: "GET",
                    path: format!(
                        "/v1/groups/{}/files/{}",
                        ids::to_hex(&group_id),
                        ids::to_hex(&fetch_token)
                    ),
                    headers: vec![
                        ("nemo-cred-id".into(), ids::to_hex(&cred.credential_id)),
                        (
                            "nemo-cred-secret".into(),
                            ids::to_hex(&cred.credential_secret),
                        ),
                    ],
                    body: vec![],
                },
            )
            .await?;
        Ok(check_body(&res)?.to_vec())
    }

    pub fn remember_group(&mut self, group: HostGroup) {
        self.groups.push(group);
    }

    fn group_origin(&self, group_id: [u8; KEY_LEN]) -> String {
        self.groups
            .iter()
            .find(|g| g.group_id == group_id)
            .map(|g| g.host_base.clone())
            .unwrap_or_default()
    }

    async fn call_group(&self, group_id: [u8; KEY_LEN], req: HttpRequest) -> Result<HttpResponse> {
        let origin = self.group_origin(group_id);
        if let Some(alt) = self.transport.redirect(&origin) {
            alt.call(req).await
        } else {
            self.transport.call(req).await
        }
    }

    fn owner_header(&self, cursor: u64, limit: u64) -> Result<String> {
        let auth = self.install.mailbox_owner_auth(cursor, limit, now_unix())?;
        Ok(ids::to_hex(&auth.encode()))
    }
}

impl HomeSession<HttpHome> {
    pub fn wakeup_handle(&self) -> Result<(HttpHome, String)> {
        Ok((self.transport.clone(), self.owner_header(self.cursor, 1)?))
    }

    /// Block until the home sends one wakeup frame. Payload MUST be empty (ADR-0020).
    pub async fn wait_wakeup(&self) -> Result<Vec<u8>> {
        let (home, auth) = self.wakeup_handle()?;
        home.wait_wakeup(&auth).await
    }
}

async fn fetch_bundle<T: HomeTransport>(transport: &T) -> Result<ServerBundle> {
    let res = transport
        .call(HttpRequest {
            method: "GET",
            path: "/v1/bundle".into(),
            headers: vec![],
            body: vec![],
        })
        .await?;
    Ok(ServerBundle::decode(check_body(&res)?)?)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}

fn check_empty(res: &HttpResponse) -> Result<()> {
    check_status(res).map(|_| ())
}

fn check_body(res: &HttpResponse) -> Result<&[u8]> {
    check_status(res)?;
    Ok(&res.body)
}

fn check_status(res: &HttpResponse) -> Result<()> {
    match res.status {
        200 | 204 => Ok(()),
        401 | 403 => Err(CoreError::Denied),
        409 => Err(CoreError::AlreadyRegistered),
        other => Err(CoreError::HomeHttp(other)),
    }
}

fn expect_map(bytes: &[u8]) -> Result<Vec<(u64, Value)>> {
    match cbor::decode(bytes)? {
        Value::Map(m) => Ok(m),
        _ => Err(CoreError::Wire(nemo_wire::WireError::Cbor("expected map"))),
    }
}

fn read_key(m: &[(u64, Value)], k: u64) -> Result<[u8; KEY_LEN]> {
    Ok(ids::copy_fixed(cbor::expect_bytes(cbor::map_get(m, k)?)?)?)
}

/// Outbound HTTPS/HTTP to a home server. TLS identity is the bundle, not Web PKI.
#[derive(Clone)]
pub struct HttpHome {
    client: reqwest::Client,
    tls: Arc<rustls::ClientConfig>,
    base: String,
}

impl HttpHome {
    pub fn new(base: impl Into<String>) -> Result<Self> {
        let base = base.into().trim_end_matches('/').to_string();
        let tls = tls_config_bundle_is_identity()?;
        // Caddy `tls internal` (and any other Web PKI) is transport, not identity
        // (ADR-0033). Clients authenticate the home via ServerBundle HPKE.
        let client = reqwest::Client::builder()
            .http1_only()
            .use_preconfigured_tls(tls.as_ref().clone())
            .build()
            .map_err(|e| CoreError::Transport(e.to_string()))?;
        Ok(Self { client, tls, base })
    }

    /// Long-lived `/v1/wakeup`. Returns the next binary frame (must be empty).
    pub async fn wait_wakeup(&self, owner_header: &str) -> Result<Vec<u8>> {
        use futures::StreamExt;
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        use tokio_tungstenite::tungstenite::Message as WsMsg;
        use tokio_tungstenite::{connect_async, connect_async_tls_with_config, Connector};

        let ws_url = wakeup_url(&self.base);
        let use_tls = ws_url.starts_with("wss://");
        let mut req = ws_url
            .into_client_request()
            .map_err(|e| CoreError::Transport(e.to_string()))?;
        req.headers_mut().insert(
            "nemo-owner",
            owner_header
                .parse()
                .map_err(|e| CoreError::Transport(format!("{e}")))?,
        );
        let (mut ws, _) = if use_tls {
            connect_async_tls_with_config(
                req,
                None,
                false,
                Some(Connector::Rustls(self.tls.clone())),
            )
            .await
        } else {
            connect_async(req).await
        }
        .map_err(|e| CoreError::Transport(e.to_string()))?;
        loop {
            match ws.next().await {
                Some(Ok(WsMsg::Binary(b))) => {
                    if !b.is_empty() {
                        return Err(CoreError::Transport("wakeup frame must be empty".into()));
                    }
                    return Ok(b.to_vec());
                }
                Some(Ok(WsMsg::Ping(_) | WsMsg::Pong(_) | WsMsg::Frame(_))) => {}
                Some(Ok(WsMsg::Close(_))) | None => {
                    return Err(CoreError::Transport("wakeup closed".into()));
                }
                Some(Err(e)) => return Err(CoreError::Transport(e.to_string())),
                Some(Ok(WsMsg::Text(_))) => {
                    return Err(CoreError::Transport("wakeup must be binary".into()));
                }
            }
        }
    }
}

/// HTTP origin for a peer home. Same `server_id` stays on this session's
/// transport (`own_base`, empty in-process). Otherwise `NEMO_HOST_ORIGINS`
/// (`host=http://127.0.0.1:1,other=http://…`) or `{http|https}://{host}`.
pub fn peer_http_origin(
    peer_host: &str,
    peer_server: ServerId,
    own_host: &str,
    own_server: ServerId,
    own_base: &str,
) -> String {
    if peer_server == own_server {
        return own_base.trim_end_matches('/').to_string();
    }
    if let Some(over) = host_origin_override(peer_host) {
        return over;
    }
    let _ = own_host;
    let scheme = if own_base.starts_with("http://") {
        "http"
    } else {
        "https"
    };
    format!("{scheme}://{}", peer_host.trim_end_matches('/'))
}

fn host_origin_override(host: &str) -> Option<String> {
    let raw = std::env::var("NEMO_HOST_ORIGINS").ok()?;
    for part in raw.split(',') {
        let (h, url) = part.split_once('=')?;
        if h.trim() == host {
            return Some(url.trim().trim_end_matches('/').to_string());
        }
    }
    None
}

pub(crate) fn wakeup_url(base: &str) -> String {
    if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}/v1/wakeup")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}/v1/wakeup")
    } else {
        format!("{base}/v1/wakeup")
    }
}

/// Web PKI is not server identity (ADR-0002 / ADR-0033). Accept Caddy `tls internal`.
fn tls_config_bundle_is_identity() -> Result<Arc<rustls::ClientConfig>> {
    let provider = rustls::crypto::ring::default_provider();
    let cfg = rustls::ClientConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .map_err(|e| CoreError::Transport(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(WebPkiIsNotIdentity))
        .with_no_client_auth();
    Ok(Arc::new(cfg))
}

#[derive(Debug)]
struct WebPkiIsNotIdentity;

impl rustls::client::danger::ServerCertVerifier for WebPkiIsNotIdentity {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

impl HomeTransport for HttpHome {
    fn redirect(&self, origin: &str) -> Option<Self> {
        let origin = origin.trim_end_matches('/');
        if origin.is_empty() {
            return None;
        }
        HttpHome::new(origin).ok()
    }

    async fn call(&self, req: HttpRequest) -> Result<HttpResponse> {
        let method = req
            .method
            .parse::<reqwest::Method>()
            .map_err(|e| CoreError::Transport(e.to_string()))?;
        let url = format!("{}{}", self.base, req.path);
        let mut builder = self.client.request(method, url);
        for (k, v) in &req.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        if req.method == "POST" {
            builder = builder.header("content-type", "application/cbor");
        }
        let res = builder
            .body(req.body)
            .send()
            .await
            .map_err(|e| CoreError::Transport(e.to_string()))?;
        let status = res.status().as_u16();
        let body = res
            .bytes()
            .await
            .map_err(|e| CoreError::Transport(e.to_string()))?
            .to_vec();
        Ok(HttpResponse { status, body })
    }
}

fn privacy_to_u64(mode: PrivacyMode) -> u64 {
    match mode {
        PrivacyMode::Normal => 0,
        PrivacyMode::Private => 1,
        PrivacyMode::High => 2,
    }
}

fn privacy_from_u64(v: u64) -> Result<PrivacyMode> {
    match v {
        0 => Ok(PrivacyMode::Normal),
        1 => Ok(PrivacyMode::Private),
        2 => Ok(PrivacyMode::High),
        _ => Err(CoreError::VaultCorrupt),
    }
}

#[cfg(test)]
mod wakeup_url_tests {
    use super::{peer_http_origin, wakeup_url};

    #[test]
    fn https_home_wakeup_is_wss() {
        assert_eq!(
            wakeup_url("https://localhost:8443"),
            "wss://localhost:8443/v1/wakeup"
        );
        assert_eq!(
            wakeup_url("http://127.0.0.1:8787"),
            "ws://127.0.0.1:8787/v1/wakeup"
        );
    }

    #[test]
    fn peer_origin_stays_on_own_home_when_server_matches() {
        let sid = [1u8; 32];
        assert_eq!(
            peer_http_origin("other.example", sid, "local", sid, "http://127.0.0.1:9"),
            "http://127.0.0.1:9"
        );
    }

    #[test]
    fn peer_origin_uses_https_host_when_homes_differ() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        assert_eq!(
            peer_http_origin(
                "bob.example",
                b,
                "alice.example",
                a,
                "https://alice.example"
            ),
            "https://bob.example"
        );
        assert_eq!(
            peer_http_origin("home-b", b, "home-a", a, "http://home-a"),
            "http://home-b"
        );
    }
}
