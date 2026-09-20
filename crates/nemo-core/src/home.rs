//! Client-to-home HTTP (phase 8). The server crate is not a runtime dependency.

use std::collections::HashMap;
use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};

use nemo_wire::cbor::{self, Value};
use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope, TtlBucket};
use nemo_wire::ids::{self, IdentityId, KEY_LEN};
use nemo_wire::prekey::SignedPrekey;
use nemo_wire::{
    ContactCard, DiscoveryRecord, GroupAdmit, GroupInvite, RevocationStatement, ServerBundle,
};

use crate::discovery::{resolve_contact, ContactPin, Discovery, DISCOVERY_REFRESH_SECS};
use crate::error::{CoreError, Result};
use crate::identity::Installation;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostCred {
    pub credential_id: [u8; KEY_LEN],
    pub credential_secret: [u8; KEY_LEN],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostGroup {
    pub group_id: [u8; KEY_LEN],
    pub cred: HostCred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostAccept {
    pub pending_id: [u8; KEY_LEN],
    pub cred: HostCred,
    pub invitee_binding: Option<[u8; KEY_LEN]>,
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
            if row.len() != 3 {
                return Err(CoreError::VaultCorrupt);
            }
            groups.push(HostGroup {
                group_id: ids::copy_fixed(cbor::expect_bytes(&row[0])?)?,
                cred: HostCred {
                    credential_id: ids::copy_fixed(cbor::expect_bytes(&row[1])?)?,
                    credential_secret: ids::copy_fixed(cbor::expect_bytes(&row[2])?)?,
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
            },
            card,
        ))
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
        }
    }

    pub fn snapshot(&self) -> HomeState {
        HomeState {
            bundle: self.bundle.clone(),
            cursor: self.cursor,
            contacts: self.contacts.clone(),
            groups: self.groups.clone(),
            home_base: self.home_base.clone(),
        }
    }

    pub fn set_home_base(&mut self, base: impl Into<String>) {
        self.home_base = base.into().trim_end_matches('/').to_string();
    }

    /// Fetch the current binding, start PQXDH, and remember the delivery address.
    pub async fn add_contact(
        &mut self,
        card: &ContactCard,
        delivery_capability: [u8; KEY_LEN],
        now_unix: u64,
    ) -> Result<()> {
        let disc = self.discovery(card.identity_id()).await?;
        let binding = resolve_contact(card, &disc, now_unix)?;
        let prekey = self.fetch_prekey(card.share_token).await?;
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
            },
        );
        Ok(())
    }

    pub async fn refresh_contact(&mut self, peer: &IdentityId, now_unix: u64) -> Result<()> {
        let disc = self.discovery(*peer).await?;
        disc.verify(now_unix)?;
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
        contact.last_discovery_unix = now_unix;
        Ok(())
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
        let res = self
            .transport
            .call(HttpRequest {
                method: "GET",
                path: format!("/v1/discovery/{}", ids::to_hex(&identity_id)),
                headers: vec![],
                body: vec![],
            })
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
        let res = self
            .transport
            .call(HttpRequest {
                method: "GET",
                path: "/v1/prekeys".into(),
                headers: vec![("nemo-token".into(), ids::to_hex(&share_token))],
                body: vec![],
            })
            .await?;
        Ok(SignedPrekey::decode(check_body(&res)?)?)
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
            .transport
            .call(HttpRequest {
                method: "POST",
                path: format!("/v1/groups/{}/append", ids::to_hex(&group_id)),
                headers: vec![],
                body: payload,
            })
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
            .transport
            .call(HttpRequest {
                method: "POST",
                path: format!("/v1/groups/{}/invites", ids::to_hex(&group_id)),
                headers: vec![],
                body,
            })
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
            .transport
            .call(HttpRequest {
                method: "POST",
                path: format!("/v1/groups/{}/admit", ids::to_hex(&group_id)),
                headers: vec![],
                body,
            })
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
        let body = cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(cred.credential_id.to_vec())),
            (1, Value::Bytes(cred.credential_secret.to_vec())),
            (2, Value::Bytes(delivery_capability.to_vec())),
            (3, Value::Bytes(home_hpke_public.to_vec())),
        ]));
        let res = self
            .transport
            .call(HttpRequest {
                method: "POST",
                path: format!("/v1/groups/{}/fanout", ids::to_hex(&group_id)),
                headers: vec![],
                body,
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
            .transport
            .call(HttpRequest {
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
            })
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
            .transport
            .call(HttpRequest {
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
            })
            .await?;
        Ok(check_body(&res)?.to_vec())
    }

    pub fn remember_group(&mut self, group: HostGroup) {
        self.groups.push(group);
    }

    fn owner_header(&self, cursor: u64, limit: u64) -> Result<String> {
        let auth = self.install.mailbox_owner_auth(cursor, limit, now_unix())?;
        Ok(ids::to_hex(&auth.encode()))
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
    base: String,
}

impl HttpHome {
    pub fn new(base: impl Into<String>) -> Result<Self> {
        let base = base.into().trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .http1_only()
            .build()
            .map_err(|e| CoreError::Transport(e.to_string()))?;
        Ok(Self { client, base })
    }
}

impl HomeTransport for HttpHome {
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
