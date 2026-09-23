use super::*;

pub(super) async fn mysql_connect_options(
    connection: &DatabaseConnection,
    secret_store: Option<&SecretStore>,
    password_override: Option<&str>,
) -> AppResult<MySqlConnectOptions> {
    let host = connection
        .host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("127.0.0.1");
    let port = connection.port.unwrap_or(3306);
    // The default database is optional: a server-level connection can browse
    // every database it can see and pick one as the active query context later.
    let database = connection
        .database
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let username = connection
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("MySQL username is required".to_string()))?;
    let password = match password_override {
        Some(secret) => Some(secret.to_string()),
        None => resolve_database_password(connection, secret_store).await?,
    };

    let mut options = MySqlConnectOptions::new()
        .host(host)
        .port(port)
        .username(username);
    if let Some(database) = database {
        options = options.database(database);
    }
    if let Some(password) = password {
        options = options.password(&password);
    }
    Ok(options)
}

/// Read a MySQL text column positionally, tolerating the binary character set
/// that MySQL/MariaDB often report for `information_schema` columns: sqlx
/// refuses to decode those as `String`, so fall back to raw bytes.
pub(super) fn mysql_text(row: &sqlx::mysql::MySqlRow, index: usize) -> Result<String, AppError> {
    match row.try_get::<String, _>(index) {
        Ok(value) => Ok(value),
        Err(_) => {
            let bytes: Vec<u8> = row.try_get(index)?;
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        }
    }
}

