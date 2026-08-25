use chrono::Utc;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Column, Executor, Row, TypeInfo, ValueRef};
use std::path::Path;
use std::time::{Duration, Instant};
use unfour_core::models::{
    DatabaseBrowseInput, DatabaseBrowseResult, DatabaseCellValue, DatabaseConnection,
    DatabaseConnectionConfig, DatabaseConnectionInput, DatabaseForeignKey, DatabaseIndex,
    DatabaseQueryInput, DatabaseQueryResult, DatabaseQuerySafety, DatabaseResultColumn,
    DatabaseRowMutationInput, DatabaseRowMutationResult, DatabaseSchema, DatabaseTable,
    DatabaseTableColumn, DatabaseTableStructure, DatabaseTableStructureInput, DatabaseTestResult,
    DbQueryHistoryEntry, DbQueryHistoryRecordInput, SavedSql, SavedSqlInput,
};
use unfour_core::redaction::redact_connection_string;
use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;

mod connection_domain;
mod connections;
mod mysql;
mod pools;
mod postgres;
mod queries;
mod row_mutations;
mod schema;
mod sql;
mod sqlite;
mod tables;

pub use connection_domain::DatabaseConnectionCleanup;
use connections::*;
use mysql::*;
use postgres::*;
use sql::*;
use sqlite::*;

#[derive(Clone)]
pub struct DatabaseService {
    pub(super) db: LocalDb,
    pub(super) secret_store: Option<SecretStore>,
}

impl DatabaseService {
    pub fn new(db: LocalDb) -> Self {
        Self {
            db,
            secret_store: None,
        }
    }

    pub fn with_secret_store(mut self, secret_store: SecretStore) -> Self {
        self.secret_store = Some(secret_store);
        self
    }

    pub fn capability_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "status": "mvp",
            "backend": "sqlx",
            "activeDrivers": ["sqlite", "postgres", "mysql"],
            "reservedDrivers": [],
            "features": [
                "connection-metadata-crud",
                "sqlite-connection-test",
                "sqlite-schema-browser",
                "sqlite-sql-editor",
                "sqlite-read-only-table-data",
                "postgres-connection-test",
                "postgres-schema-browser",
                "postgres-sql-editor",
                "postgres-read-only-table-data",
                "mysql-connection-test",
                "mysql-schema-browser",
                "mysql-sql-editor",
                "mysql-read-only-table-data",
                "confirmed-single-row-crud",
                "optimistic-row-conflict-detection",
                "paged-query-results",
                "credential-backed-auth",
                "on-demand-table-structure"
            ]
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "database_tests/mod.rs"]
mod tests;
