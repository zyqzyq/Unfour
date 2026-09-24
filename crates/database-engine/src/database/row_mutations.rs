use super::*;
use sqlx::{Execute, MySql, Postgres, QueryBuilder, Sqlite};
use std::collections::{HashMap, HashSet};
use unfour_core::models::DatabaseCellValueMode;

impl DatabaseService {
    /// Execute one confirmed row mutation. Identifiers are accepted only after
    /// matching live table metadata and values are always sent as bind values.
    pub async fn mutate_table_row(
        &self,
        input: DatabaseRowMutationInput,
    ) -> AppResult<DatabaseRowMutationResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let table_name = input.table_name.trim();
        if table_name.is_empty() {
            return Err(AppError::Validation(
                "table name cannot be empty".to_string(),
            ));
        }
        if !input.confirm_mutation {
            return Err(AppError::ConfirmationRequired {
                message: "row changes require explicit confirmation".to_string(),
                details: serde_json::json!({
                    "operation": input.operation,
                    "tableName": table_name,
                }),
            });
        }

        let operation = input.operation.trim().to_ascii_lowercase();
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        if connection.read_only {
            return Err(AppError::ReadOnly(
                "this connection is read-only; row edits are not allowed".to_string(),
            ));
        }

        match connection.driver.as_str() {
            "sqlite" => {
                let pool = sqlite_pool(&connection).await?;
                require_capability(pool.profile.capabilities.row_mutation, "row mutation")?;
                ensure_sqlite_table_exists(&pool, table_name).await?;
                let columns = sqlite_columns(&pool, table_name).await?;
                validate_mutation(&operation, &input, &columns)?;
                execute_sqlite_mutation(&pool, None, table_name, &operation, &input).await
            }
            "postgres" => {
                let effective = Self::effective_connection(&connection, input.catalog.as_deref());
                let pool = self.postgres_pool(&effective).await?;
                require_capability(pool.profile.capabilities.row_mutation, "row mutation")?;
                let schema = input
                    .schema
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .unwrap_or("public");
                ensure_postgres_table_exists(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                let columns = postgres_columns(&pool, schema, table_name)
                    .await
                    .map_err(sanitize_pg_app_error)?;
                validate_mutation(&operation, &input, &columns)?;
                execute_postgres_mutation(
                    &pool,
                    Some(schema),
                    table_name,
                    &operation,
                    &input,
                    &columns,
                )
                .await
                .map_err(sanitize_pg_app_error)
            }
            "mysql" => {
                let effective = Self::effective_connection(&connection, input.catalog.as_deref());
                let pool = self.mysql_pool(&effective).await?;
                require_capability(pool.profile.capabilities.row_mutation, "row mutation")?;
                let schema = input
                    .catalog
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .or_else(|| {
                        input
                            .schema
                            .as_deref()
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
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
                validate_mutation(&operation, &input, &columns)?;
                execute_mysql_mutation(&pool, Some(schema), table_name, &operation, &input)
                    .await
                    .map_err(sanitize_mysql_app_error)
            }
            driver => Err(AppError::Unsupported(format!(
                "{} row editing is not yet supported",
                display_driver(driver)
            ))),
        }
    }
}

fn validate_mutation(
    operation: &str,
    input: &DatabaseRowMutationInput,
    columns: &[DatabaseTableColumn],
) -> AppResult<()> {
    if !matches!(operation, "insert" | "update" | "delete") {
        return Err(AppError::Validation(format!(
            "unsupported row operation: {operation}"
        )));
    }
    if operation == "update" && input.values.is_empty() {
        return Err(AppError::Validation(
            "update requires at least one column value".to_string(),
        ));
    }

    let column_map = columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect::<HashMap<_, _>>();
    validate_cells("values", &input.values, operation == "insert", &column_map)?;
    validate_cells("primary key", &input.primary_key, false, &column_map)?;
    validate_cells(
        "original values",
        &input.original_values,
        false,
        &column_map,
    )?;

    if operation != "insert" {
        let actual = columns
            .iter()
            .filter(|column| column.primary_key)
            .map(|column| column.name.as_str())
            .collect::<HashSet<_>>();
        let requested = input
            .primary_key
            .iter()
            .map(|cell| cell.column.as_str())
            .collect::<HashSet<_>>();
        if actual.is_empty() {
            return Err(AppError::Validation(
                "update/delete requires a real table primary key".to_string(),
            ));
        }
        if actual != requested || requested.len() != input.primary_key.len() {
            return Err(AppError::Validation(
                "primary key columns do not match the table primary key".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_cells(
    label: &str,
    cells: &[DatabaseCellValue],
    allow_default: bool,
    columns: &HashMap<&str, &DatabaseTableColumn>,
) -> AppResult<()> {
    let mut seen = HashSet::new();
    for cell in cells {
        if cell.column.trim().is_empty() || !seen.insert(cell.column.as_str()) {
            return Err(AppError::Validation(format!(
                "{label} contains an empty or duplicate column"
            )));
        }
        let column = columns.get(cell.column.as_str()).ok_or_else(|| {
            AppError::Validation(format!("unknown table column: {}", cell.column))
        })?;
        if column.generated && label == "values" && cell.mode != DatabaseCellValueMode::Default {
            return Err(AppError::Validation(format!(
                "generated column cannot be written: {}",
                cell.column
            )));
        }
        if cell.mode == DatabaseCellValueMode::Default && !allow_default {
            return Err(AppError::Validation(format!(
                "DEFAULT is not valid in {label}"
            )));
        }
    }
    Ok(())
}

fn value(cell: &DatabaseCellValue) -> Option<String> {
    match cell.mode {
        DatabaseCellValueMode::Null => None,
        _ => cell.value.clone(),
    }
}

fn finish_result(affected_rows: u64, sql: String) -> AppResult<DatabaseRowMutationResult> {
    if affected_rows != 1 {
        return Err(AppError::RowConflict(format!(
            "expected to affect exactly one row, affected {affected_rows}; reload the table before retrying"
        )));
    }
    Ok(DatabaseRowMutationResult { affected_rows, sql })
}

async fn execute_sqlite_mutation(
    pool: &sqlx::SqlitePool,
    schema: Option<&str>,
    table: &str,
    operation: &str,
    input: &DatabaseRowMutationInput,
) -> AppResult<DatabaseRowMutationResult> {
    let dialect = SqlDialect::Standard;
    let mut query = QueryBuilder::<Sqlite>::new("");
    build_sqlite_query(&mut query, dialect, schema, table, operation, input)?;
    let built = query.build();
    let sql = built.sql().to_string();
    let result = built.execute(pool).await?;
    finish_result(result.rows_affected(), sql)
}

fn build_sqlite_query<'a>(
    query: &mut QueryBuilder<'a, Sqlite>,
    dialect: SqlDialect,
    schema: Option<&str>,
    table: &str,
    operation: &str,
    input: &'a DatabaseRowMutationInput,
) -> AppResult<()> {
    let qualified = dialect.quote_qualified(schema, table);
    match operation {
        "insert" => build_insert(query, dialect, &qualified, input, false, |query, cell| {
            query.push_bind(value(cell));
        }),
        "update" => {
            query.push("UPDATE ").push(qualified).push(" SET ");
            let mut separated = query.separated(", ");
            for cell in &input.values {
                separated
                    .push(dialect.quote_ident(&cell.column))
                    .push_unseparated(" = ");
                separated.push_bind_unseparated(value(cell));
            }
            push_predicates(
                query,
                dialect,
                &input.primary_key,
                &input.original_values,
                " IS ",
                |query, cell| {
                    query.push_bind(value(cell));
                },
            );
            Ok(())
        }
        "delete" => {
            query.push("DELETE FROM ").push(qualified);
            push_predicates(
                query,
                dialect,
                &input.primary_key,
                &input.original_values,
                " IS ",
                |query, cell| {
                    query.push_bind(value(cell));
                },
            );
            Ok(())
        }
        _ => unreachable!(),
    }
}

async fn execute_mysql_mutation(
    pool: &sqlx::MySqlPool,
    schema: Option<&str>,
    table: &str,
    operation: &str,
    input: &DatabaseRowMutationInput,
) -> AppResult<DatabaseRowMutationResult> {
    let dialect = SqlDialect::MySql;
    let qualified = dialect.quote_qualified(schema, table);
    let mut query = QueryBuilder::<MySql>::new("");
    match operation {
        "insert" => build_insert(
            &mut query,
            dialect,
            &qualified,
            input,
            true,
            |query, cell| {
                query.push_bind(value(cell));
            },
        )?,
        "update" => {
            query.push("UPDATE ").push(qualified).push(" SET ");
            let mut separated = query.separated(", ");
            for cell in &input.values {
                separated
                    .push(dialect.quote_ident(&cell.column))
                    .push_unseparated(" = ");
                separated.push_bind_unseparated(value(cell));
            }
            push_predicates(
                &mut query,
                dialect,
                &input.primary_key,
                &input.original_values,
                " <=> ",
                |query, cell| {
                    query.push_bind(value(cell));
                },
            );
        }
        "delete" => {
            query.push("DELETE FROM ").push(qualified);
            push_predicates(
                &mut query,
                dialect,
                &input.primary_key,
                &input.original_values,
                " <=> ",
                |query, cell| {
                    query.push_bind(value(cell));
                },
            );
        }
        _ => unreachable!(),
    }
    let built = query.build();
    let sql = built.sql().to_string();
    let result = built.execute(pool).await?;
    finish_result(result.rows_affected(), sql)
}

async fn execute_postgres_mutation(
    pool: &sqlx::PgPool,
    schema: Option<&str>,
    table: &str,
    operation: &str,
    input: &DatabaseRowMutationInput,
    columns: &[DatabaseTableColumn],
) -> AppResult<DatabaseRowMutationResult> {
    let dialect = SqlDialect::Standard;
    let qualified = dialect.quote_qualified(schema, table);
    let types = columns
        .iter()
        .map(|column| (column.name.as_str(), column.data_type.as_str()))
        .collect::<HashMap<_, _>>();
    let mut query = QueryBuilder::<Postgres>::new("");
    let push_value = |query: &mut QueryBuilder<'_, Postgres>, cell: &DatabaseCellValue| {
        let data_type = types[cell.column.as_str()];
        query
            .push("(")
            .push_bind(value(cell))
            .push("::text::")
            .push(data_type)
            .push(")");
    };
    match operation {
        "insert" => build_insert(&mut query, dialect, &qualified, input, false, push_value)?,
        "update" => {
            query.push("UPDATE ").push(qualified).push(" SET ");
            let mut separated = query.separated(", ");
            for cell in &input.values {
                separated
                    .push(dialect.quote_ident(&cell.column))
                    .push_unseparated(" = ");
                let data_type = types[cell.column.as_str()];
                separated
                    .push_unseparated("(")
                    .push_bind_unseparated(value(cell))
                    .push_unseparated("::text::")
                    .push_unseparated(data_type)
                    .push_unseparated(")");
            }
            push_predicates(
                &mut query,
                dialect,
                &input.primary_key,
                &input.original_values,
                " IS NOT DISTINCT FROM ",
                push_value,
            );
        }
        "delete" => {
            query.push("DELETE FROM ").push(qualified);
            push_predicates(
                &mut query,
                dialect,
                &input.primary_key,
                &input.original_values,
                " IS NOT DISTINCT FROM ",
                push_value,
            );
        }
        _ => unreachable!(),
    }
    let built = query.build();
    let sql = built.sql().to_string();
    let result = built.execute(pool).await?;
    finish_result(result.rows_affected(), sql)
}

fn build_insert<'a, DB, F>(
    query: &mut QueryBuilder<'a, DB>,
    dialect: SqlDialect,
    qualified: &str,
    input: &'a DatabaseRowMutationInput,
    mysql: bool,
    mut push_value: F,
) -> AppResult<()>
where
    DB: sqlx::Database,
    F: FnMut(&mut QueryBuilder<'a, DB>, &'a DatabaseCellValue),
{
    let values = input
        .values
        .iter()
        .filter(|cell| cell.mode != DatabaseCellValueMode::Default)
        .collect::<Vec<_>>();
    query.push("INSERT INTO ").push(qualified);
    if values.is_empty() {
        query.push(if mysql {
            " () VALUES ()"
        } else {
            " DEFAULT VALUES"
        });
        return Ok(());
    }
    query.push(" (");
    {
        let mut separated = query.separated(", ");
        for cell in &values {
            separated.push(dialect.quote_ident(&cell.column));
        }
    }
    query.push(") VALUES (");
    for (index, cell) in values.iter().enumerate() {
        if index > 0 {
            query.push(", ");
        }
        push_value(query, cell);
    }
    query.push(")");
    Ok(())
}

fn push_predicates<'a, DB, F>(
    query: &mut QueryBuilder<'a, DB>,
    dialect: SqlDialect,
    primary_key: &'a [DatabaseCellValue],
    original_values: &'a [DatabaseCellValue],
    operator: &str,
    mut push_value: F,
) where
    DB: sqlx::Database,
    F: FnMut(&mut QueryBuilder<'a, DB>, &'a DatabaseCellValue),
{
    query.push(" WHERE ");
    for (index, cell) in primary_key.iter().chain(original_values.iter()).enumerate() {
        if index > 0 {
            query.push(" AND ");
        }
        query.push(dialect.quote_ident(&cell.column)).push(operator);
        push_value(query, cell);
    }
}
