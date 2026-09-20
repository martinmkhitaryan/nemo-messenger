use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{SigningKey, VerifyingKey};

use crate::cbor::{self, Value};
use crate::error::{Result, WireError};
use crate::ids::{self, copy_fixed, IdentityId, ServerId, KEY_LEN, SIG_LEN};
use crate::sign;
use crate::PROTOCOL_VERSION;

pub const CARD_MAX_BYTES: usize = 400;
pub const HOST_MAX_BYTES: usize = 64;
pub const CLOCK_SKEW_SECS: u64 = 300;
pub const URI_PREFIX: &str = "nemo:1:";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomeServerBinding {
    pub server_id: ServerId,
    pub server_hpke_public_key: [u8; KEY_LEN],
    pub host: String,
    pub seq: u64,
    pub expires_at: u64,
    pub signature: [u8; SIG_LEN],
}

impl HomeServerBinding {
    fn unsigned_cbor(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Bytes(self.server_id.to_vec())),
            (1, Value::Bytes(self.server_hpke_public_key.to_vec())),
            (2, Value::Text(self.host.clone())),
            (3, Value::Uint(self.seq)),
            (4, Value::Uint(self.expires_at)),
        ]))
    }

    pub fn sign(sk: &SigningKey, mut binding: HomeServerBinding) -> Result<HomeServerBinding> {
        validate_host(&binding.host)?;
        if binding.server_id != ids::server_id(&binding.server_hpke_public_key) {
            return Err(WireError::ServerIdMismatch);
        }
        binding.signature = sign::sign(sk, sign::HOME_SERVER_BINDING, &binding.unsigned_cbor())?;
        Ok(binding)
    }

    pub fn verify(&self, identity_pk: &VerifyingKey, now_unix: u64) -> Result<()> {
        validate_host(&self.host)?;
        if self.server_id != ids::server_id(&self.server_hpke_public_key) {
            return Err(WireError::ServerIdMismatch);
        }
        if self.expires_at + CLOCK_SKEW_SECS < now_unix {
            return Err(WireError::BindingExpired);
        }
        sign::verify(
            identity_pk,
            sign::HOME_SERVER_BINDING,
            &self.unsigned_cbor(),
            &self.signature,
        )
    }

    pub fn to_cbor_value(&self) -> Value {
        Value::Map(vec![
            (0, Value::Bytes(self.server_id.to_vec())),
            (1, Value::Bytes(self.server_hpke_public_key.to_vec())),
            (2, Value::Text(self.host.clone())),
            (3, Value::Uint(self.seq)),
            (4, Value::Uint(self.expires_at)),
            (5, Value::Bytes(self.signature.to_vec())),
        ])
    }

    pub fn from_cbor_value(v: &Value) -> Result<Self> {
        let m = cbor::expect_map(v)?;
        Ok(Self {
            server_id: copy_fixed(cbor::expect_bytes(cbor::map_get(m, 0)?)?)?,
            server_hpke_public_key: copy_fixed(cbor::expect_bytes(cbor::map_get(m, 1)?)?)?,
            host: cbor::expect_text(cbor::map_get(m, 2)?)?.to_owned(),
            seq: cbor::expect_uint(cbor::map_get(m, 3)?)?,
            expires_at: cbor::expect_uint(cbor::map_get(m, 4)?)?,
            signature: copy_fixed(cbor::expect_bytes(cbor::map_get(m, 5)?)?)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactCard {
    pub identity_public_key: [u8; KEY_LEN],
    pub revocation_public_key: [u8; KEY_LEN],
    pub share_token: [u8; KEY_LEN],
    pub binding: HomeServerBinding,
}

impl ContactCard {
    pub fn identity_id(&self) -> IdentityId {
        ids::identity_id(&self.identity_public_key)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let bytes = cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.identity_public_key.to_vec())),
            (2, Value::Bytes(self.revocation_public_key.to_vec())),
            (3, Value::Bytes(self.share_token.to_vec())),
            (4, self.binding.to_cbor_value()),
        ]));
        if bytes.len() > CARD_MAX_BYTES {
            return Err(WireError::CardTooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > CARD_MAX_BYTES {
            return Err(WireError::CardTooLarge);
        }
        let Value::Map(m) = cbor::decode(bytes)? else {
            return Err(WireError::Cbor("card must be a map"));
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
        if version != PROTOCOL_VERSION as u64 {
            return Err(WireError::UnknownVersion(version));
        }
        Ok(Self {
            identity_public_key: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?,
            revocation_public_key: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 2)?)?)?,
            share_token: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 3)?)?)?,
            binding: HomeServerBinding::from_cbor_value(cbor::map_get(&m, 4)?)?,
        })
    }

    pub fn to_uri(&self) -> Result<String> {
        Ok(format!(
            "{URI_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(self.encode()?)
        ))
    }

    pub fn from_uri(uri: &str) -> Result<Self> {
        let rest = uri
            .strip_prefix(URI_PREFIX)
            .ok_or(WireError::Uri("missing nemo:1: prefix"))?;
        let bytes = URL_SAFE_NO_PAD
            .decode(rest)
            .map_err(|_| WireError::Uri("base64url"))?;
        Self::decode(&bytes)
    }

    /// Verify binding signature, server_id, host, and expiry. Does not consume the share token.
    pub fn verify(&self, now_unix: u64) -> Result<()> {
        let pk = VerifyingKey::from_bytes(&self.identity_public_key)
            .map_err(|_| WireError::Signature)?;
        self.binding.verify(&pk, now_unix)
    }
}

