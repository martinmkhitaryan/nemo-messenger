//! Blind delivery: mailboxes, capabilities, group streams, local HTTP.
//!
//! MIT. This crate MUST NOT depend on `nemo-core` or `libsignal`.

pub mod error;
pub mod federation;
pub mod group;
pub mod home;
pub mod http;
pub mod pg;
pub mod s2s;
mod tls;

pub use error::{Result, ServerError};
pub use federation::{apply_sign_rotate, pin, pin_each_other, pump, refuse, Enqueue, PumpStats};
pub use group::{
    CreatedGroup, FanoutTarget, GroupHost, GroupId, MemberCred, PendingJoin, StreamAppend,
};
pub use home::{DiscoveryRow, HomeServer, Limits, StoredEnvelope, FETCH_LIMIT_MAX};
pub use http::{router, serve, AppState};

/// Install the rustls ring provider (idempotent).
pub fn tls_install() {
    crate::tls::install_provider();
}

/// Operator logs MUST NOT echo connection URLs, passphrases, or SQLCipher keys.
pub fn log_ops(prefix: &str, err: impl std::fmt::Display) {
    let s = err.to_string();
    let lower = s.to_ascii_lowercase();
    if lower.contains("postgres://")
        || lower.contains("password")
        || lower.contains("passphrase")
        || lower.contains("pragma key")
        || lower.contains("credential_secret")
    {
        eprintln!("{prefix}: (details omitted)");
    } else {
        eprintln!("{prefix}: {s}");
    }
}
