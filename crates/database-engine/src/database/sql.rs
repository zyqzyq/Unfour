use super::*;

pub(super) fn quote_identifier(value: &str) -> String {
    DatabaseDialect::Postgres.quote_identifier(value)
}

/// Trim and validate an optional database/schema identifier used as a
/// connection or session context (for example, PostgreSQL `search_path`).
/// Returns `None` for empty input. Callers still quote/escape the result before
/// use; this rejects control characters as defense in depth.
pub(super) fn clean_identifier(value: Option<&str>) -> AppResult<Option<&str>> {
    let trimmed = match value.map(str::trim).filter(|item| !item.is_empty()) {
        None => return Ok(None),
        Some(item) => item,
    };
    if trimmed.chars().any(|ch| ch == '\0' || ch.is_control()) {
        return Err(AppError::Validation(
            "database or schema name contains invalid characters".to_string(),
        ));
    }
    Ok(Some(trimmed))
}

pub(super) fn quote_mysql_identifier(value: &str) -> String {
    DatabaseDialect::Mysql.quote_identifier(value)
}

pub(super) fn quote_qualified_identifier(schema: &str, table_name: &str) -> String {
    format!(
        "{}.{}",
        quote_identifier(schema),
        quote_identifier(table_name)
    )
}

pub(super) fn postgres_browse_sql(
    schema: &str,
    table_name: &str,
    where_sql: &str,
    order_sql: &str,
    limit: u32,
    offset: u32,
) -> String {
    format!(
        "SELECT * FROM {}{}{} LIMIT {} OFFSET {}",
        quote_qualified_identifier(schema, table_name),
        where_sql,
        order_sql,
        limit,
        offset
    )
}

pub(super) fn quote_mysql_qualified_identifier(schema: &str, table_name: &str) -> String {
    format!(
        "{}.{}",
        quote_mysql_identifier(schema),
        quote_mysql_identifier(table_name)
    )
}

pub(super) fn mysql_browse_sql(
    schema: &str,
    table_name: &str,
    where_sql: &str,
    order_sql: &str,
    limit: u32,
    offset: u32,
) -> String {
    format!(
        "SELECT * FROM {}{}{} LIMIT {} OFFSET {}",
        quote_mysql_qualified_identifier(schema, table_name),
        where_sql,
        order_sql,
        limit,
        offset
    )
}

pub(super) const DEFAULT_QUERY_TIMEOUT_MS: u64 = 30_000;
pub(super) const MIN_QUERY_TIMEOUT_MS: u64 = 1_000;
pub(super) const MAX_QUERY_TIMEOUT_MS: u64 = 300_000;

/// Resolve a per-statement timeout, clamping caller input into a sane band and
/// applying a default so a runaway query cannot hang a session indefinitely.
pub(super) fn resolve_timeout(timeout_ms: Option<u64>) -> Duration {
    let ms = timeout_ms
        .unwrap_or(DEFAULT_QUERY_TIMEOUT_MS)
        .clamp(MIN_QUERY_TIMEOUT_MS, MAX_QUERY_TIMEOUT_MS);
    Duration::from_millis(ms)
}

/// Trim a browse filter to a non-empty needle, or `None` when blank.
pub(super) fn normalize_filter(filter: Option<&str>) -> Option<String> {
    filter
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Build a validated, quoted `ORDER BY` fragment. The column must be one of the
/// table's real columns (defense in depth on top of identifier quoting); an
/// empty/absent column yields no clause.
pub(super) fn order_by_clause(
    order_by: Option<&str>,
    descending: bool,
    columns: &[String],
    quote: fn(&str) -> String,
) -> AppResult<String> {
    let Some(column) = order_by.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(String::new());
    };
    if !columns.iter().any(|name| name == column) {
        return Err(AppError::Validation(format!(
            "unknown sort column: {}",
            column
        )));
    }
    Ok(format!(
        " ORDER BY {} {}",
        quote(column),
        if descending { "DESC" } else { "ASC" }
    ))
}

