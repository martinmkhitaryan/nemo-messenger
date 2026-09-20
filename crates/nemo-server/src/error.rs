use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    Wire(#[from] nemo_wire::WireError),
    /// Unknown, expired, burned, or empty-prekey — same shape (ADR-0031).
    #[error("denied")]
    Denied,
    #[error("mailbox owner authentication failed")]
    OwnerAuth,
    #[error("mailbox already registered")]
    AlreadyRegistered,
    #[error("fetch limit must be 1..=256")]
    FetchLimit,
    #[error("peer is not pinned")]
    NotPinned,
    #[error("peer is refused")]
    PeerRefused,
    #[error("s2s frame counter gap")]
    CounterGap,
    #[error("s2s hello mismatch")]
    HelloMismatch,
    #[error("outbound expired")]
    OutboundExpired,
}

pub type Result<T> = std::result::Result<T, ServerError>;