/// Nullable counterpart to [`mysql_text`] for columns such as `column_default`.
pub(super) fn mysql_text_opt(
    row: &sqlx::mysql::MySqlRow,
    index: usize,
) -> Result<Option<String>, AppError> {
    match row.try_get::<Option<String>, _>(index) {
        Ok(value) => Ok(value),
        Err(_) => {
            let bytes: Option<Vec<u8>> = row.try_get(index)?;
            Ok(bytes.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
        }
    }
}

pub(super) async fn mysql_columns(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseTableColumn>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT column_name, column_type, is_nullable, column_key, column_default, extra
        FROM information_schema.columns
        WHERE table_schema = ? AND table_name = ?
        ORDER BY ordinal_position
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    rows.iter()
        .map(|row| {
            // Read positionally (column_name, column_type, is_nullable,
            // column_key, column_default) and tolerate the binary charset MySQL
            // reports for information_schema columns.
            let name: String = mysql_text(row, 0)?;
            let data_type: String = mysql_text(row, 1)?;
            let is_nullable: String = mysql_text(row, 2)?;
            let column_key: String = mysql_text(row, 3)?;
            let default_value: Option<String> = mysql_text_opt(row, 4)?;
            let extra: String = mysql_text(row, 5)?;
            let normalized_extra = extra.to_ascii_lowercase();
            Ok(DatabaseTableColumn {
                name,
                data_type,
                nullable: is_nullable == "YES",
                primary_key: column_key == "PRI",
                default_value,
                generated: normalized_extra.contains("generated"),
                auto_increment: normalized_extra.contains("auto_increment"),
            })
        })
        .collect()
}

pub(super) async fn mysql_indexes(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseIndex>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT index_name, non_unique, seq_in_index, column_name
        FROM information_schema.statistics
        WHERE table_schema = ? AND table_name = ?
        ORDER BY index_name, seq_in_index
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut indexes: Vec<DatabaseIndex> = Vec::new();
    for row in &rows {
        // Read positionally (index_name, non_unique, seq_in_index, column_name)
        // and tolerate the binary charset MySQL reports for information_schema
        // text columns. NON_UNIQUE widened to BIGINT in MySQL 8, so accept i32.
        let name: String = mysql_text(row, 0)?;
        let non_unique: i64 = row
            .try_get::<i64, _>(1)
            .or_else(|_| row.try_get::<i32, _>(1).map(i64::from))?;
        let column_name: String = mysql_text(row, 3)?;

        if let Some(existing) = indexes.iter_mut().find(|idx| idx.name == name) {
            existing.columns.push(column_name);
        } else {
            indexes.push(DatabaseIndex {
                primary: name == "PRIMARY",
                unique: non_unique == 0,
                name,
                columns: vec![column_name],
            });
        }
    }

    Ok(indexes)
}

pub(super) async fn mysql_foreign_keys(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseForeignKey>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT constraint_name, column_name, referenced_table_name, referenced_column_name
        FROM information_schema.key_column_usage
        WHERE table_schema = ? AND table_name = ? AND referenced_table_name IS NOT NULL
        ORDER BY constraint_name, ordinal_position
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut keys: Vec<DatabaseForeignKey> = Vec::new();
    for row in &rows {
        // Read positionally (constraint_name, column_name,
        // referenced_table_name, referenced_column_name) and tolerate the binary
        // charset MySQL reports for information_schema columns.
        let name: String = mysql_text(row, 0)?;
        let column_name: String = mysql_text(row, 1)?;
        let referenced_table: String = mysql_text(row, 2)?;
        let referenced_column: String = mysql_text(row, 3)?;

        if let Some(existing) = keys.iter_mut().find(|fk| fk.name == name) {
            existing.columns.push(column_name);
            existing.referenced_columns.push(referenced_column);
        } else {
            keys.push(DatabaseForeignKey {
                name,
                columns: vec![column_name],
                referenced_table,
                referenced_columns: vec![referenced_column],
            });
        }
    }

    Ok(keys)
}

pub(super) async fn mysql_ddl(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Option<String>, AppError> {
    let sql = format!(
        "SHOW CREATE TABLE {}",
        quote_mysql_qualified_identifier(schema, table_name)
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    // SHOW CREATE TABLE returns (Table, Create Table); read the DDL positionally
    // so the result is not tied to the server's column-name casing.
    Ok(Some(row.try_get::<String, _>(1)?))
}

pub(super) async fn ensure_mysql_table_exists(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<(), AppError> {
    // Select a literal rather than table_name: MySQL returns information_schema
    // text columns as VARBINARY, which cannot decode into a Rust String. We only
    // need to know whether the row exists.
    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT 1
        FROM information_schema.tables
        WHERE table_schema = ? AND table_name = ?
        LIMIT 1
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_optional(pool)
    .await?;

    row.map(|_| ())
        .ok_or_else(|| AppError::NotFound(format!("{schema}.{table_name}")))
}

/// Resolve the object kind ("table" or "view") from information_schema so the
/// structure panel reports views correctly instead of always returning
/// "table". Reads the `table_type` column positionally and tolerates the
/// binary charset MySQL reports for information_schema text columns. Falls
/// back to "table" when the row is missing (the caller has already verified
/// existence via `ensure_mysql_table_exists`).
pub(super) async fn mysql_table_kind(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<String, AppError> {
    let row = sqlx::query(
        r#"
        SELECT table_type
        FROM information_schema.tables
        WHERE table_schema = ? AND table_name = ?
        LIMIT 1
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|row| {
            let table_type = mysql_text(&row, 0).unwrap_or_default();
            if table_type.eq_ignore_ascii_case("VIEW") {
                "view".to_string()
            } else {
                "table".to_string()
            }
        })
        .unwrap_or_else(|| "table".to_string()))
}

pub(super) async fn mysql_table_row_count(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<u64, AppError> {
    let sql = format!(
        "SELECT COUNT(*) AS total_rows FROM {}",
        quote_mysql_qualified_identifier(schema, table_name)
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let total_rows: i64 = row.try_get("total_rows")?;
    Ok(total_rows.max(0) as u64)
}

pub(super) async fn mysql_table_result_columns(
    pool: &sqlx::MySqlPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseResultColumn>, AppError> {
    Ok(mysql_columns(pool, schema, table_name)
        .await?
        .into_iter()
        .map(|column| DatabaseResultColumn {
            name: column.name,
            data_type: column.data_type,
        })
        .collect())
}

pub(super) fn mysql_result_columns(row: &sqlx::mysql::MySqlRow) -> Vec<DatabaseResultColumn> {
    row.columns()
        .iter()
        .map(|column| DatabaseResultColumn {
            name: column.name().to_string(),
            data_type: column.type_info().name().to_string(),
        })
        .collect()
}

pub(super) fn mysql_table_from_metadata(
    catalog: String,
    name: String,
    table_type: String,
    columns: Vec<DatabaseTableColumn>,
) -> DatabaseTable {
    DatabaseTable {
        catalog: Some(catalog),
        schema: None,
        name,
        kind: if table_type == "VIEW" {
            "view".to_string()
        } else {
            "table".to_string()
        },
        columns,
    }
}

pub(super) fn mysql_row_values(row: &sqlx::mysql::MySqlRow) -> AppResult<Vec<Option<String>>> {
    (0..row.columns().len())
        .map(|index| {
            let raw = row.try_get_raw(index)?;
            if raw.is_null() {
                return Ok(None);
            }

            if let Ok(value) = row.try_get::<String, _>(index) {
                return Ok(Some(value));
            }
            if let Ok(value) = row.try_get::<i64, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<u64, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<i32, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<u32, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<f64, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<f32, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<bool, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveDateTime, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveDate, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::NaiveTime, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
                return Ok(Some(format!("<binary {} bytes>", value.len())));
            }
            if let Ok(value) = row.try_get::<serde_json::Value, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<uuid::Uuid, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(index) {
                return Ok(Some(value.to_rfc3339()));
            }
            if let Ok(value) = row.try_get::<chrono::DateTime<chrono::Local>, _>(index) {
                return Ok(Some(value.to_rfc3339()));
            }

            Ok(Some("<unsupported>".to_string()))
        })
        .collect()
}

pub(super) fn sanitize_mysql_error(error: sqlx::Error) -> AppError {
    let message = error.to_string();
    let safe = redact_connection_string(&message);
    if safe == message {
        AppError::Database(error)
    } else {
        AppError::Database(sqlx::Error::Protocol(safe))
    }
}

pub(super) fn sanitize_mysql_app_error(error: AppError) -> AppError {
    match error {
        AppError::Database(sqlx_err) => sanitize_mysql_error(sqlx_err),
        other => other,
    }
}
