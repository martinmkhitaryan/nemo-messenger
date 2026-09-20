//! Version-1 wire encodings shared by client and server.
//!
//! This crate MUST NOT depend on `libsignal`. See ADR-0028.

pub mod card;
pub mod cbor;
pub mod discovery;
pub mod envelope;
pub mod error;
pub mod hostframe;
pub mod hpke;
pub mod ids;
pub mod join;
pub mod prekey;
pub mod revocation;
pub mod sign;

pub use card::{ContactCard, HomeServerBinding, CARD_MAX_BYTES};
pub use discovery::DiscoveryRecord;
pub use envelope::{
    InnerEnvelope, MessageType, OuterEnvelope, PaddedMessage, TtlBucket, INNER_TEXT_OUTER,
};
pub use error::WireError;
pub use hostframe::{
    fail_payload, hello_payload, ok_payload, parse_hello, parse_payload, AttachmentReserve,
    AttachmentSizeBucket, MailboxOwnerAuth, RemoveBundle, S2sFrame, S2sPayload, ServerBundle,
    ServerSignRotate, SigningKeyReplace,
};
pub use hpke::{encap_key, open_outer, seal_inner, seal_to_server, HpkeKeypair};
pub use ids::{
    fingerprint, from_hex, identity_id, parse_identity_id, server_id, to_hex, IdentityId, ServerId,
    ID_LEN,
};
pub use join::{
    allowed_intro_ttl, invitee_binding, verify_bound_invite, GroupAdmit, GroupInvite, InviteeProof,
    INTRO_TTL_30_MIN,
};
pub use revocation::RevocationStatement;
pub use sign::{sign, verify, SigningKey, VerifyingKey, CONTEXT_PREFIX};

pub const PROTOCOL_VERSION: u8 = 1;
