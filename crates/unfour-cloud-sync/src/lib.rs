mod canonical;
mod canonical_parent;
mod conflict_scope;
mod entity_registry;
mod error;
mod hook;
mod remote_compatibility;
mod repository;
mod service;
mod transport;
mod types;

pub use canonical::{canonical_payload, parse_remote_change, parse_snapshot_item};
pub use entity_registry::{
    sync_entity_descriptor, sync_entity_descriptor_for_domain, ParentDependency, RecoveryPolicy,
    SnapshotProvider, SyncEntityAdapters, SyncEntityDescriptor, SYNC_ENTITY_REGISTRY,
};
pub use error::*;
pub use hook::SyncOutboxHook;
pub use repository::SyncRepository;
pub use service::{SyncRuntime, SyncService};
pub use transport::{
    CloudSyncAuthFailure, DesktopSessionCredential, DesktopSessionProvider, HttpSyncTransport,
    SyncTransport, TransportError,
};
pub use types::*;

pub const CLOUD_SYNC_ENTITLEMENT: &str = "cloud_sync";
/// Protocol 5 is the first frozen common sync-engine and wire-contract baseline.
/// Adding an ordinary Registry entity does not change this value; readers skip
/// future entity names they do not recognize.
pub const PROTOCOL_VERSION: u32 = 5;
pub const PAYLOAD_SCHEMA_VERSION: i64 = 1;
