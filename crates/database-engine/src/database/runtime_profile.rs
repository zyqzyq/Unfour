//! Runtime facts, never connection configuration or Cloud Sync data.
//! Add product detection and metadata overrides here/at the metadata boundary,
//! not another transport driver. Intentionally no serde or persistence traits.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseDialect {
    Sqlite,
    Postgres,
    Mysql,
    Generic,
}

impl DatabaseDialect {
    pub fn quote_identifier(self, value: &str) -> String {
        let quote = if self == Self::Mysql { '`' } else { '"' };
        format!(
            "{quote}{}{quote}",
            value.replace(quote, &format!("{quote}{quote}"))
        )
    }

    /// Offline preflight (Flow/editor) cannot detect a server. Use the protocol's
    /// baseline grammar; connected operations use the runtime profile.
    pub fn for_driver(driver: &str) -> Self {
        match driver {
            "sqlite" => Self::Sqlite,
            "postgres" => Self::Postgres,
            "mysql" => Self::Mysql,
            _ => Self::Generic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedServerType {
    Sqlite,
    PostgreSql,
    OpenGauss,
    Mysql,
    UnknownPostgresCompatible,
}

/// Supported engine operations, not authorization, object permissions, or a
/// promise that every SQL statement will work. Existing safety gates still apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseCapabilities {
    /// Can discover server catalogs beyond the configured connection target.
    pub catalogs: bool,
    pub schemas: bool,
    pub indexes: bool,
    pub foreign_keys: bool,
    pub ddl: bool,
    pub generated_columns: bool,
    pub row_mutation: bool,
    pub export: bool,
    pub explain: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDatabaseProfile {
    pub detected_server_type: DetectedServerType,
    /// Original server-reported version/banner, retained only in memory.
    /// `None` means the version probe failed or was unavailable. Absence is not
    /// a placeholder string, and it is not a connection failure.
    pub server_version: Option<String>,
    pub dialect: DatabaseDialect,
    pub capabilities: DatabaseCapabilities,
}

impl RuntimeDatabaseProfile {
    pub(super) fn server_name(&self) -> &'static str {
        match self.detected_server_type {
            DetectedServerType::Sqlite => "SQLite",
            DetectedServerType::PostgreSql => "PostgreSQL",
            DetectedServerType::OpenGauss => "openGauss",
            DetectedServerType::Mysql => "MySQL",
            DetectedServerType::UnknownPostgresCompatible => "UnknownPostgresCompatible",
        }
    }

    pub(super) fn postgres(version: String) -> Self {
        Self::resolve(detect_postgres_server(&version), Some(version))
    }

    pub(super) fn resolve(server: DetectedServerType, version: Option<String>) -> Self {
        use DetectedServerType::*;
        // Adding a detected product requires an explicit compatibility policy.
        let (dialect, catalogs, schemas, full_metadata) = match server {
            Sqlite => (DatabaseDialect::Sqlite, false, false, true),
            Mysql => (DatabaseDialect::Mysql, true, false, true),
            OpenGauss => (DatabaseDialect::Postgres, true, true, false),
            PostgreSql => (DatabaseDialect::Postgres, true, true, true),
            UnknownPostgresCompatible => (DatabaseDialect::Postgres, false, true, false),
        };
        // openGauss core metadata has a dedicated strategy. Optional PostgreSQL
        // index/FK/DDL queries are not certified for it and remain disabled.
        Self {
            detected_server_type: server,
            server_version: version,
            dialect,
            capabilities: DatabaseCapabilities {
                catalogs,
                schemas,
                indexes: full_metadata,
                foreign_keys: full_metadata,
                ddl: full_metadata,
                generated_columns: full_metadata || server == OpenGauss,
                row_mutation: full_metadata || server == OpenGauss,
                export: full_metadata || server == OpenGauss,
                explain: true,
            },
        }
    }
}

/// Recognize PostgreSQL's leading product token and numeric release, never a
/// substring such as "compatible with PostgreSQL". Fork markers take priority:
/// several products embed the upstream banner. A banner is evidence, not proof
/// of identity; future products can add stronger probes before this fallback.
pub(super) fn detect_postgres_server(version: &str) -> DetectedServerType {
    let lower = version.trim().to_ascii_lowercase();
    if lower.contains("opengauss") {
        return DetectedServerType::OpenGauss;
    }
    let fork = ["gaussdb", "cockroach", "yugabyte", "redshift", "greenplum"]
        .iter()
        .any(|marker| lower.contains(marker));
    let release = lower
        .strip_prefix("postgresql ")
        .and_then(|s| s.split_whitespace().next());
    if !fork
        && release.is_some_and(|v| {
            let mut parts = v.split('.');
            parts
                .next()
                .is_some_and(|major| !major.is_empty() && major.bytes().all(|b| b.is_ascii_digit()))
                && parts
                    .next()
                    .is_some_and(|minor| minor.starts_with(|c: char| c.is_ascii_digit()))
        })
    {
        DetectedServerType::PostgreSql
    } else {
        DetectedServerType::UnknownPostgresCompatible
    }
}

/// An operation-scoped pool and facts detected on that pool. No cache keyed by
/// saved connection id: edits, catalog changes and reconnects detect anew.
pub(super) struct RuntimePool<DB: sqlx::Database> {
    pub pool: sqlx::Pool<DB>,
    pub profile: RuntimeDatabaseProfile,
}

impl<DB: sqlx::Database> std::ops::Deref for RuntimePool<DB> {
    type Target = sqlx::Pool<DB>;
    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

pub(super) fn require_capability(supported: bool, operation: &str) -> unfour_core::AppResult<()> {
    if supported {
        Ok(())
    } else {
        Err(unfour_core::AppError::Unsupported(format!(
            "runtime database does not support {operation}"
        )))
    }
}