/// `(CAST(col AS TEXT) LIKE ? OR ...)` for SQLite. One placeholder per column.
pub(super) fn sqlite_filter_where(columns: &[String]) -> String {
    let parts = columns
        .iter()
        .map(|name| format!("CAST({} AS TEXT) LIKE ?", quote_identifier(name)))
        .collect::<Vec<_>>();
    format!("({})", parts.join(" OR "))
}

/// `(CAST(col AS TEXT) ILIKE $1 OR ...)` for PostgreSQL. Reuses a single bind.
pub(super) fn postgres_filter_where(columns: &[String]) -> String {
    let parts = columns
        .iter()
        .map(|name| format!("CAST({} AS TEXT) ILIKE $1", quote_identifier(name)))
        .collect::<Vec<_>>();
    format!("({})", parts.join(" OR "))
}

const MAX_EXPORT_FILTERS: usize = 32;
const MAX_EXPORT_IN_VALUES: usize = 1_000;

pub(super) struct ExportSelect {
    pub sql: String,
    pub binds: Vec<String>,
    pub columns: Vec<DatabaseTableColumn>,
}

/// Build a parameterized data SELECT for table export.
///
/// Identifiers are quoted only after they match a real column. Values are
/// returned as binds. Callers must not interpolate `filters` into SQL.
pub(super) fn build_export_select(
    dialect: DatabaseDialect,
    qualified_table: &str,
    table_columns: &[DatabaseTableColumn],
    columns: Option<&[String]>,
    filters: &[DatabaseExportFilter],
    limit: Option<u32>,
) -> AppResult<ExportSelect> {
    if filters.len() > MAX_EXPORT_FILTERS {
        return Err(AppError::Validation("too many export filters".into()));
    }
    let quote: fn(&str) -> String = if dialect == DatabaseDialect::Mysql {
        quote_mysql_identifier
    } else {
        quote_identifier
    };
    let selected = match columns {
        None => table_columns.to_vec(),
        Some(requested) => {
            if requested.is_empty() {
                return Err(AppError::Validation(
                    "export columns cannot be empty".into(),
                ));
            }
            let mut selected = Vec::with_capacity(requested.len());
            for name in requested {
                let column = name.trim();
                if column.is_empty() {
                    return Err(AppError::Validation(
                        "export column name cannot be empty".into(),
                    ));
                }
                if selected
                    .iter()
                    .any(|item: &DatabaseTableColumn| item.name == column)
                {
                    return Err(AppError::Validation(format!(
                        "duplicate export column: {column}"
                    )));
                }
                let Some(found) = table_columns.iter().find(|item| item.name == column) else {
                    return Err(AppError::Validation(format!(
                        "unknown export column: {column}"
                    )));
                };
                selected.push(found.clone());
            }
            selected
        }
    };
    let select_list = if columns.is_none() {
        "*".to_string()
    } else {
        selected
            .iter()
            .map(|column| quote(&column.name))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut binds = Vec::new();
    let mut predicates = Vec::new();
    for filter in filters {
        let column = filter.column.trim();
        if !table_columns.iter().any(|item| item.name == column) {
            return Err(AppError::Validation(format!(
                "unknown export filter column: {column}"
            )));
        }
        let quoted = quote(column);
        let cast = if dialect == DatabaseDialect::Mysql {
            format!("CAST({quoted} AS CHAR)")
        } else {
            format!("CAST({quoted} AS TEXT)")
        };
        match filter.op {
            DatabaseExportFilterOp::Eq => {
                if filter.values.len() != 1 {
                    return Err(AppError::Validation(
                        "eq export filter requires exactly one value".into(),
                    ));
                }
                match &filter.values[0] {
                    None => predicates.push(format!("{quoted} IS NULL")),
                    Some(value) => {
                        binds.push(value.clone());
                        predicates.push(format!(
                            "{cast} = {}",
                            export_placeholder(dialect, binds.len())
                        ));
                    }
                }
            }
            DatabaseExportFilterOp::In => {
                if filter.values.is_empty() || filter.values.len() > MAX_EXPORT_IN_VALUES {
                    return Err(AppError::Validation(
                        "in export filter requires between 1 and 1000 values".into(),
                    ));
                }
                if filter.values.iter().any(Option::is_none) {
                    return Err(AppError::Validation(
                        "in export filter cannot include null".into(),
                    ));
                }
                let mut placeholders = Vec::with_capacity(filter.values.len());
                for value in filter.values.iter().flatten() {
                    binds.push(value.clone());
                    placeholders.push(export_placeholder(dialect, binds.len()));
                }
                predicates.push(format!("{cast} IN ({})", placeholders.join(", ")));
            }
        }
    }
    let where_sql = if predicates.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", predicates.join(" AND "))
    };
    let limit_sql = match limit {
        None => String::new(),
        Some(0) => {
            return Err(AppError::Validation(
                "export limit must be greater than zero".into(),
            ))
        }
        Some(limit) => format!(" LIMIT {limit}"),
    };
    Ok(ExportSelect {
        sql: format!("SELECT {select_list} FROM {qualified_table}{where_sql}{limit_sql}"),
        binds,
        columns: selected,
    })
}

