//! Host-framed group-stream bodies and federation/owner encodings.
//!
//! The host MUST NOT parse `mls_commit` inside [`RemoveBundle`].

use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::Rng;

use crate::cbor::{self, Value};
use crate::envelope::TtlBucket;
use crate::error::{Result, WireError};
use crate::ids::{self, copy_fixed, ServerId, KEY_LEN, SIG_LEN};
use crate::sign;
use crate::PROTOCOL_VERSION;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentSizeBucket {
    A1 = 1,
    A2 = 2,
    A3 = 3,
    A4 = 4,
}

impl AttachmentSizeBucket {
    pub fn from_u8(v: u8) -> Result<Self> {
        Ok(match v {
            1 => Self::A1,
            2 => Self::A2,
            3 => Self::A3,
            4 => Self::A4,
            _ => return Err(WireError::BadEnvelope),
        })
    }

    pub fn inner_len(self) -> usize {
        match self {
            Self::A1 => crate::envelope::A1,
            Self::A2 => crate::envelope::A2,
            Self::A3 => crate::envelope::A3,
            Self::A4 => crate::envelope::A4,
        }
    }

    pub fn for_len(n: usize) -> Result<Self> {
        for b in [Self::A1, Self::A2, Self::A3, Self::A4] {
            if n <= b.inner_len() {
                return Ok(b);
            }
        }
        Err(WireError::NoBucket)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveBundle {
    pub credential_id: [u8; KEY_LEN],
    pub sidecar_signature: [u8; SIG_LEN],
    pub mls_commit: Vec<u8>,
}

impl RemoveBundle {
    /// Sidecar binds the named credential to **this** group and **this**
    /// Commit so a captured sidecar cannot be re-appended with other bytes.
    fn sidecar_payload(
        group_id: &[u8; KEY_LEN],
        credential_id: &[u8; KEY_LEN],
        mls_commit: &[u8],
    ) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(credential_id.to_vec())),
            (1, Value::Text("revoke".into())),
            (2, Value::Bytes(group_id.to_vec())),
            (3, Value::Bytes(ids::sha256(mls_commit).to_vec())),
        ]))
    }

    pub fn sign(
        appender_sk: &SigningKey,
        group_id: [u8; KEY_LEN],
        credential_id: [u8; KEY_LEN],
        mls_commit: Vec<u8>,
    ) -> Result<Self> {
        let sidecar_signature = sign::sign(
            appender_sk,
            sign::REMOVE_SIDECAR,
            &Self::sidecar_payload(&group_id, &credential_id, &mls_commit),
        )?;
        Ok(Self {
            credential_id,
            sidecar_signature,
            mls_commit,
        })
    }

    pub fn verify_sidecar(&self, appender_pk: &VerifyingKey, group_id: &[u8; KEY_LEN]) -> Result<()> {
        sign::verify(
            appender_pk,
            sign::REMOVE_SIDECAR,
            &Self::sidecar_payload(group_id, &self.credential_id, &self.mls_commit),
            &self.sidecar_signature,
        )
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let n = u32::try_from(self.mls_commit.len()).map_err(|_| WireError::NoBucket)?;
        let mut out = Vec::with_capacity(KEY_LEN + SIG_LEN + 4 + self.mls_commit.len());
        out.extend_from_slice(&self.credential_id);
        out.extend_from_slice(&self.sidecar_signature);
        out.extend_from_slice(&n.to_be_bytes());
        out.extend_from_slice(&self.mls_commit);
        Ok(out)
    }

    /// Parses from a padded body. Does not inspect `mls_commit`.
    pub fn decode(body: &[u8]) -> Result<Self> {
        const HDR: usize = KEY_LEN + SIG_LEN + 4;
        if body.len() < HDR {
            return Err(WireError::BadEnvelope);
        }
        let credential_id = copy_fixed(&body[..KEY_LEN])?;
        let sidecar_signature = copy_fixed(&body[KEY_LEN..KEY_LEN + SIG_LEN])?;
        let n = u32::from_be_bytes(body[KEY_LEN + SIG_LEN..HDR].try_into().unwrap()) as usize;
        if body.len() < HDR + n {
            return Err(WireError::BadEnvelope);
        }
        Ok(Self {
            credential_id,
            sidecar_signature,
            mls_commit: body[HDR..HDR + n].to_vec(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SigningKeyReplace {
    pub new_public_key: [u8; KEY_LEN],
    pub signature: [u8; SIG_LEN],
}

impl SigningKeyReplace {
    fn payload(new_public_key: &[u8; KEY_LEN]) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![(
            0,
            Value::Bytes(new_public_key.to_vec()),
        )]))
    }

    pub fn sign(current_sk: &SigningKey, new_public_key: [u8; KEY_LEN]) -> Result<Self> {
        let signature = sign::sign(
            current_sk,
            sign::SIGNING_KEY_REPLACE,
            &Self::payload(&new_public_key),
        )?;
        Ok(Self {
            new_public_key,
            signature,
        })
    }

    pub fn verify(&self, current_pk: &VerifyingKey) -> Result<()> {
        sign::verify(
            current_pk,
            sign::SIGNING_KEY_REPLACE,
            &Self::payload(&self.new_public_key),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(KEY_LEN + SIG_LEN);
        out.extend_from_slice(&self.new_public_key);
        out.extend_from_slice(&self.signature);
        out
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < KEY_LEN + SIG_LEN {
            return Err(WireError::BadEnvelope);
        }
        Ok(Self {
            new_public_key: copy_fixed(&body[..KEY_LEN])?,
            signature: copy_fixed(&body[KEY_LEN..KEY_LEN + SIG_LEN])?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentReserve {
    pub fetch_token: [u8; KEY_LEN],
    pub size_bucket: AttachmentSizeBucket,
    pub ttl_bucket: TtlBucket,
}

impl AttachmentReserve {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(KEY_LEN + 2);
        out.extend_from_slice(&self.fetch_token);
        out.push(self.size_bucket as u8);
        out.push(self.ttl_bucket.0);
        out
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < KEY_LEN + 2 {
            return Err(WireError::BadEnvelope);
        }
        Ok(Self {
            fetch_token: copy_fixed(&body[..KEY_LEN])?,
            size_bucket: AttachmentSizeBucket::from_u8(body[KEY_LEN])?,
            ttl_bucket: TtlBucket::from_u8(body[KEY_LEN + 1])?,
        })
    }
}

/// What an owner signature authorises: one request, to one server.
///
/// `body_hash` is `SHA-256(request body)` when the auth travels in a header,
/// and all zeros when the auth **is** the body (`/v1/mailbox/fetch`, `/ack`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthScope {
    pub server_id: ServerId,
    pub method: String,
    pub path: String,
    pub body_hash: [u8; KEY_LEN],
}

impl AuthScope {
    pub fn header(server_id: ServerId, method: &str, path: &str, body: &[u8]) -> Self {
        Self {
            server_id,
            method: method.to_owned(),
            path: path.to_owned(),
            body_hash: ids::sha256(body),
        }
    }

    pub fn body(server_id: ServerId, method: &str, path: &str) -> Self {
        Self {
            server_id,
            method: method.to_owned(),
            path: path.to_owned(),
            body_hash: [0; KEY_LEN],
        }
    }
}

/// Owner fetch/ack signature payload (phase 4). `ts` older than 120s MUST be
/// rejected by the server, and `nonce` MUST be rejected if seen before within
/// that window. The signature is bound to an [`AuthScope`] so a captured
/// header cannot be replayed against another route or another server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailboxOwnerAuth {
    pub mailbox_hint: Vec<u8>,
    pub cursor: u64,
    pub limit: u64,
    pub ts: u64,
    pub scope: AuthScope,
    pub nonce: [u8; KEY_LEN],
    pub signature: [u8; SIG_LEN],
}

impl MailboxOwnerAuth {
    pub const MAX_AGE_SECS: u64 = 120;

    fn unsigned_cbor(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(self.mailbox_hint.clone())),
            (1, Value::Uint(self.cursor)),
            (2, Value::Uint(self.limit)),
            (3, Value::Uint(self.ts)),
            (5, Value::Bytes(self.scope.server_id.to_vec())),
            (6, Value::Text(self.scope.method.clone())),
            (7, Value::Text(self.scope.path.clone())),
            (8, Value::Bytes(self.scope.body_hash.to_vec())),
            (9, Value::Bytes(self.nonce.to_vec())),
        ]))
    }

    pub fn sign(
        identity_sk: &SigningKey,
        mailbox_hint: Vec<u8>,
        cursor: u64,
        limit: u64,
        ts: u64,
        scope: AuthScope,
    ) -> Result<Self> {
        let mut nonce = [0u8; KEY_LEN];
        rand::rng().fill_bytes(&mut nonce);
        let mut auth = Self {
            mailbox_hint,
            cursor,
            limit,
            ts,
            scope,
            nonce,
            signature: [0; 64],
        };
        auth.signature = sign::sign(identity_sk, sign::MAILBOX_OWNER, &auth.unsigned_cbor())?;
        Ok(auth)
    }

    /// Verify freshness, scope, and signature. Replay (nonce) is the caller's.
    pub fn verify(&self, identity_pk: &VerifyingKey, now_unix: u64, expect: &AuthScope) -> Result<()> {
        let age = now_unix.saturating_sub(self.ts);
        if age > Self::MAX_AGE_SECS || self.ts > now_unix + Self::MAX_AGE_SECS {
            return Err(WireError::BindingExpired);
        }
        if self.scope != *expect {
            return Err(WireError::Signature);
        }
        sign::verify(
            identity_pk,
            sign::MAILBOX_OWNER,
            &self.unsigned_cbor(),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(self.mailbox_hint.clone())),
            (1, Value::Uint(self.cursor)),
            (2, Value::Uint(self.limit)),
            (3, Value::Uint(self.ts)),
            (4, Value::Bytes(self.signature.to_vec())),
            (5, Value::Bytes(self.scope.server_id.to_vec())),
            (6, Value::Text(self.scope.method.clone())),
            (7, Value::Text(self.scope.path.clone())),
            (8, Value::Bytes(self.scope.body_hash.to_vec())),
            (9, Value::Bytes(self.nonce.to_vec())),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes)? else {
            return Err(WireError::Cbor("owner auth must be a map"));
        };
        Ok(Self {
            mailbox_hint: cbor::expect_bytes(cbor::map_get(&m, 0)?)?.to_vec(),
            cursor: cbor::expect_uint(cbor::map_get(&m, 1)?)?,
            limit: cbor::expect_uint(cbor::map_get(&m, 2)?)?,
            ts: cbor::expect_uint(cbor::map_get(&m, 3)?)?,
            signature: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 4)?)?)?,
            scope: AuthScope {
                server_id: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 5)?)?)?,
                method: cbor::expect_text(cbor::map_get(&m, 6)?)?.to_owned(),
                path: cbor::expect_text(cbor::map_get(&m, 7)?)?.to_owned(),
                body_hash: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 8)?)?)?,
            },
            nonce: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 9)?)?)?,
        })
    }
}

