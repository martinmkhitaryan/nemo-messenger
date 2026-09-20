//! Build and open mailbox envelopes. HTTP is `home::HomeSession`.

use nemo_wire::envelope::{InnerEnvelope, MessageType, OuterEnvelope, PaddedMessage, TtlBucket};
use nemo_wire::hpke::{self, HpkeKeypair};
use nemo_wire::ids::KEY_LEN;
use rand::RngCore;

use crate::error::{CoreError, Result};

pub fn random_token() -> [u8; KEY_LEN] {
    let mut t = [0u8; KEY_LEN];
    rand::rngs::OsRng.fill_bytes(&mut t);
    t
}

/// Pad `body` and HPKE-seal it to the destination server's public key.
pub fn wrap(
    dest_hpke_public: &[u8; KEY_LEN],
    delivery_capability: [u8; KEY_LEN],
    ttl_bucket: TtlBucket,
    type_: MessageType,
    body: Vec<u8>,
) -> Result<OuterEnvelope> {
    let inner = InnerEnvelope {
        delivery_capability,
        ttl_bucket,
        idempotency_token: random_token(),
        padded_message: PaddedMessage::pad(type_, body)?,
    };
    Ok(hpke::seal_to_server(dest_hpke_public, &inner)?)
}

/// What Server B does: open HPKE, keep the inner. Clients never hold this secret.
pub fn deliver(server: &HpkeKeypair, outer: &OuterEnvelope) -> Result<InnerEnvelope> {
    Ok(hpke::open_outer(server, outer)?)
}

pub fn expect_type(inner: &InnerEnvelope, want: MessageType) -> Result<&[u8]> {
    if inner.padded_message.type_ != want {
        return Err(CoreError::WrongMailboxType);
    }
    Ok(inner.padded_message.body.as_slice())
}

pub fn expect_ratchet(inner: &InnerEnvelope) -> Result<&[u8]> {
    match inner.padded_message.type_ {
        MessageType::DoubleRatchet | MessageType::AttachmentDr => {
            Ok(inner.padded_message.body.as_slice())
        }
        _ => Err(CoreError::WrongMailboxType),
    }
}

/// Phase 4 §7: a hole after the last acked seq is loss, not delay.
pub fn messages_lost(last_acked_seq: u64, first_fetched_seq: u64) -> bool {
    first_fetched_seq > last_acked_seq.saturating_add(1)
}
