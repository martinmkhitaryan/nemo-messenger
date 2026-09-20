//! Nested InnerEnvelope HPKE (RFC 9180).
//!
//! Suite: DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20Poly1305.
//! Info: `nemo-v1/s2s-inner`. AAD: `version || destination_server_id`.

use hpke::{
    aead::ChaCha20Poly1305, kdf::HkdfSha256, kem::X25519HkdfSha256, Deserializable,
    Kem as KemTrait, OpModeR, OpModeS, Serializable,
};
use rand_core::{OsRng, UnwrapErr};

use crate::envelope::{InnerEnvelope, OuterEnvelope};
use crate::error::{Result, WireError};
use crate::ids::{self, copy_fixed, ServerId, KEY_LEN};
use crate::PROTOCOL_VERSION;

type Kem = X25519HkdfSha256;
type Kdf = HkdfSha256;
type Aead = ChaCha20Poly1305;
type PublicKey = <Kem as KemTrait>::PublicKey;
type PrivateKey = <Kem as KemTrait>::PrivateKey;
type EncappedKey = <Kem as KemTrait>::EncappedKey;

pub const INFO: &[u8] = b"nemo-v1/s2s-inner";
pub const ENCAP_LEN: usize = 32;

pub fn encap_key(ciphertext: &[u8]) -> Result<[u8; ENCAP_LEN]> {
    if ciphertext.len() < ENCAP_LEN {
        return Err(WireError::Hpke("truncated enc"));
    }
    copy_fixed(&ciphertext[..ENCAP_LEN])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HpkeKeypair {
    pub public: [u8; KEY_LEN],
    pub secret: [u8; KEY_LEN],
}

impl HpkeKeypair {
    pub fn generate() -> Self {
        let (sk, pk) = Kem::gen_keypair(&mut UnwrapErr(OsRng));
        let public = copy_fixed(pk.to_bytes().as_slice()).expect("x25519 pk");
        let secret = copy_fixed(sk.to_bytes().as_slice()).expect("x25519 sk");
        Self { public, secret }
    }

    pub fn server_id(&self) -> ServerId {
        ids::server_id(&self.public)
    }
}

pub fn aad(destination_server_id: &[u8; KEY_LEN]) -> [u8; 1 + KEY_LEN] {
    let mut out = [0u8; 1 + KEY_LEN];
    out[0] = PROTOCOL_VERSION;
    out[1..].copy_from_slice(destination_server_id);
    out
}

/// RFC 9180 ciphertext: `enc || ct`.
pub fn seal(
    recipient_pk: &[u8; KEY_LEN],
    destination_server_id: &[u8; KEY_LEN],
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let pk = PublicKey::from_bytes(recipient_pk).map_err(|_| WireError::Hpke("recipient pk"))?;
    let (enc, ct) = hpke::single_shot_seal::<Aead, Kdf, Kem, _>(
        &OpModeS::Base,
        &pk,
        INFO,
        plaintext,
        &aad(destination_server_id),
        &mut UnwrapErr(OsRng),
    )
    .map_err(|_| WireError::Hpke("seal"))?;
    let enc_bytes = enc.to_bytes();
    let mut out = Vec::with_capacity(enc_bytes.len() + ct.len());
    out.extend_from_slice(&enc_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn open(
    recipient_sk: &[u8; KEY_LEN],
    destination_server_id: &[u8; KEY_LEN],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    if ciphertext.len() < ENCAP_LEN {
        return Err(WireError::Hpke("truncated enc"));
    }
    let sk = PrivateKey::from_bytes(recipient_sk).map_err(|_| WireError::Hpke("recipient sk"))?;
    let enc = EncappedKey::from_bytes(&ciphertext[..ENCAP_LEN])
        .map_err(|_| WireError::Hpke("encapped key"))?;
    hpke::single_shot_open::<Aead, Kdf, Kem>(
        &OpModeR::Base,
        &sk,
        &enc,
        INFO,
        &ciphertext[ENCAP_LEN..],
        &aad(destination_server_id),
    )
    .map_err(|_| WireError::Hpke("open"))
}

/// Alice has only Server B's HPKE public key (from a contact card binding).
pub fn seal_to_server(
    recipient_pk: &[u8; KEY_LEN],
    inner: &InnerEnvelope,
) -> Result<OuterEnvelope> {
    let dest = ids::server_id(recipient_pk);
    let plaintext = inner.encode_padded()?;
    let hpke_ciphertext = seal(recipient_pk, &dest, &plaintext)?;
    Ok(OuterEnvelope {
        destination_server_id: dest,
        hpke_ciphertext,
    })
}

pub fn seal_inner(recipient: &HpkeKeypair, inner: &InnerEnvelope) -> Result<OuterEnvelope> {
    seal_to_server(&recipient.public, inner)
}

pub fn open_outer(recipient: &HpkeKeypair, outer: &OuterEnvelope) -> Result<InnerEnvelope> {
    if outer.destination_server_id != recipient.server_id() {
        return Err(WireError::ServerIdMismatch);
    }
    let plaintext = open(
        &recipient.secret,
        &outer.destination_server_id,
        &outer.hpke_ciphertext,
    )?;
    InnerEnvelope::decode_padded(&plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{MessageType, PaddedMessage, TtlBucket};

    #[test]
    fn seal_open_inner() {
        let server = HpkeKeypair::generate();
        let inner = InnerEnvelope {
            delivery_capability: [0x11; 32],
            ttl_bucket: TtlBucket::HOUR,
            idempotency_token: [0x22; 32],
            padded_message: PaddedMessage::pad(MessageType::DoubleRatchet, vec![7, 8, 9]).unwrap(),
        };
        let outer = seal_inner(&server, &inner).unwrap();
        assert_eq!(outer.destination_server_id, server.server_id());
        let opened = open_outer(&server, &outer).unwrap();
        assert_eq!(opened.delivery_capability, inner.delivery_capability);
        assert_eq!(opened.ttl_bucket, inner.ttl_bucket);
        assert_eq!(&opened.padded_message.body[..3], &[7, 8, 9]);
        let wire = outer.encode();
        let decoded = OuterEnvelope::decode(&wire).unwrap();
        assert_eq!(
            open_outer(&server, &decoded).unwrap().idempotency_token,
            [0x22; 32]
        );
    }

    #[test]
    fn wrong_key_fails() {
        let a = HpkeKeypair::generate();
        let b = HpkeKeypair::generate();
        let inner = InnerEnvelope {
            delivery_capability: [1; 32],
            ttl_bucket: TtlBucket::DEFAULT,
            idempotency_token: [2; 32],
            padded_message: PaddedMessage::pad(MessageType::MlsApp, vec![1]).unwrap(),
        };
        let outer = seal_inner(&a, &inner).unwrap();
        assert!(open_outer(&b, &outer).is_err());
    }
}