fn export_placeholder(dialect: DatabaseDialect, index: usize) -> String {
    if dialect == DatabaseDialect::Postgres {
        format!("${index}")
    } else {
        "?".to_string()
    }
}

/// `(CAST(col AS CHAR) LIKE ? OR ...)` for MySQL. One placeholder per column.
pub(super) fn mysql_filter_where(columns: &[String]) -> String {
    let parts = columns
        .iter()
        .map(|name| format!("CAST({} AS CHAR) LIKE ?", quote_mysql_identifier(name)))
        .collect::<Vec<_>>();
    format!("({})", parts.join(" OR "))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn browse_result(
    table_name: &str,
    sql: String,
    limit: u32,
    offset: u32,
    total_rows: Option<u64>,
    columns: Vec<DatabaseResultColumn>,
    mut rows: Vec<Vec<Option<String>>>,
    started: Instant,
) -> DatabaseBrowseResult {
    let has_more = total_rows.map_or(rows.len() > limit as usize, |total| {
        u64::from(offset) + u64::from(limit) < total
    });
    rows.truncate(limit as usize);
    let total_rows_exact = total_rows.is_some() || (!has_more && (offset == 0 || !rows.is_empty()));
    let total_rows = total_rows.unwrap_or_else(|| {
        if rows.is_empty() {
            0
        } else {
            u64::from(offset) + rows.len() as u64 + u64::from(has_more)
        }
    });
    DatabaseBrowseResult {
        total_rows_exact,
        has_more,
        table_name: table_name.to_string(),
        sql,
        limit,
        offset,
        total_rows,
        read_only: true,
        result: DatabaseQueryResult {
            warnings: Vec::new(),
            columns,
            rows,
            affected_rows: 0,
            duration_ms: started.elapsed().as_millis(),
            safety: DatabaseQuerySafety {
                classification: "read".to_string(),
                requires_confirmation: false,
                confirmed: true,
                message: None,
            },
            truncated: false,
            row_limit: 0,
        },
    }
}

#[derive(Clone, Copy)]
pub(super) enum SqlDialect {
    /// SQLite and PostgreSQL: double-quoted identifiers, `''` escapes only.
    Standard,
    /// MySQL/MariaDB: backtick identifiers and backslash escaping.
    MySql,
}

impl SqlDialect {
    pub(super) fn quote_ident(&self, value: &str) -> String {
        match self {
            SqlDialect::Standard => quote_identifier(value),
            SqlDialect::MySql => quote_mysql_identifier(value),
        }
    }

    pub(super) fn quote_qualified(&self, schema: Option<&str>, table: &str) -> String {
        match schema {
            Some(schema) => format!("{}.{}", self.quote_ident(schema), self.quote_ident(table)),
            None => self.quote_ident(table),
        }
    }
}

pub(super) fn display_driver(driver: &str) -> &'static str {
    match driver {
        "postgres" => "PostgreSQL",
        "mysql" => "MySQL/MariaDB",
        "sqlite" => "SQLite",
        _ => "Database",
    }
}
