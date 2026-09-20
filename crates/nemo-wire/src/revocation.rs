use ed25519_dalek::{SigningKey, VerifyingKey};

use crate::cbor::{self, Value};
use crate::error::{Result, WireError};
use crate::ids::{copy_fixed, IdentityId, KEY_LEN, SIG_LEN};
use crate::sign;
use crate::PROTOCOL_VERSION;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevocationStatement {
    pub identity_id: IdentityId,
    pub coarse_timestamp: u64,
    pub signature: [u8; SIG_LEN],
}

impl RevocationStatement {
    fn unsigned_cbor(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.identity_id.to_vec())),
            (2, Value::Text("revoked".into())),
            (3, Value::Uint(self.coarse_timestamp)),
        ]))
    }

    pub fn sign(
        revocation_sk: &SigningKey,
        identity_id: IdentityId,
        coarse_timestamp: u64,
    ) -> Result<Self> {
        let mut stmt = Self {
            identity_id,
            coarse_timestamp,
            signature: [0; 64],
        };
        stmt.signature = sign::sign(revocation_sk, sign::REVOCATION, &stmt.unsigned_cbor())?;
        Ok(stmt)
    }

    pub fn verify(&self, revocation_pk: &VerifyingKey) -> Result<()> {
        sign::verify(
            revocation_pk,
            sign::REVOCATION,
            &self.unsigned_cbor(),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.identity_id.to_vec())),
            (2, Value::Text("revoked".into())),
            (3, Value::Uint(self.coarse_timestamp)),
            (4, Value::Bytes(self.signature.to_vec())),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes)? else {
            return Err(WireError::Cbor("revocation must be a map"));
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
        if version != PROTOCOL_VERSION as u64 {
            return Err(WireError::UnknownVersion(version));
        }
        let status = cbor::expect_text(cbor::map_get(&m, 2)?)?;
        if status != "revoked" {
            return Err(WireError::BadRevocationStatus);
        }
        Ok(Self {
            identity_id: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?,
            coarse_timestamp: cbor::expect_uint(cbor::map_get(&m, 3)?)?,
            signature: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 4)?)?)?,
        })
    }

    pub fn verifying_key(revocation_public_key: &[u8; KEY_LEN]) -> Result<VerifyingKey> {
        VerifyingKey::from_bytes(revocation_public_key).map_err(|_| WireError::Signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::identity_id;
    use rand::rngs::OsRng;

    #[test]
    fn revocation_roundtrip() {
        let identity_sk = SigningKey::generate(&mut OsRng);
        let rev_sk = SigningKey::generate(&mut OsRng);
        let id = identity_id(&identity_sk.verifying_key().to_bytes());
        let stmt = RevocationStatement::sign(&rev_sk, id, 1_700_000_000).unwrap();
        stmt.verify(&rev_sk.verifying_key()).unwrap();
        let bytes = stmt.encode();
        let decoded = RevocationStatement::decode(&bytes).unwrap();
        decoded.verify(&rev_sk.verifying_key()).unwrap();
        assert!(decoded.verify(&identity_sk.verifying_key()).is_err());
    }
}
