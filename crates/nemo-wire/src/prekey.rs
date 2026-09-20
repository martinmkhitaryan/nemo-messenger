use ed25519_dalek::{SigningKey, VerifyingKey};

use crate::cbor::{self, Value};
use crate::error::{Result, WireError};
use crate::ids::{copy_fixed, KEY_LEN, SIG_LEN};
use crate::sign;
use crate::PROTOCOL_VERSION;

/// Opaque signed prekey blob stored by discovery. The server MUST NOT parse `libsignal_prekey`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedPrekey {
    pub identity_public_key: [u8; KEY_LEN],
    pub prekey_id: u64,
    pub libsignal_prekey: Vec<u8>,
    pub signature: [u8; SIG_LEN],
}

impl SignedPrekey {
    fn unsigned_cbor(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.identity_public_key.to_vec())),
            (2, Value::Uint(self.prekey_id)),
            (3, Value::Bytes(self.libsignal_prekey.clone())),
        ]))
    }

    pub fn sign(
        identity_sk: &SigningKey,
        prekey_id: u64,
        libsignal_prekey: Vec<u8>,
    ) -> Result<Self> {
        let mut blob = Self {
            identity_public_key: identity_sk.verifying_key().to_bytes(),
            prekey_id,
            libsignal_prekey,
            signature: [0; 64],
        };
        blob.signature = sign::sign(identity_sk, sign::PREKEY, &blob.unsigned_cbor())?;
        Ok(blob)
    }

    pub fn verify(&self, pinned_identity: &VerifyingKey) -> Result<()> {
        if self.identity_public_key != pinned_identity.to_bytes() {
            return Err(WireError::Signature);
        }
        sign::verify(
            pinned_identity,
            sign::PREKEY,
            &self.unsigned_cbor(),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.identity_public_key.to_vec())),
            (2, Value::Uint(self.prekey_id)),
            (3, Value::Bytes(self.libsignal_prekey.clone())),
            (4, Value::Bytes(self.signature.to_vec())),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes)? else {
            return Err(WireError::Cbor("prekey must be a map"));
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
        if version != PROTOCOL_VERSION as u64 {
            return Err(WireError::UnknownVersion(version));
        }
        Ok(Self {
            identity_public_key: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?,
            prekey_id: cbor::expect_uint(cbor::map_get(&m, 2)?)?,
            libsignal_prekey: cbor::expect_bytes(cbor::map_get(&m, 3)?)?.to_vec(),
            signature: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 4)?)?)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn prekey_blob_is_opaque() {
        let sk = SigningKey::generate(&mut OsRng);
        let blob = SignedPrekey::sign(&sk, 7, vec![0xde, 0xad]).unwrap();
        blob.verify(&sk.verifying_key()).unwrap();
        let other = SigningKey::generate(&mut OsRng);
        assert!(blob.verify(&other.verifying_key()).is_err());
        assert_eq!(SignedPrekey::decode(&blob.encode()).unwrap(), blob);
    }
}
