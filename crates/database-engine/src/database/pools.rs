use super::*;

impl DatabaseService {
    /// Fresh runtime facts for the requested target, never a saved connection
    /// field. This engine API does not change UI, MCP, Flow or sync contracts.
    pub async fn runtime_profile(
        &self,
        workspace_id: String,
        connection_id: String,
        catalog: Option<String>,
    ) -> AppResult<RuntimeDatabaseProfile> {
        let connection = self.get_connection(&workspace_id, &connection_id).await?;
        let effective =
            Self::effective_connection(&connection, clean_identifier(catalog.as_deref())?);
        match connection.driver.as_str() {
            "sqlite" => Ok(sqlite_pool(&effective).await?.profile),
            "postgres" => Ok(self.postgres_pool(&effective).await?.profile),
            "mysql" => Ok(self.mysql_pool(&effective).await?.profile),
            driver => Err(AppError::Unsupported(format!(
                "unsupported database driver: {driver}"
            ))),
        }
    }

    /// Create a PostgreSQL connection pool, loading the password from SecretStore
    /// if a credential reference is present on the connection.
    pub(super) async fn postgres_pool(
        &self,
        connection: &DatabaseConnection,
    ) -> AppResult<RuntimePool<sqlx::Postgres>> {
        self.postgres_pool_with_secret(connection, None).await
    }

    /// PostgreSQL pool that prefers an inline `password_override` (the
    /// not-yet-saved secret from the "test connection" dialog) over the stored
    /// keychain credential. Falls back to the saved credential when no override
    /// is supplied, preserving existing behavior for saved connections.
    pub(super) async fn postgres_pool_with_secret(
        &self,
        connection: &DatabaseConnection,
        password_override: Option<&str>,
    ) -> AppResult<RuntimePool<sqlx::Postgres>> {
        let options =
            pg_connect_options(connection, self.secret_store.as_ref(), password_override).await?;
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(sanitize_pg_error)?;
        let version = probe_server_version(
            "postgres",
            sqlx::query_scalar("SELECT version()").fetch_one(&pool),
        )
        .await;
        let profile = match version {
            Some(version) => RuntimeDatabaseProfile::postgres(version),
            None => {
                RuntimeDatabaseProfile::resolve(DetectedServerType::UnknownPostgresCompatible, None)
            }
        };
        Ok(RuntimePool { pool, profile })
    }

    /// Create a MySQL connection pool, loading the password from SecretStore
    /// if a credential reference is present on the connection.
    pub(super) async fn mysql_pool(
        &self,
        connection: &DatabaseConnection,
    ) -> AppResult<RuntimePool<sqlx::MySql>> {
        self.mysql_pool_with_secret(connection, None).await
    }

    /// MySQL pool that prefers an inline `password_override` (the not-yet-saved
    /// secret from the "test connection" dialog) over the stored keychain
    /// credential. Falls back to the saved credential when no override is
    /// supplied, preserving existing behavior for saved connections.
    pub(super) async fn mysql_pool_with_secret(
        &self,
        connection: &DatabaseConnection,
        password_override: Option<&str>,
    ) -> AppResult<RuntimePool<sqlx::MySql>> {
        let options =
            mysql_connect_options(connection, self.secret_store.as_ref(), password_override)
                .await?;
        let pool = MySqlPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await
            .map_err(sanitize_mysql_error)?;
        let version = probe_server_version(
            "mysql",
            sqlx::query_scalar("SELECT VERSION()").fetch_one(&pool),
        )
        .await;
        Ok(RuntimePool {
            pool,
            profile: RuntimeDatabaseProfile::resolve(DetectedServerType::Mysql, version),
        })
    }

    /// Return a connection clone with `database` overridden to the given
    /// catalog when the catalog differs from the connection's current database.
    /// This is required for PostgreSQL (and MySQL) because they cannot
    /// cross-database query; the pool must target the database that owns the
    /// table being browsed, inspected, or mutated.
    pub(super) fn effective_connection(
        connection: &DatabaseConnection,
        catalog: Option<&str>,
    ) -> DatabaseConnection {
        match catalog {
            Some(name) if connection.database.as_deref() != Some(name) => {
                let mut overridden = connection.clone();
                overridden.database = Some(name.to_string());
                overridden
            }
            _ => connection.clone(),
        }
    }
}

/// Post-connect version probe only. Connect acquisition stays on the pool
/// timeout; a probe that never returns is degraded detection, not a failed
/// connection, and is not retried.
const SERVER_VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Run one version query. SQL errors and probe timeouts both yield `None`.
async fn probe_server_version<F>(driver: &str, probe: F) -> Option<String>
where
    F: std::future::Future<Output = Result<String, sqlx::Error>>,
{
    match tokio::time::timeout(SERVER_VERSION_PROBE_TIMEOUT, probe).await {
        Ok(Ok(version)) => Some(version),
        Ok(Err(error)) => {
            record_version_probe_failure(driver, error);
            None
        }
        Err(_) => {
            record_version_probe_timeout(driver);
            None
        }
    }
}

/// A failed version probe is degraded detection, not a connect failure.
fn record_version_probe_failure(driver: &str, error: sqlx::Error) {
    let sanitized = match driver {
        "mysql" => sanitize_mysql_error(error),
        _ => sanitize_pg_error(error),
    };
    unfour_diag::log_operation_event(
        "database_server_detection_degraded",
        "database",
        "detect_server_version",
        "degraded",
        None,
        Some(unfour_diag::app_error_kind(&sanitized)),
        serde_json::json!({ "driver": driver }),
    );
}

/// Probe deadline elapsed. Log only the driver and a stable reason — never the
/// connection target, DSN, or credential.
fn record_version_probe_timeout(driver: &str) {
    unfour_diag::log_operation_event(
        "database_server_detection_degraded",
        "database",
        "detect_server_version",
        "degraded",
        None,
        Some("timeout"),
        serde_json::json!({ "driver": driver, "reason": "timeout" }),
    );
}
