//! Client core: identity, PQXDH / Double Ratchet (libsignal), MLS groups (OpenMLS).
//!
//! AGPL-3.0-only. This crate MUST NOT be a dependency of `nemo-wire` or `nemo-server`.

pub mod app;
pub mod attachment;
mod audio;
pub mod bundle;
pub mod call;
pub mod discovery;
pub mod error;
pub mod group;
pub mod home;
pub mod identity;
pub mod mailbox;
pub mod privacy;
mod store;
pub mod vault;

pub use app::{
    decode, decode_text, encode, encode_text, invite_ttl_bucket, random_call_id, reject_direct_ice,
    AppBody, AppHeader, AppMessage, FileMeta, CALL_ID_LEN,
};
pub use attachment::{open_group_file, seal_group_file};
pub use call::{Call, LocalSignal, TurnConfig};
pub use discovery::{
    classify_binding_gossip, resolve_contact, BindingGossipCheck, ContactPin, Discovery,
    DISCOVERY_REFRESH_SECS,
};
pub use error::{CoreError, Result};
pub use group::{
    Group, PendingJoin, CIPHERSUITE, CREDENTIAL_ID_EXT, GROUP_SIGNING_EXT, UPDATE_BEFORE_SEND,
    UPDATE_INTERVAL, UPDATE_ON_ONLINE,
};
pub use home::{
    EnqueueResult, HomeSession, HomeState, HomeTransport, HostAccept, HostCred, HostGroup,
    HttpHome, HttpRequest, HttpResponse, MailboxRow, StoredContact, FETCH_LIMIT,
};
pub use identity::{
    revocation_from_mnemonic, Installation, RevocationExport, PREKEY_RESTOCK_BELOW,
    PREKEY_STOCK_TARGET,
};
pub use mailbox::messages_lost;
pub use privacy::{
    hop_for, reject_maximum, EnvelopeSink, Hop, MemSink, PrivacyMode, PrivacyTransport,
    WakeCoalesce, PRIVATE_BATCH_MAX_MS, PRIVATE_EXTRA_MAX_MS, PRIVATE_EXTRA_MIN_MS,
    WAKE_COALESCE_MS,
};
pub use vault::Vault;
