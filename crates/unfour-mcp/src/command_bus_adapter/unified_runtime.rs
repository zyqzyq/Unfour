use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use unfour_cloud_sync::{SyncDependencies, SyncOutboxHook};
use unfour_command_bus::{CommandBus, CommandBusExtensions};
use unfour_local_storage::LocalDb;

use crate::StorageMode;

pub(super) async fn unified_command_bus(mode: StorageMode) -> unfour_core::AppResult<CommandBus> {
    let db = match mode {
        StorageMode::Default => LocalDb::connect_existing_default().await?,
        StorageMode::Ephemeral => {
            let options = SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true)
                .foreign_keys(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await?;
            LocalDb::from_pool(pool)
        }
    };

    // The standalone adapter shares the same compatibility migration entry
    // point as desktop. Ephemeral mode applies it only in memory and therefore
    // preserves the no-filesystem registry/CI contract.
    unfour_cloud_sync_storage::migrate(db.pool()).await?;
    let dependencies = SyncDependencies::default();
    let hook = Arc::new(SyncOutboxHook::new(
        dependencies.ids,
        dependencies.clock,
        None,
    ));
    let extensions = CommandBusExtensions::new(vec![hook]);

    match mode {
        StorageMode::Default => {
            CommandBus::from_existing_db_without_seeding_with_extensions(db, extensions).await
        }
        StorageMode::Ephemeral => CommandBus::from_db_with_extensions(db, extensions).await,
    }
}
