//! SQLite persistence boundary for the local-first sync overlay.
//!
//! The private modules implement the same repository API by responsibility:
//! bindings/diagnostics, initial upload, durable outbox heads, network attempts,
//! remote reconciliation, snapshot staging, protocol bootstrap and recovery.
//! Methods ending in `_on` use the caller's connection; moving an implementation
//! here must not introduce a nested transaction or commit Core changes separately.

use sqlx::SqlitePool;

mod api_bootstrap;
mod attempts;
mod bindings;
mod bootstrap;
mod initial_upload;
mod orphan_reconciliation;
mod outbox;
mod reconciliation;
mod recovery;
mod snapshots;

#[derive(Clone)]
pub struct SyncRepository {
    pool: SqlitePool,
}

impl SyncRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
