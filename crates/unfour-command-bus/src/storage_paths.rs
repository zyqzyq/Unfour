use std::path::PathBuf;

use unfour_core::AppResult;
use unfour_local_storage::LocalDb;

use crate::CommandBus;

pub use unfour_paths::{DEFAULT_DATABASE_FILE, DEFAULT_PRODUCT_DATA_DIR};

pub fn default_database_path() -> AppResult<PathBuf> {
    LocalDb::default_database_path()
}

impl CommandBus {
    pub async fn from_existing_default_storage_read_only() -> AppResult<Self> {
        let db = LocalDb::connect_existing_default_read_only().await?;
        Self::from_existing_db_read_only(db).await
    }

    pub async fn from_existing_default_storage() -> AppResult<Self> {
        let db = LocalDb::connect_existing_default().await?;
        Self::from_existing_db_read_only(db).await
    }

    pub async fn from_existing_storage_dir_read_only(
        storage_dir: impl AsRef<std::path::Path>,
    ) -> AppResult<Self> {
        let db = LocalDb::connect_existing_read_only_path(
            storage_dir.as_ref().join(DEFAULT_DATABASE_FILE),
        )
        .await?;
        Self::from_existing_db_read_only(db).await
    }
}
