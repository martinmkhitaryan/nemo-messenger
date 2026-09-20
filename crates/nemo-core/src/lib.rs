//! Client core: identity, PQXDH / Double Ratchet (libsignal), MLS groups (OpenMLS).
//!
//! AGPL-3.0-only. This crate MUST NOT be a dependency of `nemo-wire` or `nemo-server`.

pub mod app;
pub mod bundle;
pub mod discovery;
pub mod error;
pub mod group;
pub mod home;
pub mod identity;
pub mod mailbox;
pub mod privacy;
mod store;
pub mod vault;

pub use app::{decode, decode_text, encode, encode_text, AppBody, AppHeader, AppMessage};
pub use discovery::{resolve_contact, ContactPin, Discovery, DISCOVERY_REFRESH_SECS};
pub use error::{CoreError, Result};
pub use group::{
    Group, PendingJoin, CIPHERSUITE, CREDENTIAL_ID_EXT, GROUP_SIGNING_EXT, UPDATE_BEFORE_SEND,
    UPDATE_INTERVAL, UPDATE_ON_ONLINE,
};
pub use home::{
    EnqueueResult, HomeSession, HomeTransport, HostAccept, HostCred, HostGroup, HttpHome,
    HttpRequest, HttpResponse, MailboxRow, FETCH_LIMIT,
};
pub use identity::{
    revocation_from_mnemonic, Installation, RevocationExport, PREKEY_RESTOCK_BELOW,
    PREKEY_STOCK_TARGET,
};
pub use mailbox::messages_lost;
pub use vault::Vault;
