//! Domain-separated Ed25519: `Sign(sk, "nemo-v1/" || context || 0x00 || enc(payload))`.

use ed25519_dalek::{Signature, Signer, Verifier};

pub use ed25519_dalek::{SigningKey, VerifyingKey};

use crate::error::{Result, WireError};

pub const CONTEXT_PREFIX: &[u8] = b"nemo-v1/";

pub const HOME_SERVER_BINDING: &str = "home-server-binding";
pub const PREKEY: &str = "prekey";
pub const REVOCATION: &str = "revocation";
pub const GROUP_INVITE: &str = "group-invite";
pub const GROUP_ADMIT: &str = "group-admit";
pub const INVITEE_PROOF: &str = "invitee-proof";
pub const REMOVE_SIDECAR: &str = "remove-sidecar";
pub const SIGNING_KEY_REPLACE: &str = "signing-key-replace";
pub const SERVER_BUNDLE: &str = "server-bundle";
pub const SERVER_SIGN_ROTATE: &str = "server-sign-rotate";
pub const MAILBOX_OWNER: &str = "mailbox-owner";

pub fn message(context: &str, payload: &[u8]) -> Result<Vec<u8>> {
    if context.is_empty() || context.contains('/') || !context.is_ascii() {
        return Err(WireError::Cbor("invalid signature context"));
    }
    let mut msg = Vec::with_capacity(CONTEXT_PREFIX.len() + context.len() + 1 + payload.len());
    msg.extend_from_slice(CONTEXT_PREFIX);
    msg.extend_from_slice(context.as_bytes());
    msg.push(0x00);
    msg.extend_from_slice(payload);
    Ok(msg)
}

pub fn sign(sk: &SigningKey, context: &str, payload: &[u8]) -> Result<[u8; 64]> {
    let msg = message(context, payload)?;
    Ok(sk.sign(&msg).to_bytes())
}

pub fn verify(
    pk: &VerifyingKey,
    context: &str,
    payload: &[u8],
    signature: &[u8; 64],
) -> Result<()> {
    let msg = message(context, payload)?;
    let sig = Signature::from_bytes(signature);
    pk.verify(&msg, &sig).map_err(|_| WireError::Signature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn context_mismatch_fails() {
        let sk = SigningKey::generate(&mut OsRng);
        let pk = sk.verifying_key();
        let payload = b"hello";
        let sig = sign(&sk, REVOCATION, payload).unwrap();
        assert!(verify(&pk, REVOCATION, payload, &sig).is_ok());
        assert!(verify(&pk, PREKEY, payload, &sig).is_err());
    }
}
