use super::*;

impl DatabaseService {
    pub async fn browse_table(
        &self,
        input: DatabaseBrowseInput,
    ) -> AppResult<DatabaseBrowseResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let table_name = input.table_name.trim();
        if table_name.is_empty() {
            return Err(AppError::Validation(
                "table name cannot be empty".to_string(),
            ));
        }

        let filter = normalize_filter(input.filter.as_deref());
        let order_by = input
            .order_by
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let descending = input.order_descending;
        let needs_columns = filter.is_some() || order_by.is_some();
        let timeout = resolve_timeout(input.timeout_ms);

        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;

        let run = async {
            match connection.driver.as_str() {
                "sqlite" => {
                    let pool = sqlite_pool(&connection).await?;
                    ensure_sqlite_table_exists(&pool, table_name).await?;

                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let offset = input.offset.unwrap_or(0);

                    let column_names = if needs_columns {
                        sqlite_columns(&pool, table_name)
                            .await?
                            .into_iter()
                            .map(|column| column.name)
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    let order_sql =
                        order_by_clause(order_by, descending, &column_names, quote_identifier)?;
                    let active_filter = filter.as_ref().filter(|_| !column_names.is_empty());
                    let where_sql = active_filter
                        .map(|_| format!(" WHERE {}", sqlite_filter_where(&column_names)))
                        .unwrap_or_default();
                    let quoted = quote_identifier(table_name);

                    let total_rows = if let Some(needle) = active_filter {
                        let count_sql =
                            format!("SELECT COUNT(*) AS total_rows FROM {}{}", quoted, where_sql);
                        let mut count = sqlx::query(&count_sql);
                        for _ in &column_names {
                            count = count.bind(format!("%{}%", needle));
                        }
                        let row = count.fetch_one(&pool).await?;
                        row.try_get::<i64, _>("total_rows")?.max(0) as u64
                    } else {
                        sqlite_table_row_count(&pool, table_name).await?
                    };

                    let sql = format!(
                        "SELECT * FROM {}{}{} LIMIT {} OFFSET {}",
                        quoted, where_sql, order_sql, limit, offset
                    );
                    let started = Instant::now();
                    let mut query = sqlx::query(&sql);
                    if let Some(needle) = active_filter {
                        for _ in &column_names {
                            query = query.bind(format!("%{}%", needle));
                        }
                    }
                    let rows = query.fetch_all(&pool).await?;
                    let columns = if let Some(row) = rows.first() {
                        sqlite_result_columns(row)
                    } else {
                        sqlite_table_result_columns(&pool, table_name).await?
                    };
                    let values = rows
                        .iter()
                        .map(sqlite_row_values)
                        .collect::<AppResult<Vec<_>>>()?;

                    Ok(browse_result(
                        table_name, sql, limit, offset, total_rows, columns, values, started,
                    ))
                }
                "postgres" => {
                    let effective =
                        Self::effective_connection(&connection, input.catalog.as_deref());
                    let pool = self.postgres_pool(&effective).await?;
                    let schema = input
                        .schema
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or("public");
                    ensure_postgres_table_exists(&pool, schema, table_name)
                        .await
                        .map_err(sanitize_pg_app_error)?;

                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let offset = input.offset.unwrap_or(0);

                    let column_names = if needs_columns {
                        postgres_columns(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_pg_app_error)?
                            .into_iter()
                            .map(|column| column.name)
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    let order_sql =
                        order_by_clause(order_by, descending, &column_names, quote_identifier)?;
                    let active_filter = filter.as_ref().filter(|_| !column_names.is_empty());
                    let where_sql = active_filter
                        .map(|_| format!(" WHERE {}", postgres_filter_where(&column_names)))
                        .unwrap_or_default();

                    let total_rows = if let Some(needle) = active_filter {
                        // The same $1 bind is reused by every column predicate.
                        let count_sql = format!(
                            "SELECT COUNT(*) AS total_rows FROM {}{}",
                            quote_qualified_identifier(schema, table_name),
                            where_sql
                        );
                        let row = sqlx::query(&count_sql)
                            .bind(format!("%{}%", needle))
                            .fetch_one(&pool)
                            .await
                            .map_err(sanitize_pg_error)?;
                        row.try_get::<i64, _>("total_rows")
                            .map_err(sanitize_pg_error)?
                            .max(0) as u64
                    } else {
                        postgres_table_row_count(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_pg_app_error)?
                    };

                    let sql = postgres_browse_sql(
                        schema, table_name, &where_sql, &order_sql, limit, offset,
                    );
                    let started = Instant::now();
                    let mut query = sqlx::query(&sql);
                    if let Some(needle) = active_filter {
                        query = query.bind(format!("%{}%", needle));
                    }
                    let rows = query.fetch_all(&pool).await.map_err(sanitize_pg_error)?;
                    let columns = if let Some(row) = rows.first() {
                        postgres_result_columns(row)
                    } else {
                        postgres_table_result_columns(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_pg_app_error)?
                    };
                    let values = rows
                        .iter()
                        .map(postgres_row_values)
                        .collect::<AppResult<Vec<_>>>()?;

                    Ok(browse_result(
                        table_name, sql, limit, offset, total_rows, columns, values, started,
                    ))
                }
                "mysql" => {
                    let effective =
                        Self::effective_connection(&connection, input.catalog.as_deref());
                    let pool = self.mysql_pool(&effective).await?;
                    // MySQL addresses tables as `database`.`table`; the catalog is
                    // that database. Prefer the explicit catalog, then the legacy
                    // schema field, then the connection's default database.
                    let schema = input
                        .catalog
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .or_else(|| {
                            input
                                .schema
                                .as_deref()
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                        })
                        .or(connection.database.as_deref())
                        .ok_or_else(|| {
                            AppError::Validation("MySQL database name is required".to_string())
                        })?;
                    ensure_mysql_table_exists(&pool, schema, table_name)
                        .await
                        .map_err(sanitize_mysql_app_error)?;

                    let limit = input.limit.unwrap_or(100).clamp(1, 1_000);
                    let offset = input.offset.unwrap_or(0);

                    let column_names = if needs_columns {
                        mysql_columns(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_mysql_app_error)?
                            .into_iter()
                            .map(|column| column.name)
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };
                    let order_sql = order_by_clause(
                        order_by,
                        descending,
                        &column_names,
                        quote_mysql_identifier,
                    )?;
                    let active_filter = filter.as_ref().filter(|_| !column_names.is_empty());
                    let where_sql = active_filter
                        .map(|_| format!(" WHERE {}", mysql_filter_where(&column_names)))
                        .unwrap_or_default();

                    let total_rows = if let Some(needle) = active_filter {
                        let count_sql = format!(
                            "SELECT COUNT(*) AS total_rows FROM {}{}",
                            quote_mysql_qualified_identifier(schema, table_name),
                            where_sql
                        );
                        let mut count = sqlx::query(&count_sql);
                        for _ in &column_names {
                            count = count.bind(format!("%{}%", needle));
                        }
                        let row = count.fetch_one(&pool).await.map_err(sanitize_mysql_error)?;
                        row.try_get::<i64, _>("total_rows")
                            .map_err(sanitize_mysql_error)?
                            .max(0) as u64
                    } else {
                        mysql_table_row_count(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_mysql_app_error)?
                    };

                    let sql =
                        mysql_browse_sql(schema, table_name, &where_sql, &order_sql, limit, offset);
                    let started = Instant::now();
                    let mut query = sqlx::query(&sql);
                    if let Some(needle) = active_filter {
                        for _ in &column_names {
                            query = query.bind(format!("%{}%", needle));
                        }
                    }
                    let rows = query.fetch_all(&pool).await.map_err(sanitize_mysql_error)?;
                    let columns = if let Some(row) = rows.first() {
                        mysql_result_columns(row)
                    } else {
                        mysql_table_result_columns(&pool, schema, table_name)
                            .await
                            .map_err(sanitize_mysql_app_error)?
                    };
                    let values = rows
                        .iter()
                        .map(mysql_row_values)
                        .collect::<AppResult<Vec<_>>>()?;

                    Ok(browse_result(
                        table_name, sql, limit, offset, total_rows, columns, values, started,
                    ))
                }
                driver => Err(AppError::Unsupported(format!(
                    "{} table browsing is not yet supported",
                    display_driver(driver)
                ))),
            }
        };
        match tokio::time::timeout(timeout, run).await {
            Ok(result) => result,
            Err(_) => Err(AppError::Timeout(format!(
                "table browse exceeded the {} ms timeout",
                timeout.as_millis()
            ))),
        }
    }

    /// Load the full structure (columns, indexes, foreign keys, DDL) for a
    /// single table on demand. Kept separate from `schema` so browsing the
    /// connection tree stays lightweight.
    pub async fn table_structure(
        &self,
        input: DatabaseTableStructureInput,
    ) -> AppResult<DatabaseTableStructure> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let table_name = input.table_name.trim();
        if table_name.is_empty() {
            return Err(AppError::Validation(
                "table name cannot be empty".to_string(),
            ));
        }

        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;

        match connection.driver.as_str() {
            "sqlite" => {
                let pool = sqlite_pool(&connection).await?;
                ensure_sqlite_table_exists(&pool, table_name).await?;
                let columns = sqlite_columns(&pool, table_name).await?;
                let indexes = sqlite_indexes(&pool, table_name).await?;
                let foreign_keys = sqlite_foreign_keys(&pool, table_name).await?;
                let kind = sqlite_table_kind(&pool, table_name).await?;
                let ddl = sqlite_ddl(&pool, table_name).await?;
                Ok(DatabaseTableStructure {
                    catalog: None,
                    schema: None,
                    name: table_name.to_string(),
                    kind,
                    columns,
                    indexes,
                    foreign_keys,
                    ddl,
                })
            }
            "postgres" => {
                let effective = Self::effective_connection(&connection, input.catalog.as_deref());
                let pool = self.postgres_pool(&effective).await?;
                let schema = input
                    .schema
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("public");
                ensure_postgres_table_exists(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                let columns = postgres_columns(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                let indexes = postgres_indexes(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                let foreign_keys = postgres_foreign_keys(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                // Resolve the actual object kind (table vs view) from
                // information_schema so views report as "view" instead of the
                // hard-coded "table" the previous implementation returned.
                let kind = postgres_table_kind(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                let ddl = postgres_ddl(&pool, schema, table_name, &kind)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                Ok(DatabaseTableStructure {
                    catalog: connection.database.clone(),
                    schema: Some(schema.to_string()),
                    name: table_name.to_string(),
                    kind,
                    columns,
                    indexes,
                    foreign_keys,
                    ddl: Some(ddl),
                })
            }
            "mysql" => {
                let effective = Self::effective_connection(&connection, input.catalog.as_deref());
                let pool = self.mysql_pool(&effective).await?;
                // MySQL addresses tables as `database`.`table`; the catalog is
                // that database. Prefer the explicit catalog, then the legacy
                // schema field, then the connection's default database.
                let schema = input
                    .catalog
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .or_else(|| {
                        input
                            .schema
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                    })
                    .or(connection.database.as_deref())
                    .ok_or_else(|| {
                        AppError::Validation("MySQL database name is required".to_string())
                    })?;
                ensure_mysql_table_exists(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                let columns = mysql_columns(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                let indexes = mysql_indexes(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                let foreign_keys = mysql_foreign_keys(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                let ddl = mysql_ddl(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                // Resolve the actual object kind (table vs view) from
                // information_schema so views report as "view" instead of the
                // hard-coded "table" the previous implementation returned.
                let kind = mysql_table_kind(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_mysql_app_error)?;
                Ok(DatabaseTableStructure {
                    catalog: Some(schema.to_string()),
                    schema: None,
                    name: table_name.to_string(),
                    kind,
                    columns,
                    indexes,
                    foreign_keys,
                    ddl,
                })
            }
            driver => Err(AppError::Unsupported(format!(
                "{} table structure is not yet supported",
                display_driver(driver)
            ))),
        }
    }
}
