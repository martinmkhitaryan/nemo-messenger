//! Client discovery checks. Servers are untrusted; verify every row.

use nemo_wire::card::{ContactCard, HomeServerBinding};
use nemo_wire::ids::{IdentityId, KEY_LEN};
use nemo_wire::{RevocationStatement, VerifyingKey};

use crate::error::{CoreError, Result};

/// MUST refresh at least this often (phase 2 §8).
pub const DISCOVERY_REFRESH_SECS: u64 = 12 * 3600;
/// SHOULD refresh this often.
pub const DISCOVERY_REFRESH_SHOULD_SECS: u64 = 4 * 3600;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Discovery {
    pub identity_public_key: [u8; KEY_LEN],
    pub revocation_public_key: [u8; KEY_LEN],
    pub binding: HomeServerBinding,
    pub revocation: Option<RevocationStatement>,
}

impl Discovery {
    pub fn identity_id(&self) -> IdentityId {
        nemo_wire::identity_id(&self.identity_public_key)
    }

    pub fn verify(&self, now_unix: u64) -> Result<()> {
        let pk = VerifyingKey::from_bytes(&self.identity_public_key)
            .map_err(|_| CoreError::Wire(nemo_wire::WireError::Signature))?;
        self.binding.verify(&pk, now_unix)?;
        if let Some(stmt) = &self.revocation {
            let rpk = RevocationStatement::verifying_key(&self.revocation_public_key)?;
            stmt.verify(&rpk)?;
            if stmt.identity_id != self.identity_id() {
                return Err(CoreError::IdentityMismatch);
            }
        }
        Ok(())
    }

    pub fn check_not_revoked(&self) -> Result<()> {
        if self.revocation.is_some() {
            Err(CoreError::Revoked)
        } else {
            Ok(())
        }
    }
}

/// First contact: current binding comes from discovery, not the card snapshot.
pub fn resolve_contact(
    card: &ContactCard,
    discovery: &Discovery,
    now_unix: u64,
) -> Result<HomeServerBinding> {
    card.verify(now_unix)?;
    discovery.verify(now_unix)?;
    if discovery.identity_public_key != card.identity_public_key {
        return Err(CoreError::IdentityMismatch);
    }
    if discovery.revocation_public_key != card.revocation_public_key {
        return Err(CoreError::IdentityMismatch);
    }
    discovery.check_not_revoked()?;
    if discovery.binding.seq < card.binding.seq {
        return Err(CoreError::BindingDowngrade);
    }
    Ok(discovery.binding.clone())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactPin {
    pub identity_id: IdentityId,
    pub identity_public_key: [u8; KEY_LEN],
    pub revocation_public_key: [u8; KEY_LEN],
    pub seq: u64,
}

impl ContactPin {
    pub fn from_card(card: &ContactCard) -> Self {
        Self {
            identity_id: card.identity_id(),
            identity_public_key: card.identity_public_key,
            revocation_public_key: card.revocation_public_key,
            seq: card.binding.seq,
        }
    }

    /// Pin the highest accepted seq. A lower discovery seq is an alert, not a downgrade.
    pub fn refresh(&mut self, discovery: &Discovery, now_unix: u64) -> Result<HomeServerBinding> {
        discovery.verify(now_unix)?;
        if discovery.identity_public_key != self.identity_public_key {
            return Err(CoreError::IdentityMismatch);
        }
        discovery.check_not_revoked()?;
        if discovery.binding.seq < self.seq {
            return Err(CoreError::BindingDowngrade);
        }
        self.seq = discovery.binding.seq;
        Ok(discovery.binding.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_wire::ids;
    use nemo_wire::SigningKey;
    use rand::rngs::OsRng;

    fn signed_pair(sk: &SigningKey, rev_pk: [u8; 32], seq: u64) -> (ContactCard, Discovery) {
        let hpke = [9u8; 32];
        let binding = HomeServerBinding::sign(
            sk,
            HomeServerBinding {
                server_id: ids::server_id(&hpke),
                server_hpke_public_key: hpke,
                host: "nemo.example".into(),
                seq,
                expires_at: 1_800_000_000,
                signature: [0; 64],
            },
        )
        .unwrap();
        let card = ContactCard {
            identity_public_key: sk.verifying_key().to_bytes(),
            revocation_public_key: rev_pk,
            share_token: [1u8; 32],
            binding: binding.clone(),
        };
        let discovery = Discovery {
            identity_public_key: card.identity_public_key,
            revocation_public_key: card.revocation_public_key,
            binding,
            revocation: None,
        };
        (card, discovery)
    }

    #[test]
    fn resolve_uses_discovery_binding() {
        let sk = SigningKey::generate(&mut OsRng);
        let rev = SigningKey::generate(&mut OsRng);
        let (card, discovery) = signed_pair(&sk, rev.verifying_key().to_bytes(), 1);
        let b = resolve_contact(&card, &discovery, 1_700_000_000).unwrap();
        assert_eq!(b.seq, 1);
    }

    #[test]
    fn lower_seq_is_downgrade() {
        let sk = SigningKey::generate(&mut OsRng);
        let rev = SigningKey::generate(&mut OsRng);
        let pk = rev.verifying_key().to_bytes();
        let (card, _) = signed_pair(&sk, pk, 2);
        let (_, discovery) = signed_pair(&sk, pk, 1);
        assert!(matches!(
            resolve_contact(&card, &discovery, 1_700_000_000),
            Err(CoreError::BindingDowngrade)
        ));
    }

    #[test]
    fn revoked_refuses_session_and_admit() {
        let sk = SigningKey::generate(&mut OsRng);
        let rev = SigningKey::generate(&mut OsRng);
        let (card, mut discovery) = signed_pair(&sk, rev.verifying_key().to_bytes(), 1);
        discovery.revocation =
            Some(RevocationStatement::sign(&rev, card.identity_id(), 1_700_000_000).unwrap());
        assert!(matches!(
            resolve_contact(&card, &discovery, 1_700_000_000),
            Err(CoreError::Revoked)
        ));
        let mut pin = ContactPin::from_card(&card);
        assert!(matches!(
            pin.refresh(&discovery, 1_700_000_000),
            Err(CoreError::Revoked)
        ));
    }

    #[test]
    fn pin_accepts_higher_seq() {
        let sk = SigningKey::generate(&mut OsRng);
        let rev = SigningKey::generate(&mut OsRng);
        let pk = rev.verifying_key().to_bytes();
        let (card, _) = signed_pair(&sk, pk, 1);
        let (_, newer) = signed_pair(&sk, pk, 3);
        let mut pin = ContactPin::from_card(&card);
        pin.refresh(&newer, 1_700_000_000).unwrap();
        assert_eq!(pin.seq, 3);
    }
}
