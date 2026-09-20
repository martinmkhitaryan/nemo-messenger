//! Signed group-invite, group-admit, and in-band invitee proof (ADR-0018).

use ed25519_dalek::{SigningKey, VerifyingKey};

use crate::cbor::{self, Value};
use crate::error::{Result, WireError};
use crate::ids::{copy_fixed, sha256, KEY_LEN, SIG_LEN};
use crate::sign;
use crate::PROTOCOL_VERSION;

pub const INTRO_TTL_5_MIN: u64 = 5 * 60;
pub const INTRO_TTL_30_MIN: u64 = 30 * 60;
pub const INTRO_TTL_1_HOUR: u64 = 60 * 60;

pub fn allowed_intro_ttl(ttl_secs: u64) -> bool {
    matches!(
        ttl_secs,
        INTRO_TTL_5_MIN | INTRO_TTL_30_MIN | INTRO_TTL_1_HOUR
    )
}

/// `H(identity_public_key || salt)`. Host-visible; not an identity.
pub fn invitee_binding(identity_public_key: &[u8; KEY_LEN], salt: &[u8; KEY_LEN]) -> [u8; KEY_LEN] {
    let mut buf = [0u8; KEY_LEN * 2];
    buf[..KEY_LEN].copy_from_slice(identity_public_key);
    buf[KEY_LEN..].copy_from_slice(salt);
    sha256(&buf)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupInvite {
    pub group_id: [u8; KEY_LEN],
    pub nonce: [u8; KEY_LEN],
    pub ttl_secs: u64,
    pub invitee_binding: Option<[u8; KEY_LEN]>,
    pub salt: Option<[u8; KEY_LEN]>,
    pub signature: [u8; SIG_LEN],
}

impl GroupInvite {
    fn payload_map(&self) -> Vec<(u64, Value)> {
        let mut pairs = vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.group_id.to_vec())),
            (2, Value::Bytes(self.nonce.to_vec())),
            (3, Value::Uint(self.ttl_secs)),
        ];
        if let Some(b) = self.invitee_binding {
            pairs.push((4, Value::Bytes(b.to_vec())));
        }
        if let Some(s) = self.salt {
            pairs.push((5, Value::Bytes(s.to_vec())));
        }
        pairs
    }

    fn payload(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(self.payload_map()))
    }

    pub fn sign(
        member_sk: &SigningKey,
        group_id: [u8; KEY_LEN],
        nonce: [u8; KEY_LEN],
        ttl_secs: u64,
        bind: Option<([u8; KEY_LEN], [u8; KEY_LEN])>,
    ) -> Result<Self> {
        if !allowed_intro_ttl(ttl_secs) {
            return Err(WireError::Cbor("intro ttl must be 5m, 30m, or 1h"));
        }
        let (invitee_binding, salt) = match bind {
            Some((b, s)) => (Some(b), Some(s)),
            None => (None, None),
        };
        let mut invite = Self {
            group_id,
            nonce,
            ttl_secs,
            invitee_binding,
            salt,
            signature: [0; SIG_LEN],
        };
        invite.signature = sign::sign(member_sk, sign::GROUP_INVITE, &invite.payload())?;
        Ok(invite)
    }

    pub fn verify(&self, member_pk: &VerifyingKey) -> Result<()> {
        if !allowed_intro_ttl(self.ttl_secs) {
            return Err(WireError::Cbor("intro ttl must be 5m, 30m, or 1h"));
        }
        if self.invitee_binding.is_some() != self.salt.is_some() {
            return Err(WireError::Cbor("bound invite needs binding and salt"));
        }
        sign::verify(
            member_pk,
            sign::GROUP_INVITE,
            &self.payload(),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut pairs = self.payload_map();
        pairs.push((6, Value::Bytes(self.signature.to_vec())));
        cbor::encode(&Value::Map(pairs))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes)? else {
            return Err(WireError::Cbor("group-invite must be a map"));
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
        if version != PROTOCOL_VERSION as u64 {
            return Err(WireError::UnknownVersion(version));
        }
        Ok(Self {
            group_id: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?,
            nonce: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 2)?)?)?,
            ttl_secs: cbor::expect_uint(cbor::map_get(&m, 3)?)?,
            invitee_binding: match cbor::map_get_opt(&m, 4) {
                Some(v) => Some(copy_fixed(cbor::expect_bytes(v)?)?),
                None => None,
            },
            salt: match cbor::map_get_opt(&m, 5) {
                Some(v) => Some(copy_fixed(cbor::expect_bytes(v)?)?),
                None => None,
            },
            signature: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 6)?)?)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupAdmit {
    pub group_id: [u8; KEY_LEN],
    pub pending_id: [u8; KEY_LEN],
    pub signature: [u8; SIG_LEN],
}

