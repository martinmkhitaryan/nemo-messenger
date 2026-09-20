//! Client-to-home HTTP (phase 8). The server crate is not a runtime dependency.

use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};

use nemo_wire::cbor::{self, Value};
use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope};
use nemo_wire::ids::{self, IdentityId, KEY_LEN};
use nemo_wire::prekey::SignedPrekey;
use nemo_wire::{ContactCard, DiscoveryRecord, GroupAdmit, GroupInvite, ServerBundle};

use crate::discovery::Discovery;
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

pub struct HomeSession<T> {
    transport: T,
    pub install: Installation,
    pub bundle: ServerBundle,
    pub cursor: u64,
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
            },
            card,
        ))
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
