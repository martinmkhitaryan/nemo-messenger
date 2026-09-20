//! Discovery row as returned by a home server (phase 8). Not a directory search.

use crate::card::HomeServerBinding;
use crate::cbor::{self, Value};
use crate::error::{Result, WireError};
use crate::ids::{copy_fixed, KEY_LEN};
use crate::revocation::RevocationStatement;
use crate::PROTOCOL_VERSION;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryRecord {
    pub identity_public_key: [u8; KEY_LEN],
    pub revocation_public_key: [u8; KEY_LEN],
    pub binding: HomeServerBinding,
    pub revocation: Option<RevocationStatement>,
}

impl DiscoveryRecord {
    pub fn encode(&self) -> Vec<u8> {
        let mut pairs = vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.identity_public_key.to_vec())),
            (2, Value::Bytes(self.revocation_public_key.to_vec())),
            (3, self.binding.to_cbor_value()),
        ];
        if let Some(stmt) = &self.revocation {
            pairs.push((4, Value::Bytes(stmt.encode())));
        }
        cbor::encode(&Value::Map(pairs))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes)? else {
            return Err(WireError::Cbor("discovery must be a map"));
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
        if version != PROTOCOL_VERSION as u64 {
            return Err(WireError::UnknownVersion(version));
        }
        let revocation = match cbor::map_get_opt(&m, 4) {
            Some(v) => Some(RevocationStatement::decode(cbor::expect_bytes(v)?)?),
            None => None,
        };
        Ok(Self {
            identity_public_key: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?,
            revocation_public_key: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 2)?)?)?,
            binding: HomeServerBinding::from_cbor_value(cbor::map_get(&m, 3)?)?,
            revocation,
        })
    }
}
