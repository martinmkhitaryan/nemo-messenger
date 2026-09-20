//! Blind delivery: mailboxes, capabilities, group streams, local HTTP.
//!
//! MIT. This crate MUST NOT depend on `nemo-core` or `libsignal`.

pub mod error;
pub mod federation;
pub mod group;
pub mod home;
pub mod http;
pub mod pg;

pub use error::{Result, ServerError};
pub use federation::{apply_sign_rotate, pin, pin_each_other, pump, refuse, Enqueue, PumpStats};
pub use group::{
    CreatedGroup, FanoutTarget, GroupHost, GroupId, MemberCred, PendingJoin, StreamAppend,
};
pub use home::{DiscoveryRow, HomeServer, Limits, StoredEnvelope, FETCH_LIMIT_MAX};
pub use http::{router, serve, AppState};
