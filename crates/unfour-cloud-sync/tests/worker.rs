//! Cloud Sync integration suite, organized by behavior rather than implementation size.
//! Shared infrastructure is under `support`; protocol fixtures stay in `contracts.rs`.

#[path = "worker/accounts.rs"]
mod accounts;
#[path = "worker/api_hierarchy.rs"]
mod api_hierarchy;
#[path = "worker/connections.rs"]
mod connections;
#[path = "worker/dead_letter_recovery.rs"]
mod dead_letter_recovery;
#[path = "worker/hierarchy_conflicts.rs"]
mod hierarchy_conflicts;
#[path = "worker/initial_upload.rs"]
mod initial_upload;
#[path = "worker/pull_cursor.rs"]
mod pull_cursor;
#[path = "worker/push_retry.rs"]
mod push_retry;
#[path = "worker/scheduling.rs"]
mod scheduling;
#[path = "worker/snapshots.rs"]
mod snapshots;
#[path = "worker/ssh_tasks.rs"]
mod ssh_tasks;
#[path = "worker/support/mod.rs"]
mod support;
#[path = "worker/transactions.rs"]
mod transactions;
#[path = "worker/workspace_conflicts.rs"]
mod workspace_conflicts;