impl GroupAdmit {
    fn payload(group_id: &[u8; KEY_LEN], pending_id: &[u8; KEY_LEN]) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(group_id.to_vec())),
            (2, Value::Bytes(pending_id.to_vec())),
            (3, Value::Text("admit".into())),
        ]))
    }

    pub fn sign(
        member_sk: &SigningKey,
        group_id: [u8; KEY_LEN],
        pending_id: [u8; KEY_LEN],
    ) -> Result<Self> {
        let signature = sign::sign(
            member_sk,
            sign::GROUP_ADMIT,
            &Self::payload(&group_id, &pending_id),
        )?;
        Ok(Self {
            group_id,
            pending_id,
            signature,
        })
    }

    pub fn verify(&self, member_pk: &VerifyingKey) -> Result<()> {
        sign::verify(
            member_pk,
            sign::GROUP_ADMIT,
            &Self::payload(&self.group_id, &self.pending_id),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.group_id.to_vec())),
            (2, Value::Bytes(self.pending_id.to_vec())),
            (3, Value::Text("admit".into())),
            (4, Value::Bytes(self.signature.to_vec())),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes)? else {
            return Err(WireError::Cbor("group-admit must be a map"));
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
        if version != PROTOCOL_VERSION as u64 {
            return Err(WireError::UnknownVersion(version));
        }
        if cbor::expect_text(cbor::map_get(&m, 3)?)? != "admit" {
            return Err(WireError::Cbor("expected admit"));
        }
        Ok(Self {
            group_id: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?,
            pending_id: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 2)?)?)?,
            signature: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 4)?)?)?,
        })
    }
}

/// Identity-key proof that the invitee matches a bound invite. Never sent to the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InviteeProof {
    pub group_id: [u8; KEY_LEN],
    pub pending_id: [u8; KEY_LEN],
    pub signature: [u8; SIG_LEN],
}

impl InviteeProof {
    fn payload(group_id: &[u8; KEY_LEN], pending_id: &[u8; KEY_LEN]) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(group_id.to_vec())),
            (2, Value::Bytes(pending_id.to_vec())),
        ]))
    }

    pub fn sign(
        identity_sk: &SigningKey,
        group_id: [u8; KEY_LEN],
        pending_id: [u8; KEY_LEN],
    ) -> Result<Self> {
        let signature = sign::sign(
            identity_sk,
            sign::INVITEE_PROOF,
            &Self::payload(&group_id, &pending_id),
        )?;
        Ok(Self {
            group_id,
            pending_id,
            signature,
        })
    }

    pub fn verify(&self, identity_pk: &VerifyingKey) -> Result<()> {
        sign::verify(
            identity_pk,
            sign::INVITEE_PROOF,
            &Self::payload(&self.group_id, &self.pending_id),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&Value::Map(vec![
            (0, Value::Uint(PROTOCOL_VERSION as u64)),
            (1, Value::Bytes(self.group_id.to_vec())),
            (2, Value::Bytes(self.pending_id.to_vec())),
            (3, Value::Bytes(self.signature.to_vec())),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let Value::Map(m) = cbor::decode(bytes)? else {
            return Err(WireError::Cbor("invitee-proof must be a map"));
        };
        let version = cbor::expect_uint(cbor::map_get(&m, 0)?)?;
        if version != PROTOCOL_VERSION as u64 {
            return Err(WireError::UnknownVersion(version));
        }
        Ok(Self {
            group_id: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 1)?)?)?,
            pending_id: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 2)?)?)?,
            signature: copy_fixed(cbor::expect_bytes(cbor::map_get(&m, 3)?)?)?,
        })
    }
}

/// Admitter check for a bound invite. The host never sees this proof.
pub fn verify_bound_invite(
    invite: &GroupInvite,
    proof: &InviteeProof,
    identity_pk: &VerifyingKey,
) -> Result<()> {
    let (Some(want), Some(salt)) = (invite.invitee_binding, invite.salt) else {
        return Err(WireError::Cbor("invite is not bound"));
    };
    if proof.group_id != invite.group_id {
        return Err(WireError::Cbor("proof group_id mismatch"));
    }
    proof.verify(identity_pk)?;
    let got = invitee_binding(&identity_pk.to_bytes(), &salt);
    if got != want {
        return Err(WireError::Signature);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn invite_admit_proof_roundtrip() {
        let member = SigningKey::generate(&mut OsRng);
        let invitee = SigningKey::generate(&mut OsRng);
        let mut nonce = [0u8; 32];
        nonce[0] = 9;
        let salt = [3u8; 32];
        let binding = invitee_binding(&invitee.verifying_key().to_bytes(), &salt);
        let invite = GroupInvite::sign(
            &member,
            [1u8; 32],
            nonce,
            INTRO_TTL_30_MIN,
            Some((binding, salt)),
        )
        .unwrap();
        invite.verify(&member.verifying_key()).unwrap();
        assert_eq!(GroupInvite::decode(&invite.encode()).unwrap(), invite);

        let admit = GroupAdmit::sign(&member, [1u8; 32], [2u8; 32]).unwrap();
        admit.verify(&member.verifying_key()).unwrap();
        assert_eq!(GroupAdmit::decode(&admit.encode()).unwrap(), admit);

        let proof = InviteeProof::sign(&invitee, [1u8; 32], [2u8; 32]).unwrap();
        verify_bound_invite(&invite, &proof, &invitee.verifying_key()).unwrap();
        assert!(verify_bound_invite(&invite, &proof, &member.verifying_key()).is_err());
    }

    #[test]
    fn rejects_bad_ttl_and_unbound_as_bound() {
        let sk = SigningKey::generate(&mut OsRng);
        assert!(GroupInvite::sign(&sk, [1u8; 32], [2u8; 32], 99, None).is_err());
        let invite = GroupInvite::sign(&sk, [1u8; 32], [2u8; 32], INTRO_TTL_5_MIN, None).unwrap();
        let proof = InviteeProof::sign(&sk, [1u8; 32], [2u8; 32]).unwrap();
        assert!(verify_bound_invite(&invite, &proof, &sk.verifying_key()).is_err());
    }
}