fn validate_host(host: &str) -> Result<()> {
    if host.is_empty() || host.len() > HOST_MAX_BYTES {
        return Err(WireError::BadHost);
    }
    if host.contains('/') || host.contains(':') || host.contains("://") {
        return Err(WireError::BadHost);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    fn sample_card(now: u64) -> (SigningKey, ContactCard) {
        let sk = SigningKey::generate(&mut OsRng);
        let rev = SigningKey::generate(&mut OsRng);
        let hpke = [9u8; 32];
        let binding = HomeServerBinding::sign(
            &sk,
            HomeServerBinding {
                server_id: ids::server_id(&hpke),
                server_hpke_public_key: hpke,
                host: "nemo.example".into(),
                seq: 1,
                expires_at: now + 3600,
                signature: [0; 64],
            },
        )
        .unwrap();
        let card = ContactCard {
            identity_public_key: sk.verifying_key().to_bytes(),
            revocation_public_key: rev.verifying_key().to_bytes(),
            share_token: [3u8; 32],
            binding,
        };
        (sk, card)
    }

    #[test]
    fn card_roundtrip_and_uri() {
        let now = 1_700_000_000;
        let (_sk, card) = sample_card(now);
        card.verify(now).unwrap();
        let bytes = card.encode().unwrap();
        assert!(bytes.len() <= CARD_MAX_BYTES);
        assert_eq!(ContactCard::decode(&bytes).unwrap(), card);
        let uri = card.to_uri().unwrap();
        assert!(uri.starts_with(URI_PREFIX));
        assert_eq!(ContactCard::from_uri(&uri).unwrap(), card);
    }

    #[test]
    fn rejects_server_id_mismatch() {
        let now = 1_700_000_000;
        let sk = SigningKey::generate(&mut OsRng);
        let hpke = [9u8; 32];
        let err = HomeServerBinding::sign(
            &sk,
            HomeServerBinding {
                server_id: [1u8; 32],
                server_hpke_public_key: hpke,
                host: "x.example".into(),
                seq: 1,
                expires_at: now + 10,
                signature: [0; 64],
            },
        )
        .unwrap_err();
        assert!(matches!(err, WireError::ServerIdMismatch));
    }

    #[test]
    fn rejects_expired() {
        let now = 1_700_000_000;
        let (_sk, card) = sample_card(now);
        assert!(card.verify(now + 3600 + CLOCK_SKEW_SECS + 1).is_err());
    }
}
