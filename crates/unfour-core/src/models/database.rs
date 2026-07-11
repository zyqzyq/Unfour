use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnectionInput {
    pub id: Option<String>,
    pub workspace_id: String,
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    #[serde(default)]
    pub ssl_mode: Option<String>,
    pub sqlite_path: Option<String>,
    pub credential_ref: Option<String>,
    /// When true, the connection rejects any data- or schema-modifying SQL and
    /// row edits. Defaults to false so existing callers and stored rows are
    /// unaffected.
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnection {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    #[serde(default)]
    pub ssl_mode: Option<String>,
    pub sqlite_path: Option<String>,
    pub credential_ref: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StoredConnection {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    /// Legacy joined-row shape for storage paths that only need parent
    /// metadata plus advanced JSON. New connection engines should prefer
    /// subtype-specific row structs for structured subtype columns.
    pub config_json: String,
    pub credential_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConnectionConfig {
    /// SQLite is driver-specific enough to remain in advanced JSON while the
    /// subtype row carries common database metadata as columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sqlite_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTestResult {
    pub ok: bool,
    pub message: String,
    pub server_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSchema {
    pub connection_id: String,
    pub tables: Vec<DatabaseTable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTable {
    /// Top-level container the object lives in: PostgreSQL/MySQL database, or
    /// `None` for SQLite (single-file). Distinct from `schema`, which only
    /// PostgreSQL nests below the catalog.
    #[serde(default)]
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub name: String,
    pub kind: String,
    pub columns: Vec<DatabaseTableColumn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTableColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    #[serde(default)]
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseIndex {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTableStructureInput {
    pub workspace_id: String,
    pub connection_id: String,
    #[serde(default)]
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub table_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseCellValue {
    pub column: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseRowMutationInput {
    pub workspace_id: String,
    pub connection_id: String,
    #[serde(default)]
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub table_name: String,
    /// One of: "insert", "update", "delete".
    pub operation: String,
    /// Column values to write for insert/update operations.
    #[serde(default)]
    pub values: Vec<DatabaseCellValue>,
    /// Primary-key columns identifying the row for update/delete operations.
    #[serde(default)]
    pub primary_key: Vec<DatabaseCellValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseRowMutationResult {
    pub affected_rows: u64,
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTableStructure {
    #[serde(default)]
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub name: String,
    pub kind: String,
    pub columns: Vec<DatabaseTableColumn>,
    pub indexes: Vec<DatabaseIndex>,
    pub foreign_keys: Vec<DatabaseForeignKey>,
    pub ddl: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseQueryInput {
    pub workspace_id: String,
    pub connection_id: String,
    pub sql: String,
    pub limit: Option<u32>,
    pub confirm_mutation: Option<bool>,
    /// Optional query context: the catalog (PostgreSQL/MySQL database) the
    /// statement should run against. Applied before execution.
    #[serde(default)]
    pub catalog: Option<String>,
    /// Optional query context: the schema (PostgreSQL) the statement should
    /// resolve unqualified names against. Applied before execution.
    #[serde(default)]
    pub schema: Option<String>,
    /// Optional per-statement timeout in milliseconds. Clamped server-side; when
    /// absent a default timeout protects against runaway queries.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DbQueryHistoryEntry {
    pub id: String,
    pub workspace_id: String,
    pub connection_id: Option<String>,
    pub connection_name: String,
    pub sql: String,
    pub status: String,
    pub classification: Option<String>,
    pub row_count: Option<i64>,
    pub affected_rows: Option<i64>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbQueryHistoryRecordInput {
    pub id: String,
    pub workspace_id: String,
    pub connection_id: Option<String>,
    pub connection_name: String,
    pub sql: String,
    pub status: String,
    pub classification: Option<String>,
    pub row_count: Option<i64>,
    pub affected_rows: Option<i64>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SavedSql {
    pub id: String,
    pub workspace_id: String,
    pub connection_id: Option<String>,
    pub name: String,
    pub sql: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSqlInput {
    /// Present when updating an existing snippet; absent to create a new one.
    pub id: Option<String>,
    pub workspace_id: String,
    pub connection_id: Option<String>,
    pub name: String,
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBrowseInput {
    pub workspace_id: String,
    pub connection_id: String,
    #[serde(default)]
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub table_name: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Optional column to sort by, pushed into the browse query so it orders the
    /// whole table rather than only the loaded page.
    #[serde(default)]
    pub order_by: Option<String>,
    #[serde(default)]
    pub order_descending: bool,
    /// Optional case-insensitive text matched across every column, pushed into
    /// the browse query (and the total-row count) so it filters the whole table.
    #[serde(default)]
    pub filter: Option<String>,
    /// Optional per-statement timeout in milliseconds. Clamped server-side.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBrowseResult {
    pub table_name: String,
    pub sql: String,
    pub limit: u32,
    pub offset: u32,
    pub total_rows: u64,
    pub read_only: bool,
    pub result: DatabaseQueryResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseQueryResult {
    pub columns: Vec<DatabaseResultColumn>,
    pub rows: Vec<Vec<Option<String>>>,
    pub affected_rows: u64,
    pub duration_ms: u128,
    pub safety: DatabaseQuerySafety,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseQuerySafety {
    pub classification: String,
    pub requires_confirmation: bool,
    pub confirmed: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseResultColumn {
    pub name: String,
    pub data_type: String,
}
