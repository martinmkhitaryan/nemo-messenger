use thiserror::Error;

#[derive(Debug, Error)]
pub enum WireError {
    #[error("unknown protocol version {0}")]
    UnknownVersion(u64),
    #[error("cbor: {0}")]
    Cbor(&'static str),
    #[error("wrong length: expected {expected}, got {got}")]
    Length { expected: usize, got: usize },
    #[error("invalid signature")]
    Signature,
    #[error("server_id does not match SHA-256(server_hpke_public_key)")]
    ServerIdMismatch,
    #[error("home-server binding expired")]
    BindingExpired,
    #[error("host must be 1..=64 UTF-8 bytes, DNS or .onion, no scheme or path")]
    BadHost,
    #[error("contact card exceeds 400 bytes")]
    CardTooLarge,
    #[error("revocation status must be \"revoked\"")]
    BadRevocationStatus,
    #[error("body does not fit any padding bucket")]
    NoBucket,
    #[error("envelope truncated or padded incorrectly")]
    BadEnvelope,
    #[error("hpke: {0}")]
    Hpke(&'static str),
    #[error("unknown message type {0:#04x}")]
    UnknownMessageType(u8),
    #[error("unknown ttl_bucket {0}")]
    UnknownTtl(u8),
    #[error("uri: {0}")]
    Uri(&'static str),
}

pub type Result<T> = std::result::Result<T, WireError>;
