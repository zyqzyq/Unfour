use super::*;

impl DatabaseService {
    pub async fn test_connection(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<DatabaseTestResult> {
        let connection = self.get_connection(&workspace_id, &connection_id).await?;
        self.test_connection_inner(connection, None).await
    }

    /// Test connectivity for a connection that may not yet be saved. The
    /// transient `secret` (the password typed in the dialog) is used as an
    /// override; when empty, the stored keychain credential referenced by
    /// `credential_ref` is used instead. This lets the "test connection" action
    /// validate a brand-new connection before it is persisted.
    pub async fn test_connection_input(
        &self,
        input: DatabaseConnectionInput,
        secret: Option<String>,
    ) -> AppResult<DatabaseTestResult> {
        validate_workspace_id(&input.workspace_id)?;
        let storage = input_to_storage(&input)?;
        let connection = DatabaseConnection {
            id: input.id.clone().unwrap_or_default(),
            workspace_id: input.workspace_id.clone(),
            name: input.name.trim().to_string(),
            driver: storage.driver.clone(),
            host: storage.host.clone(),
            port: storage.port,
            database: storage.database_name.clone(),
            username: storage.username.clone(),
            ssl_mode: storage.ssl_mode.clone(),
            sqlite_path: storage.config.sqlite_path.clone(),
            credential_ref: empty_to_none(input.credential_ref.clone()),
            read_only: storage.read_only,
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
            revision: 0,
            sync_status: "new".to_string(),
            remote_id: None,
        };
        let password_override = secret.as_deref().filter(|value| !value.is_empty());
        self.test_connection_inner(connection, password_override)
            .await
    }

    async fn test_connection_inner(
        &self,
        connection: DatabaseConnection,
        password_override: Option<&str>,
    ) -> AppResult<DatabaseTestResult> {
        let started = Instant::now();
        let fields = serde_json::json!({ "driver": &connection.driver });
        unfour_diag::log_operation_event(
            "database_connect_started",
            "database",
            "test_connection",
            "started",
            None,
            None,
            fields.clone(),
        );

        let result = match connection.driver.as_str() {
            "sqlite" => {
                let pool = sqlite_pool(&connection).await?;
                Ok(DatabaseTestResult {
                    ok: true,
                    message: "SQLite connection OK".to_string(),
                    server_version: pool.profile.server_version.clone(),
                    protocol: Some(connection.driver.clone()),
                    detected_server: Some(pool.profile.server_name().into()),
                })
            }
            "postgres" => {
                let pool = self
                    .postgres_pool_with_secret(&connection, password_override)
                    .await?;
                Ok(DatabaseTestResult {
                    ok: true,
                    message: "PostgreSQL connection OK".to_string(),
                    server_version: pool.profile.server_version.clone(),
                    protocol: Some(connection.driver.clone()),
                    detected_server: Some(pool.profile.server_name().into()),
                })
            }
            "mysql" => {
                let pool = self
                    .mysql_pool_with_secret(&connection, password_override)
                    .await?;
                Ok(DatabaseTestResult {
                    ok: true,
                    message: "MySQL connection OK".to_string(),
                    server_version: pool.profile.server_version.clone(),
                    protocol: Some(connection.driver.clone()),
                    detected_server: Some(pool.profile.server_name().into()),
                })
            }
            driver => Err(AppError::Unsupported(format!(
                "database driver is not supported: {}",
                driver
            ))),
        };

        match &result {
            Ok(_) => unfour_diag::log_operation_event(
                "database_connect_completed",
                "database",
                "test_connection",
                "ok",
                Some(started.elapsed().as_millis()),
                None,
                fields,
            ),
            Err(error) => unfour_diag::log_operation_event(
                "database_connect_failed",
                "database",
                "test_connection",
                "error",
                Some(started.elapsed().as_millis()),
                Some(unfour_diag::app_error_kind(error)),
                fields,
            ),
        }
        result
    }

    pub async fn schema(
        &self,
        workspace_id: String,
        connection_id: String,
        catalog: Option<String>,
    ) -> AppResult<DatabaseSchema> {
        let connection = self.get_connection(&workspace_id, &connection_id).await?;
        let catalog = clean_identifier(catalog.as_deref())?.map(str::to_string);

        match connection.driver.as_str() {
            "sqlite" => {
                let pool = sqlite_pool(&connection).await?;
                let table_rows = sqlx::query(
                    r#"
                    SELECT name, type
                    FROM sqlite_master
                    WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
                    ORDER BY type, name
                    "#,
                )
                .fetch_all(&*pool)
                .await?;

                let mut tables = Vec::with_capacity(table_rows.len());
                for row in table_rows {
                    let name: String = row.try_get("name")?;
                    let kind: String = row.try_get("type")?;
                    let columns = sqlite_columns(&pool, &name).await?;
                    tables.push(DatabaseTable {
                        catalog: None,
                        schema: None,
                        name,
                        kind,
                        columns,
                    });
                }

                Ok(DatabaseSchema {
                    connection_id,
                    tables,
                })
            }
            "postgres" => {
                // PostgreSQL cannot cross-database query, so to browse a catalog
                // other than the connection default we open a pool bound to that
                // database. Every object in the listing belongs to this catalog.
                let effective = Self::effective_connection(&connection, catalog.as_deref());
                let pool = self.postgres_pool(&effective).await?;
                let catalog = effective.database.clone();
                let table_rows = sqlx::query(
                    r#"
                    SELECT table_schema, table_name, table_type
                    FROM information_schema.tables
                    WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
                    ORDER BY table_schema, table_name
                    "#,
                )
                .fetch_all(&*pool)
                .await
                .map_err(sanitize_pg_error)?;

                let mut tables = Vec::with_capacity(table_rows.len());
                for row in table_rows {
                    let schema: String = row.try_get("table_schema").map_err(sanitize_pg_error)?;
                    let name: String = row.try_get("table_name").map_err(sanitize_pg_error)?;
                    let table_type: String =
                        row.try_get("table_type").map_err(sanitize_pg_error)?;
                    let kind = if table_type == "VIEW" {
                        "view".to_string()
                    } else {
                        "table".to_string()
                    };
                    let columns = postgres_columns(&pool, &schema, &name)
                        .await
                        .map_err(sanitize_pg_app_error)?;
                    tables.push(postgres_table_from_metadata(
                        catalog.clone(),
                        schema,
                        name,
                        kind,
                        columns,
                    ));
                }

                Ok(DatabaseSchema {
                    connection_id,
                    tables,
                })
            }
            "mysql" => {
                let pool = self.mysql_pool(&connection).await?;
                // Scope to one database (catalog) when given; bound as a
                // parameter so an identifier with quotes cannot break the query.
                // When a specific catalog is requested we list its tables even if
                // it is a system schema (the user opened it explicitly). The
                // unscoped "load everything" path still skips the system schemas
                // so selecting a connection does not eagerly pull them all in.
                let mut sql = String::from(
                    "SELECT table_schema, table_name, table_type FROM information_schema.tables",
                );
                if catalog.is_some() {
                    sql.push_str(" WHERE table_schema = ?");
                } else {
                    sql.push_str(
                        " WHERE table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys')",
                    );
                }
                sql.push_str(" ORDER BY table_schema, table_name");
                let mut query = sqlx::query(&sql);
                if let Some(cat) = catalog.as_deref() {
                    query = query.bind(cat);
                }
                let table_rows = query
                    .fetch_all(&*pool)
                    .await
                    .map_err(sanitize_mysql_error)?;

                let mut tables = Vec::with_capacity(table_rows.len());
                for row in table_rows {
                    // Read positionally (table_schema, table_name, table_type)
                    // and tolerate the binary charset MySQL reports for
                    // information_schema columns.
                    let schema: String = mysql_text(&row, 0).map_err(sanitize_mysql_app_error)?;
                    let name: String = mysql_text(&row, 1).map_err(sanitize_mysql_app_error)?;
                    let table_type: String =
                        mysql_text(&row, 2).map_err(sanitize_mysql_app_error)?;
                    let columns = mysql_columns(&pool, &schema, &name)
                        .await
                        .map_err(sanitize_mysql_app_error)?;
                    // In MySQL `table_schema` is the database itself, so it maps
                    // to the catalog level; MySQL has no nested schema.
                    tables.push(mysql_table_from_metadata(schema, name, table_type, columns));
                }

                Ok(DatabaseSchema {
                    connection_id,
                    tables,
                })
            }
            driver => Err(AppError::Unsupported(format!(
                "{} schema browsing is not yet supported",
                display_driver(driver)
            ))),
        }
    }

    /// List the catalogs (databases) the connection can see. SQLite returns an
    /// empty list because a connection is a single file. PostgreSQL and MySQL
    /// enumerate the server's databases so the tree can browse beyond the
    /// connection's default database, one catalog at a time.
    pub async fn list_catalogs(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> AppResult<Vec<String>> {
        let connection = self.get_connection(&workspace_id, &connection_id).await?;

        match connection.driver.as_str() {
            "sqlite" => Ok(Vec::new()),
            "postgres" => {
                let pool = self.postgres_pool(&connection).await?;
                if !pool.profile.capabilities.catalogs {
                    // The connected target remains browsable even when server
                    // catalog discovery is unavailable. UI needs no product branch.
                    return Ok(connection.database.clone().into_iter().collect());
                }
                let rows = sqlx::query(
                    r#"
                    SELECT datname
                    FROM pg_database
                    WHERE datistemplate = false AND datallowconn = true
                    ORDER BY datname
                    "#,
                )
                .fetch_all(&*pool)
                .await
                .map_err(sanitize_pg_error)?;
                rows.into_iter()
                    .map(|row| row.try_get::<String, _>("datname").map_err(AppError::from))
                    .collect()
            }
            "mysql" => {
                let pool = self.mysql_pool(&connection).await?;
                // List every schema, including the system databases
                // (information_schema, mysql, performance_schema, sys), so they
                // are browsable from the tree like any other database.
                if !pool.profile.capabilities.catalogs {
                    return Ok(Vec::new());
                }
                let rows = sqlx::query(
                    r#"
                    SELECT schema_name
                    FROM information_schema.schemata
                    ORDER BY schema_name
                    "#,
                )
                .fetch_all(&*pool)
                .await
                .map_err(sanitize_mysql_error)?;
                rows.iter()
                    // Read positionally and tolerate the binary charset MySQL
                    // reports for information_schema columns (a by-name
                    // "schema_name" lookup can also miss the uppercase column).
                    .map(|row| mysql_text(row, 0))
                    .collect()
            }
            driver => Err(AppError::Unsupported(format!(
                "{} catalog listing is not yet supported",
                display_driver(driver)
            ))),
        }
    }
}
