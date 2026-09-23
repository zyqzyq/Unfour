use super::*;

pub(super) async fn pg_connect_options(
    connection: &DatabaseConnection,
    secret_store: Option<&SecretStore>,
    password_override: Option<&str>,
) -> AppResult<PgConnectOptions> {
    let host = connection
        .host
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("127.0.0.1");
    let port = connection.port.unwrap_or(5432);
    let database = connection
        .database
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Validation("PostgreSQL database name is required".to_string()))?;
    let username = connection
        .username
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Validation("PostgreSQL username is required".to_string()))?;

    let password = match password_override {
        Some(secret) => Some(secret.to_string()),
        None => resolve_database_password(connection, secret_store).await?,
    };

    let mut options = PgConnectOptions::new()
        .host(host)
        .port(port as u16)
        .database(database)
        .username(username);

    if let Some(pw) = password {
        options = options.password(&pw);
    }

    Ok(options)
}

/// Load a database password from SecretStore if a credential reference is
/// present. Returns `None` when no credential_ref is configured.
pub(super) async fn resolve_database_password(
    connection: &DatabaseConnection,
    secret_store: Option<&SecretStore>,
) -> AppResult<Option<String>> {
    if let Some(credential_ref) = connection
        .credential_ref
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let store = secret_store.ok_or_else(|| {
            AppError::Config(
                "SecretStore is not available; cannot load database password".to_string(),
            )
        })?;
        let secret = store
            .read_secret(connection.workspace_id.clone(), credential_ref.to_string())
            .await
            .map_err(|_| {
                AppError::Config("Failed to load database password from SecretStore".to_string())
            })?;
        Ok(Some(secret))
    } else {
        Ok(None)
    }
}

pub(super) async fn postgres_columns(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseTableColumn>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT c.column_name,
               pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type,
               c.is_nullable,
               c.column_default,
               c.is_generated,
               c.identity_generation,
               EXISTS (
                 SELECT 1
                 FROM pg_catalog.pg_constraint con
                 WHERE con.conrelid = cls.oid
                   AND con.contype = 'p'
                   AND a.attnum = ANY(con.conkey)
               ) AS primary_key
        FROM information_schema.columns c
        JOIN pg_catalog.pg_namespace n ON n.nspname = c.table_schema
        JOIN pg_catalog.pg_class cls ON cls.relnamespace = n.oid AND cls.relname = c.table_name
        JOIN pg_catalog.pg_attribute a ON a.attrelid = cls.oid AND a.attname = c.column_name
        WHERE c.table_schema = $1 AND c.table_name = $2
        ORDER BY c.ordinal_position
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let name: String = row.try_get("column_name")?;
            let data_type: String = row.try_get("data_type")?;
            let is_nullable: String = row.try_get("is_nullable")?;
            let column_default: Option<String> = row.try_get("column_default")?;
            let is_generated: String = row.try_get("is_generated")?;
            let identity_generation: Option<String> = row.try_get("identity_generation")?;
            let primary_key: bool = row.try_get("primary_key")?;
            let auto_increment = identity_generation.is_some()
                || column_default
                    .as_deref()
                    .is_some_and(|value| value.starts_with("nextval("));

            Ok(DatabaseTableColumn {
                name,
                data_type,
                nullable: is_nullable == "YES",
                primary_key,
                default_value: column_default,
                generated: is_generated != "NEVER"
                    || identity_generation.as_deref() == Some("ALWAYS"),
                auto_increment,
            })
        })
        .collect()
}

pub(super) async fn postgres_indexes(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseIndex>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT i.relname AS index_name,
               ix.indisunique AS is_unique,
               ix.indisprimary AS is_primary,
               pg_get_indexdef(ix.indexrelid, k.ord::integer, true) AS column_name,
               k.ord
        FROM pg_class t
        JOIN pg_namespace n ON n.oid = t.relnamespace
        JOIN pg_index ix ON ix.indrelid = t.oid
        JOIN pg_class i ON i.oid = ix.indexrelid
        JOIN LATERAL generate_series(1, ix.indnatts) AS k(ord) ON true
        WHERE n.nspname = $1 AND t.relname = $2
        ORDER BY index_name, ord
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut indexes: Vec<DatabaseIndex> = Vec::new();
    for row in rows {
        let name: String = row.try_get("index_name")?;
        let unique: bool = row.try_get("is_unique")?;
        let primary: bool = row.try_get("is_primary")?;
        let column_name: String = row.try_get("column_name")?;

        if let Some(existing) = indexes.iter_mut().find(|idx| idx.name == name) {
            existing.columns.push(column_name);
        } else {
            indexes.push(DatabaseIndex {
                name,
                columns: vec![column_name],
                unique,
                primary,
            });
        }
    }

    Ok(indexes)
}

