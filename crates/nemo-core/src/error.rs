use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Wire(#[from] nemo_wire::WireError),
    #[error(transparent)]
    Signal(#[from] libsignal_protocol::SignalProtocolError),
    #[error("revocation mnemonic is invalid")]
    Mnemonic,
    #[error("no one-time PQXDH prekey; refuse new 1:1")]
    EmptyPrekeyStock,
    #[error("libsignal identity does not match the pinned contact")]
    IdentityMismatch,
    #[error("invalid libsignal key material")]
    BadKey,
    #[error("ciphertext is not a PQXDH or Double Ratchet message")]
    BadCiphertext,
    #[error("application text exceeds 8192 UTF-8 bytes")]
    TextTooLong,
    #[error("reaction emoji exceeds 32 UTF-8 bytes")]
    EmojiTooLong,
    #[error("ICE candidate is not relay-only")]
    DirectIceForbidden,
    #[error("mls: {0}")]
    Mls(String),
    #[error("own MLS Update is older than 72 hours; Update first")]
    StaleMlsUpdate,
    #[error("unframed MLS Remove is forbidden; use RemoveBundle")]
    UnframedRemove,
    #[error("external MLS join or External Commit is forbidden")]
    ExternalMlsJoin,
    #[error("MLS ReInit is forbidden")]
    MlsReinit,
    #[error("ciphersuite must be 0x0003")]
    WrongCiphersuite,
    #[error("RemoveBundle does not remove exactly the named credential_id")]
    BadRemoveBundle,
    #[error("mailbox inner type is not what this path decrypts")]
    WrongMailboxType,
    #[error("Maximum privacy mode is not shipped")]
    MaximumNotShipped,
    #[error("cover traffic is not shipped in this mode")]
    CoverNotShipped,
    #[error("contact is revoked")]
    Revoked,
    #[error("home-server binding seq went backwards")]
    BindingDowngrade,
    #[error("home server denied the request")]
    Denied,
    #[error("identity already registered on this home")]
    AlreadyRegistered,
    #[error("home HTTP status {0}")]
    HomeHttp(u16),
    #[error("home transport: {0}")]
    Transport(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub(crate) fn mls_err<E: std::fmt::Display>(err: E) -> CoreError {
    CoreError::Mls(err.to_string())
}
