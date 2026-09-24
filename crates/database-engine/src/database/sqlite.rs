use super::*;

pub(super) async fn sqlite_columns(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<Vec<DatabaseTableColumn>> {
    let sql = format!("PRAGMA table_xinfo({})", quote_identifier(table_name));
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let name: String = row.try_get("name")?;
            let data_type: String = row.try_get("type")?;
            let notnull: i64 = row.try_get("notnull")?;
            let primary_key: i64 = row.try_get("pk")?;
            let default_value: Option<String> = row.try_get("dflt_value")?;
            let hidden: i64 = row.try_get("hidden")?;
            let auto_increment =
                primary_key > 0 && data_type.trim().eq_ignore_ascii_case("integer");

            Ok(DatabaseTableColumn {
                name,
                data_type,
                nullable: notnull == 0,
                primary_key: primary_key > 0,
                default_value,
                generated: matches!(hidden, 2 | 3),
                auto_increment,
            })
        })
        .collect()
}

pub(super) async fn sqlite_table_kind(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT type FROM sqlite_master WHERE name = ?1 LIMIT 1")
            .bind(table_name)
            .fetch_optional(pool)
            .await?;
    Ok(row
        .map(|value| value.0)
        .unwrap_or_else(|| "table".to_string()))
}

pub(super) async fn sqlite_ddl(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT sql FROM sqlite_master WHERE name = ?1 LIMIT 1")
            .bind(table_name)
            .fetch_optional(pool)
            .await?;
    let Some(mut ddl) = row.and_then(|value| value.0) else {
        return Ok(None);
    };
    if !ddl.trim_end().ends_with(';') {
        ddl.push(';');
    }
    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT sql FROM sqlite_master WHERE type = 'index' AND tbl_name = ?1 AND sql IS NOT NULL ORDER BY name",
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;
    for (index,) in indexes {
        ddl.push('\n');
        ddl.push_str(index.trim_end_matches(';'));
        ddl.push(';');
    }
    Ok(Some(ddl))
}

pub(super) async fn sqlite_indexes(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<Vec<DatabaseIndex>> {
    let list_sql = format!("PRAGMA index_list({})", quote_identifier(table_name));
    let list_rows = sqlx::query(&list_sql).fetch_all(pool).await?;

    let mut indexes = Vec::with_capacity(list_rows.len());
    for row in list_rows {
        let name: String = row.try_get("name")?;
        let unique: i64 = row.try_get("unique")?;
        let origin: String = row.try_get("origin").unwrap_or_default();

        let info_sql = format!("PRAGMA index_info({})", quote_identifier(&name));
        let info_rows = sqlx::query(&info_sql).fetch_all(pool).await?;
        let columns = info_rows
            .iter()
            .map(|info| info.try_get::<String, _>("name"))
            .collect::<Result<Vec<_>, _>>()?;

        indexes.push(DatabaseIndex {
            name,
            columns,
            unique: unique != 0,
            primary: origin == "pk",
        });
    }

    Ok(indexes)
}

pub(super) async fn sqlite_foreign_keys(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<Vec<DatabaseForeignKey>> {
    let sql = format!("PRAGMA foreign_key_list({})", quote_identifier(table_name));
    let rows = sqlx::query(&sql).fetch_all(pool).await?;

    // Rows for the same foreign key share an `id`; group them in order.
    let mut grouped: Vec<(i64, DatabaseForeignKey)> = Vec::new();
    for row in rows {
        let id: i64 = row.try_get("id")?;
        let referenced_table: String = row.try_get("table")?;
        let from: String = row.try_get("from")?;
        let to: String = row.try_get("to")?;

        if let Some((_, fk)) = grouped.iter_mut().find(|(existing, _)| *existing == id) {
            fk.columns.push(from);
            fk.referenced_columns.push(to);
        } else {
            grouped.push((
                id,
                DatabaseForeignKey {
                    name: format!("fk_{}_{}", table_name, id),
                    columns: vec![from],
                    referenced_table,
                    referenced_columns: vec![to],
                },
            ));
        }
    }

    Ok(grouped.into_iter().map(|(_, fk)| fk).collect())
}

pub(super) async fn ensure_sqlite_table_exists(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<()> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT name
        FROM sqlite_master
        WHERE name = ?1 AND type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
        LIMIT 1
        "#,
    )
    .bind(table_name)
    .fetch_optional(pool)
    .await?;

    row.map(|_| ())
        .ok_or_else(|| AppError::NotFound(table_name.to_string()))
}

pub(super) async fn sqlite_table_row_count(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<u64> {
    let sql = format!(
        "SELECT COUNT(*) AS total_rows FROM {}",
        quote_identifier(table_name)
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let total_rows: i64 = row.try_get("total_rows")?;
    Ok(total_rows.max(0) as u64)
}

pub(super) async fn sqlite_table_result_columns(
    pool: &sqlx::SqlitePool,
    table_name: &str,
) -> AppResult<Vec<DatabaseResultColumn>> {
    Ok(sqlite_columns(pool, table_name)
        .await?
        .into_iter()
        .map(|column| DatabaseResultColumn {
            name: column.name,
            data_type: column.data_type,
        })
        .collect())
}

pub(super) fn sqlite_result_columns(row: &sqlx::sqlite::SqliteRow) -> Vec<DatabaseResultColumn> {
    row.columns()
        .iter()
        .map(|column| DatabaseResultColumn {
            name: column.name().to_string(),
            data_type: column.type_info().name().to_string(),
        })
        .collect()
}

pub(super) fn sqlite_row_values(row: &sqlx::sqlite::SqliteRow) -> AppResult<Vec<Option<String>>> {
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
            if let Ok(value) = row.try_get::<f64, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<bool, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
                return Ok(Some(format!("<binary {} bytes>", value.len())));
            }
            if let Ok(value) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(index) {
                return Ok(Some(value.to_rfc3339()));
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

            Ok(Some("<unsupported>".to_string()))
        })
        .collect()
}

pub(super) async fn sqlite_pool(
    connection: &DatabaseConnection,
) -> AppResult<RuntimePool<sqlx::Sqlite>> {
    let path = connection
        .sqlite_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("SQLite path is required".to_string()))?;

    if path != ":memory:" && !Path::new(path).exists() {
        return Err(AppError::Validation(format!(
            "SQLite file does not exist: {}",
            path
        )));
    }

    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(AppError::from)?;
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&pool)
        .await?;
    Ok(RuntimePool {
        pool,
        profile: RuntimeDatabaseProfile::resolve(DetectedServerType::Sqlite, Some(version)),
    })
}