pub(super) async fn postgres_foreign_keys(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseForeignKey>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT con.conname AS name,
               att.attname AS column_name,
               cl.relname AS referenced_table,
               fatt.attname AS referenced_column,
               k.ord AS ord
        FROM pg_constraint con
        JOIN pg_class c ON c.oid = con.conrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS k(attnum, ord) ON true
        JOIN pg_attribute att ON att.attrelid = con.conrelid AND att.attnum = k.attnum
        JOIN pg_class cl ON cl.oid = con.confrelid
        JOIN LATERAL unnest(con.confkey) WITH ORDINALITY AS fk(attnum, ford) ON fk.ford = k.ord
        JOIN pg_attribute fatt ON fatt.attrelid = con.confrelid AND fatt.attnum = fk.attnum
        WHERE con.contype = 'f' AND n.nspname = $1 AND c.relname = $2
        ORDER BY name, ord
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut keys: Vec<DatabaseForeignKey> = Vec::new();
    for row in rows {
        let name: String = row.try_get("name")?;
        let column_name: String = row.try_get("column_name")?;
        let referenced_table: String = row.try_get("referenced_table")?;
        let referenced_column: String = row.try_get("referenced_column")?;

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

pub(super) async fn ensure_postgres_table_exists(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<(), AppError> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = $1
          AND table_name = $2
          AND table_schema NOT IN ('pg_catalog', 'information_schema')
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
/// "table". Falls back to "table" when the row is missing (the caller has
/// already verified existence via `ensure_postgres_table_exists`).
pub(super) async fn postgres_table_kind(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<String, AppError> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT table_type
        FROM information_schema.tables
        WHERE table_schema = $1 AND table_name = $2
        LIMIT 1
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_optional(pool)
    .await?;
    Ok(row
        .map(|(table_type,)| {
            if table_type == "VIEW" {
                "view".to_string()
            } else {
                "table".to_string()
            }
        })
        .unwrap_or_else(|| "table".to_string()))
}

pub(super) async fn postgres_ddl(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
    kind: &str,
) -> AppResult<String> {
    let qualified = quote_qualified_identifier(schema, table_name);
    if kind == "view" {
        let definition: String = sqlx::query_scalar(
            "SELECT pg_get_viewdef(c.oid, true) FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = $1 AND c.relname = $2",
        )
        .bind(schema)
        .bind(table_name)
        .fetch_one(pool)
        .await?;
        return Ok(format!(
            "CREATE VIEW {qualified} AS\n{};",
            definition.trim_end_matches(';')
        ));
    }

    let rows = sqlx::query(
        r#"SELECT a.attname AS name, format_type(a.atttypid, a.atttypmod) AS data_type,
                  a.attnotnull AS not_null, pg_get_expr(d.adbin, d.adrelid) AS default_value,
                  a.attidentity::text AS identity, a.attgenerated::text AS generated
           FROM pg_attribute a
           JOIN pg_class c ON c.oid = a.attrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
           LEFT JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum
           WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 AND NOT a.attisdropped
           ORDER BY a.attnum"#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;
    let mut clauses = Vec::with_capacity(rows.len());
    for row in rows {
        let name: String = row.try_get("name")?;
        let data_type: String = row.try_get("data_type")?;
        let not_null: bool = row.try_get("not_null")?;
        let default_value: Option<String> = row.try_get("default_value")?;
        let identity: String = row.try_get("identity")?;
        let generated: String = row.try_get("generated")?;
        clauses.push(postgres_column_ddl(
            &name,
            &data_type,
            not_null,
            default_value.as_deref(),
            &identity,
            &generated,
        ));
    }
    let constraints = sqlx::query(
        r#"SELECT con.conname, pg_get_constraintdef(con.oid, true) AS definition
           FROM pg_constraint con JOIN pg_class c ON c.oid = con.conrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
           WHERE n.nspname = $1 AND c.relname = $2 AND con.contype IN ('p','f','u','c','x')
           ORDER BY CASE con.contype WHEN 'p' THEN 0 WHEN 'u' THEN 1 WHEN 'c' THEN 2 WHEN 'f' THEN 3 ELSE 4 END, con.conname"#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;
    for row in constraints {
        let name: String = row.try_get("conname")?;
        let definition: String = row.try_get("definition")?;
        clauses.push(postgres_constraint_ddl(&name, &definition));
    }
    // Constraint-backed indexes are created by the table constraints above.
    let indexes: Vec<(String,)> = sqlx::query_as(
        r#"SELECT pg_get_indexdef(i.indexrelid)
           FROM pg_index i JOIN pg_class c ON c.oid = i.indrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
           LEFT JOIN pg_constraint con ON con.conindid = i.indexrelid
           WHERE n.nspname = $1 AND c.relname = $2 AND con.oid IS NULL
           ORDER BY i.indexrelid"#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;
    Ok(assemble_postgres_ddl(
        &qualified,
        &clauses,
        indexes.into_iter().map(|(ddl,)| ddl),
    ))
}

fn postgres_constraint_ddl(name: &str, definition: &str) -> String {
    format!("CONSTRAINT {} {definition}", quote_identifier(name))
}

fn assemble_postgres_ddl(
    qualified: &str,
    clauses: &[String],
    indexes: impl IntoIterator<Item = String>,
) -> String {
    let mut ddl = format!(
        "CREATE TABLE {qualified} (\n    {}\n);",
        clauses.join(",\n    ")
    );
    for index in indexes {
        ddl.push_str("\n");
        ddl.push_str(index.trim_end_matches(';'));
        ddl.push(';');
    }
    ddl
}

fn postgres_column_ddl(
    name: &str,
    data_type: &str,
    not_null: bool,
    default_value: Option<&str>,
    identity: &str,
    generated: &str,
) -> String {
    let mut sql = format!("{} {}", quote_identifier(name), data_type);
    if generated == "s" || generated == "v" {
        if let Some(expr) = default_value {
            sql.push_str(&format!(
                " GENERATED ALWAYS AS ({expr}) {}",
                if generated == "s" {
                    "STORED"
                } else {
                    "VIRTUAL"
                }
            ));
        }
    } else if identity == "a" || identity == "d" {
        sql.push_str(if identity == "a" {
            " GENERATED ALWAYS AS IDENTITY"
        } else {
            " GENERATED BY DEFAULT AS IDENTITY"
        });
    } else if default_value.is_some_and(|value| value.starts_with("nextval("))
        && matches!(data_type, "smallint" | "integer" | "bigint")
    {
        // A SERIAL column's nextval default names a separate sequence. An
        // identity column recreates that dependency in a standalone export.
        sql.push_str(" GENERATED BY DEFAULT AS IDENTITY");
    } else if let Some(default_value) = default_value {
        sql.push_str(&format!(" DEFAULT {default_value}"));
    }
    if not_null {
        sql.push_str(" NOT NULL");
    }
    sql
}

#[cfg(test)]
mod ddl_tests {
    use super::{assemble_postgres_ddl, postgres_column_ddl, postgres_constraint_ddl};

    #[test]
    fn postgres_column_ddl_handles_types_defaults_identity_and_generated() {
        assert_eq!(
            postgres_column_ddl("user\"id", "bigint", true, Some("42"), "", ""),
            "\"user\"\"id\" bigint DEFAULT 42 NOT NULL"
        );
        assert_eq!(
            postgres_column_ddl("id", "integer", true, None, "a", ""),
            "\"id\" integer GENERATED ALWAYS AS IDENTITY NOT NULL"
        );
        assert_eq!(
            postgres_column_ddl("slug", "text", false, Some("lower(name)"), "", "s"),
            "\"slug\" text GENERATED ALWAYS AS (lower(name)) STORED"
        );
        assert_eq!(
            postgres_column_ddl(
                "id",
                "bigint",
                true,
                Some("nextval('items_id_seq'::regclass)"),
                "",
                ""
            ),
            "\"id\" bigint GENERATED BY DEFAULT AS IDENTITY NOT NULL"
        );
    }

    #[test]
    fn postgres_table_ddl_includes_primary_key_foreign_key_and_index() {
        let clauses = vec![
            postgres_column_ddl("id", "integer", true, None, "", ""),
            postgres_constraint_ddl("orders_pkey", "PRIMARY KEY (id)"),
            postgres_constraint_ddl(
                "orders_customer_fk",
                "FOREIGN KEY (customer_id) REFERENCES customers(id)",
            ),
        ];
        let ddl = assemble_postgres_ddl(
            "\"public\".\"orders\"",
            &clauses,
            [
                "CREATE INDEX orders_created_idx ON public.orders USING btree (created_at)"
                    .to_string(),
            ],
        );
        assert!(ddl.contains("CONSTRAINT \"orders_pkey\" PRIMARY KEY (id)"));
        assert!(ddl.contains("CONSTRAINT \"orders_customer_fk\" FOREIGN KEY"));
        assert!(ddl.contains(
            "CREATE INDEX orders_created_idx ON public.orders USING btree (created_at);"
        ));
    }
}

pub(super) async fn postgres_table_row_count(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<u64, AppError> {
    let sql = format!(
        "SELECT COUNT(*) AS total_rows FROM {}",
        quote_qualified_identifier(schema, table_name)
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let total_rows: i64 = row.try_get("total_rows")?;
    Ok(total_rows.max(0) as u64)
}

pub(super) async fn postgres_table_result_columns(
    pool: &sqlx::PgPool,
    schema: &str,
    table_name: &str,
) -> Result<Vec<DatabaseResultColumn>, AppError> {
    Ok(postgres_columns(pool, schema, table_name)
        .await?
        .into_iter()
        .map(|column| DatabaseResultColumn {
            name: column.name,
            data_type: column.data_type,
        })
        .collect())
}

pub(super) fn postgres_result_columns(row: &sqlx::postgres::PgRow) -> Vec<DatabaseResultColumn> {
    row.columns()
        .iter()
        .map(|column| DatabaseResultColumn {
            name: column.name().to_string(),
            data_type: column.type_info().name().to_string(),
        })
        .collect()
}

pub(super) fn postgres_table_from_metadata(
    catalog: Option<String>,
    schema: String,
    name: String,
    kind: String,
    columns: Vec<DatabaseTableColumn>,
) -> DatabaseTable {
    DatabaseTable {
        catalog,
        schema: Some(schema),
        name,
        kind,
        columns,
    }
}

pub(super) fn postgres_row_values(row: &sqlx::postgres::PgRow) -> AppResult<Vec<Option<String>>> {
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
            if let Ok(value) = row.try_get::<i32, _>(index) {
                return Ok(Some(value.to_string()));
            }
            if let Ok(value) = row.try_get::<i16, _>(index) {
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

/// Sanitize a sqlx::Error into an AppError.
///
/// Instead of wiping the whole message whenever it mentions credentials, we
/// keep the original diagnostic text and only scrub credential material that
/// may have leaked into it (e.g. a connection string with an embedded
/// password). This preserves useful errors such as "password authentication
/// failed for user \"x\"" while still redacting the secret value.
pub(super) fn sanitize_pg_error(error: sqlx::Error) -> AppError {
    let msg = error.to_string();
    let safe = redact_connection_string(&msg);
    if safe == msg {
        AppError::Database(error)
    } else {
        AppError::Database(sqlx::Error::Protocol(safe))
    }
}

/// Sanitize an AppError from a helper that already wraps sqlx errors.
pub(super) fn sanitize_pg_app_error(error: AppError) -> AppError {
    match error {
        AppError::Database(sqlx_err) => {
            let msg = sqlx_err.to_string();
            let safe = redact_connection_string(&msg);
            if safe == msg {
                AppError::Database(sqlx_err)
            } else {
                AppError::Database(sqlx::Error::Protocol(safe))
            }
        }
        other => other,
    }
}