/// Home-server signature over one HTTP response, bound to the client's
/// per-request `nemo-nonce`. Lets the client authenticate the home through
/// any TLS terminator (ADR-0033: Web PKI is not identity).
pub const RESPONSE_NONCE_HEADER: &str = "nemo-nonce";
pub const RESPONSE_SIG_HEADER: &str = "nemo-server-sig";

fn response_payload(nonce: &[u8; KEY_LEN], status: u16, body: &[u8]) -> Vec<u8> {
    cbor::encode(&Value::Map(vec![
        (0, Value::Uint(PROTOCOL_VERSION as u64)),
        (1, Value::Bytes(nonce.to_vec())),
        (2, Value::Uint(status as u64)),
        (3, Value::Bytes(ids::sha256(body).to_vec())),
    ]))
}

pub fn sign_response(
    server_sk: &SigningKey,
    nonce: &[u8; KEY_LEN],
    status: u16,
    body: &[u8],
) -> Result<[u8; SIG_LEN]> {
    sign::sign(server_sk, sign::SERVER_RESPONSE, &response_payload(nonce, status, body))
}

pub fn verify_response(
    server_pk: &VerifyingKey,
    nonce: &[u8; KEY_LEN],
    status: u16,
    body: &[u8],
    signature: &[u8; SIG_LEN],
) -> Result<()> {
    sign::verify(
        server_pk,
        sign::SERVER_RESPONSE,
        &response_payload(nonce, status, body),
        signature,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerBundle {
    pub server_id: ServerId,
    pub server_hpke_public_key: [u8; KEY_LEN],
    pub server_sign_public_key: [u8; KEY_LEN],
    pub host: String,
    pub s2s_port: u64,
    pub signature: [u8; SIG_LEN],
}

impl ServerBundle {
    fn unsigned_cbor(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.server_id.to_vec())),
            (2, Value::Bytes(self.server_hpke_public_key.to_vec())),
            (3, Value::Bytes(self.server_sign_public_key.to_vec())),
            (4, Value::Text(self.host.clone())),
            (5, Value::Uint(self.s2s_port)),
        ]))
    }

    pub fn sign(server_sk: &SigningKey, mut bundle: ServerBundle) -> Result<ServerBundle> {
        if bundle.server_id != ids::server_id(&bundle.server_hpke_public_key) {
            return Err(WireError::ServerIdMismatch);
        }
        if bundle.server_sign_public_key != server_sk.verifying_key().to_bytes() {
            return Err(WireError::Signature);
        }
        bundle.signature = sign::sign(server_sk, sign::SERVER_BUNDLE, &bundle.unsigned_cbor())?;
        Ok(bundle)
    }

    pub fn verify(&self) -> Result<()> {
        if self.server_id != ids::server_id(&self.server_hpke_public_key) {
            return Err(WireError::ServerIdMismatch);
        }
        let pk = VerifyingKey::from_bytes(&self.server_sign_public_key)
            .map_err(|_| WireError::Signature)?;
        sign::verify(
            &pk,
            sign::SERVER_BUNDLE,
            &self.unsigned_cbor(),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.server_id.to_vec())),
            (2, Value::Bytes(self.server_hpke_public_key.to_vec())),
            (3, Value::Bytes(self.server_sign_public_key.to_vec())),
            (4, Value::Text(self.host.clone())),
            (5, Value::Uint(self.s2s_port)),
            (6, Value::Bytes(self.signature.to_vec())),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes)? else {
            return Err(WireError::Cbor("server bundle must be a map"));
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
        if version != PROTOCOL_VERSION as u64 {
            return Err(WireError::UnknownVersion(version));
        }
        Ok(Self {
            server_id: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?,
            server_hpke_public_key: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 2)?)?)?,
            server_sign_public_key: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 3)?)?)?,
            host: cbor::expect_text(cbor::map_get(&m, 4)?)?.to_owned(),
            s2s_port: cbor::expect_uint(cbor::map_get(&m, 5)?)?,
            signature: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 6)?)?)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerSignRotate {
    pub new_public_key: [u8; KEY_LEN],
    pub seq: u64,
    pub signature: [u8; SIG_LEN],
}

