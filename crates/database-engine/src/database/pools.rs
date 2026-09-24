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
        let version: String = sqlx::query_scalar("SELECT version()")
            .fetch_one(&pool)
            .await
            .map_err(sanitize_pg_error)?;
        Ok(RuntimePool {
            pool,
            profile: RuntimeDatabaseProfile::postgres(version),
        })
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
        let version: String = sqlx::query_scalar("SELECT VERSION()")
            .fetch_one(&pool)
            .await
            .map_err(sanitize_mysql_error)?;
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
