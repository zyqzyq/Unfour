mod canonical;
mod conflict_scope;
mod hook;
mod repository;
mod service;
mod transport;
mod types;

pub use canonical::{canonical_payload, parse_remote_change, parse_snapshot_item};
pub use hook::SyncOutboxHook;
pub use repository::SyncRepository;
pub use service::{SyncRuntime, SyncService};
pub use transport::{
    DesktopSessionCredential, DesktopSessionProvider, HttpSyncTransport, SyncTransport,
    TransportError,
};
pub use types::*;

pub const CLOUD_SYNC_ENTITLEMENT: &str = "cloud_sync";
/// Protocol v4 preserves the v3 SSH Task entities and adds the Connection
/// aggregate (`connection`). The current client declares this value on every
/// request; the API may support multiple protocol contracts per request
/// version, while this client does not negotiate other device versions.
pub const PROTOCOL_VERSION: u32 = 4;
pub const PAYLOAD_SCHEMA_VERSION: i64 = 1;