impl ServerSignRotate {
    fn unsigned_cbor(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(self.new_public_key.to_vec())),
            (1, Value::Uint(self.seq)),
        ]))
    }

    pub fn sign(current_sk: &SigningKey, new_public_key: [u8; KEY_LEN], seq: u64) -> Result<Self> {
        let mut stmt = Self {
            new_public_key,
            seq,
            signature: [0; 64],
        };
        stmt.signature = sign::sign(current_sk, sign::SERVER_SIGN_ROTATE, &stmt.unsigned_cbor())?;
        Ok(stmt)
    }

    pub fn verify(&self, current_pk: &VerifyingKey) -> Result<()> {
        sign::verify(
            current_pk,
            sign::SERVER_SIGN_ROTATE,
            &self.unsigned_cbor(),
            &self.signature,
        )
    }
}

/// TLS application-data frame after mTLS (phase 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct S2sFrame {
    pub counter: u64,
    pub payload: Vec<u8>,
}

impl S2sFrame {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let n = u32::try_from(self.payload.len()).map_err(|_| WireError::NoBucket)?;
        let mut out = Vec::with_capacity(8 + 4 + self.payload.len());
        out.extend_from_slice(&self.counter.to_be_bytes());
        out.extend_from_slice(&n.to_be_bytes());
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 12 {
            return Err(WireError::BadEnvelope);
        }
        let counter = u64::from_be_bytes(bytes[..8].try_into().unwrap());
        let n = u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let rest = &bytes[12..];
        if rest.len() != n {
            return Err(WireError::Length {
                expected: n,
                got: rest.len(),
            });
        }
        Ok(Self {
            counter,
            payload: rest.to_vec(),
        })
    }
}

pub fn hello_payload(sender: &ServerId, receiver: &ServerId) -> Vec<u8> {
    cbor::encode(&Value::Map(vec![
        (0, Value::Uint(PROTOCOL_VERSION as u64)),
        (1, Value::Bytes(sender.to_vec())),
        (2, Value::Bytes(receiver.to_vec())),
    ]))
}

pub fn ok_payload(seq: u64) -> Vec<u8> {
    cbor::encode(&Value::Map(vec![
        (0, Value::Text("ok".into())),
        (1, Value::Uint(seq)),
    ]))
}

pub fn fail_payload() -> Vec<u8> {
    cbor::encode(&Value::Map(vec![(0, Value::Text("fail".into()))]))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum S2sPayload {
    Hello {
        sender: ServerId,
        receiver: ServerId,
    },
    Ok {
        seq: u64,
    },
    Fail,
    Ciphertext(Vec<u8>),
}

pub fn parse_payload(bytes: &[u8]) -> Result<S2sPayload> {
    if let Ok(Value::Map(m)) = cbor::decode(bytes) {
        if let Ok(v) = cbor::map_get(&m, 0) {
            if let Ok(1) = cbor::expect_uint(v) {
                let (sender, receiver) = parse_hello(bytes)?;
                return Ok(S2sPayload::Hello { sender, receiver });
            }
            if let Ok(s) = cbor::expect_text(v) {
                return Ok(match s {
                    "ok" => S2sPayload::Ok {
                        seq: cbor::expect_uint(cbor::map_get(&m, 1)?)?,
                    },
                    "fail" => S2sPayload::Fail,
                    _ => S2sPayload::Ciphertext(bytes.to_vec()),
                });
            }
        }
    }
    Ok(S2sPayload::Ciphertext(bytes.to_vec()))
}

pub fn parse_hello(bytes: &[u8]) -> Result<(ServerId, ServerId)> {
    let Value::Map(m) = cbor::decode(bytes)? else {
        return Err(WireError::Cbor("hello must be a map"));
    };
    let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
    if version != PROTOCOL_VERSION as u64 {
        return Err(WireError::UnknownVersion(version));
    }
    Ok((
        copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?,
        copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 2)?)?)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{MessageType, PaddedMessage};
    use crate::hpke::HpkeKeypair;

    #[test]
    fn remove_bundle_roundtrip_and_opaque_commit() {
        let sk = SigningKey::generate(&mut rand::rng());
        let gid = [0x11; 32];
        let bundle = RemoveBundle::sign(&sk, gid, [0xab; 32], vec![0, 1, 0, 2]).unwrap();
        bundle.verify_sidecar(&sk.verifying_key(), &gid).unwrap();
        let body = bundle.encode().unwrap();
        let padded = PaddedMessage::pad(MessageType::RemoveBundle, body).unwrap();
        let mut decoded = RemoveBundle::decode(&padded.encode()[2..]).unwrap();
        assert_eq!(decoded.mls_commit, vec![0, 1, 0, 2]);
        decoded.verify_sidecar(&sk.verifying_key(), &gid).unwrap();
        // Bound to the group and to the commit bytes.
        assert!(decoded.verify_sidecar(&sk.verifying_key(), &[0x22; 32]).is_err());
        decoded.mls_commit = vec![9];
        assert!(decoded.verify_sidecar(&sk.verifying_key(), &gid).is_err());
    }

    #[test]
    fn signing_key_replace() {
        let current = SigningKey::generate(&mut rand::rng());
        let new = SigningKey::generate(&mut rand::rng());
        let msg = SigningKeyReplace::sign(&current, new.verifying_key().to_bytes()).unwrap();
        msg.verify(&current.verifying_key()).unwrap();
        assert!(msg.verify(&new.verifying_key()).is_err());
        let decoded = SigningKeyReplace::decode(&msg.encode()).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn attachment_reserve_no_filename() {
        let r = AttachmentReserve {
            fetch_token: [9; 32],
            size_bucket: AttachmentSizeBucket::A2,
            ttl_bucket: TtlBucket::DAY,
        };
        assert_eq!(AttachmentReserve::decode(&r.encode()).unwrap(), r);
    }

    #[test]
    fn server_bundle_matches_hpke_id() {
        let hpke = HpkeKeypair::generate();
        let sign_sk = SigningKey::generate(&mut rand::rng());
        let bundle = ServerBundle::sign(
            &sign_sk,
            ServerBundle {
                server_id: hpke.server_id(),
                server_hpke_public_key: hpke.public,
                server_sign_public_key: sign_sk.verifying_key().to_bytes(),
                host: "nemo.example".into(),
                s2s_port: 8443,
                signature: [0; 64],
            },
        )
        .unwrap();
        bundle.verify().unwrap();
        assert_eq!(ServerBundle::decode(&bundle.encode()).unwrap(), bundle);
    }

    #[test]
    fn s2s_hello_frame() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let payload = hello_payload(&a, &b);
        let frame = S2sFrame {
            counter: 1,
            payload: payload.clone(),
        };
        let decoded = S2sFrame::decode(&frame.encode().unwrap()).unwrap();
        assert_eq!(parse_hello(&decoded.payload).unwrap(), (a, b));
        assert_eq!(
            parse_payload(&payload).unwrap(),
            S2sPayload::Hello {
                sender: a,
                receiver: b
            }
        );
        assert_eq!(
            parse_payload(&ok_payload(7)).unwrap(),
            S2sPayload::Ok { seq: 7 }
        );
        assert_eq!(parse_payload(&fail_payload()).unwrap(), S2sPayload::Fail);
        assert!(matches!(
            parse_payload(&[0xde, 0xad]),
            Ok(S2sPayload::Ciphertext(_))
        ));
    }

    #[test]
    fn mailbox_owner_freshness() {
        let sk = SigningKey::generate(&mut rand::rng());
        let now = 1_700_000_000;
        let scope = AuthScope::body([7; 32], "POST", "/v1/mailbox/fetch");
        let auth = MailboxOwnerAuth::sign(&sk, vec![1, 2, 3], 0, 64, now, scope.clone()).unwrap();
        auth.verify(&sk.verifying_key(), now, &scope).unwrap();
        assert!(auth.verify(&sk.verifying_key(), now + 121, &scope).is_err());
        // Scope-bound: another route, server, or body does not verify.
        let other = AuthScope::body([7; 32], "POST", "/v1/mailbox/ack");
        assert!(auth.verify(&sk.verifying_key(), now, &other).is_err());
        let other_server = AuthScope::body([8; 32], "POST", "/v1/mailbox/fetch");
        assert!(auth.verify(&sk.verifying_key(), now, &other_server).is_err());
        let decoded = MailboxOwnerAuth::decode(&auth.encode()).unwrap();
        assert_eq!(decoded, auth);
    }

    #[test]
    fn response_signature_binds_nonce_status_body() {
        let sk = SigningKey::generate(&mut rand::rng());
        let nonce = [3u8; 32];
        let sig = sign_response(&sk, &nonce, 200, b"body").unwrap();
        verify_response(&sk.verifying_key(), &nonce, 200, b"body", &sig).unwrap();
        assert!(verify_response(&sk.verifying_key(), &[4u8; 32], 200, b"body", &sig).is_err());
        assert!(verify_response(&sk.verifying_key(), &nonce, 403, b"body", &sig).is_err());
        assert!(verify_response(&sk.verifying_key(), &nonce, 200, b"other", &sig).is_err());
    }
}
