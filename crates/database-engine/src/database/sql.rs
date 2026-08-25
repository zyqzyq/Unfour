use super::*;

pub(super) fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
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
    format!("`{}`", value.replace('`', "``"))
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
    total_rows: u64,
    columns: Vec<DatabaseResultColumn>,
    rows: Vec<Vec<Option<String>>>,
    started: Instant,
) -> DatabaseBrowseResult {
    DatabaseBrowseResult {
        table_name: table_name.to_string(),
        sql,
        limit,
        offset,
        total_rows,
        read_only: true,
        result: DatabaseQueryResult {
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

pub(super) fn returns_rows(sql: &str) -> bool {
    let keyword = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        keyword.as_str(),
        "select" | "with" | "pragma" | "explain" | "show"
    ) && detect_select_into(sql).is_none()
}

pub(super) fn validate_single_statement(sql: &str) -> AppResult<()> {
    let trimmed = sql.trim();
    let without_trailing = trimmed.trim_end_matches(';').trim_end();
    if without_trailing.contains(';') {
        return Err(AppError::Validation(
            "only one SQL statement can be executed at a time".to_string(),
        ));
    }
    Ok(())
}

/// Data-modifying keywords used to detect a write hidden behind an
/// `EXPLAIN`/`WITH` wrapper. PostgreSQL executes `EXPLAIN ANALYZE <write>` and
/// data-modifying CTEs (`WITH t AS (DELETE ... RETURNING *) ...`), both of which
/// the leading keyword alone would misread as a safe, no-confirmation read.
const WRITE_KEYWORDS: &[&str] = &["insert", "update", "delete", "replace", "merge", "upsert"];
const SCHEMA_KEYWORDS: &[&str] = &[
    "create", "alter", "drop", "truncate", "vacuum", "reindex", "grant", "revoke",
];

/// Scan a statement's tokens for a data-modifying or schema-changing keyword.
/// Returns the matching safety classification when one is found. This errs
/// toward over-detection: a keyword appearing inside a string literal only
/// triggers an extra confirmation prompt, it never lets a real write through.
pub(super) fn detect_wrapped_write(sql: &str) -> Option<DatabaseQuerySafety> {
    let mut has_write = false;
    let mut has_schema = false;
    for token in sql.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if token.is_empty() {
            continue;
        }
        let lowered = token.to_ascii_lowercase();
        if SCHEMA_KEYWORDS.contains(&lowered.as_str()) {
            has_schema = true;
        } else if WRITE_KEYWORDS.contains(&lowered.as_str()) {
            has_write = true;
        }
    }

    let classification = if has_schema {
        "schema-change"
    } else if has_write {
        "mutation"
    } else {
        return None;
    };

    Some(DatabaseQuerySafety {
        classification: classification.to_string(),
        requires_confirmation: true,
        confirmed: false,
        message: Some(
            "This statement can modify data or schema despite its EXPLAIN/WITH prefix. Confirm to execute it."
                .to_string(),
        ),
    })
}

/// Classify a `SELECT ... INTO` statement. PostgreSQL's `SELECT INTO table`
/// creates a new table (a schema change) and MySQL's
/// `SELECT ... INTO OUTFILE|DUMPFILE` writes to the server filesystem. Both are
/// writes that must require confirmation and must be blocked on read-only
/// connections, even though the leading keyword is `select`.
///
/// A bare `SELECT ... INTO @var` (MySQL session-variable assignment) is
/// harmless and is intentionally NOT flagged, so it still runs as a read. The
/// scan keeps looking past an `@var` target in case a later `INTO` writes a
/// table or file.
pub(super) fn detect_select_into(sql: &str) -> Option<DatabaseQuerySafety> {
    let lowered = sql.to_ascii_lowercase();
    let tokens: Vec<&str> = lowered
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '@')
        .filter(|t| !t.is_empty())
        .collect();

    let mut idx = 0;
    while idx < tokens.len() {
        if tokens[idx] == "select" {
            let mut j = idx + 1;
            while j < tokens.len() {
                if tokens[j] == "into" {
                    let target = tokens.get(j + 1).copied().unwrap_or("");
                    if target == "outfile" || target == "dumpfile" {
                        return Some(DatabaseQuerySafety {
                            classification: "mutation".to_string(),
                            requires_confirmation: true,
                            confirmed: false,
                            message: Some(
                                "This statement writes to the server filesystem (INTO OUTFILE/DUMPFILE). Confirm to execute it."
                                    .to_string(),
                            ),
                        });
                    }
                    if !target.starts_with('@') {
                        return Some(DatabaseQuerySafety {
                            classification: "schema-change".to_string(),
                            requires_confirmation: true,
                            confirmed: false,
                            message: Some(
                                "This SELECT INTO statement creates a table (schema change). Confirm to execute it."
                                    .to_string(),
                            ),
                        });
                    }
                }
                j += 1;
            }
        }
        idx += 1;
    }
    None
}

pub(super) fn read_safety() -> DatabaseQuerySafety {
    DatabaseQuerySafety {
        classification: "read".to_string(),
        requires_confirmation: false,
        confirmed: true,
        message: None,
    }
}

pub(super) fn classify_query(sql: &str) -> DatabaseQuerySafety {
    let keyword = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();

    match keyword.as_str() {
        // `select` leads, but PostgreSQL `SELECT INTO table` and MySQL
        // `SELECT ... INTO OUTFILE|DUMPFILE` are writes despite the keyword.
        "select" => detect_select_into(sql).unwrap_or_else(read_safety),
        "pragma" | "show" => read_safety(),
        // EXPLAIN and WITH can wrap a statement that actually writes (EXPLAIN
        // ANALYZE <write>, data-modifying CTEs, and SELECT ... INTO table /
        // INTO OUTFILE in PostgreSQL/MySQL), so look past the wrapper before
        // trusting them as no-confirmation reads.
        "explain" | "with" => detect_select_into(sql)
            .or_else(|| detect_wrapped_write(sql))
            .unwrap_or_else(read_safety),
        "insert" | "update" | "delete" | "replace" => DatabaseQuerySafety {
            classification: "mutation".to_string(),
            requires_confirmation: true,
            confirmed: false,
            message: Some("This SQL statement may change data. Confirm to execute it.".to_string()),
        },
        "create" | "alter" | "drop" | "truncate" | "vacuum" | "reindex" => DatabaseQuerySafety {
            classification: "schema-change".to_string(),
            requires_confirmation: true,
            confirmed: false,
            message: Some(
                "This SQL statement may change schema or database storage. Confirm to execute it."
                    .to_string(),
            ),
        },
        "begin" | "commit" | "rollback" => DatabaseQuerySafety {
            classification: "transaction-control".to_string(),
            requires_confirmation: true,
            confirmed: false,
            message: Some(
                "Transaction control statements require confirmation in this editor.".to_string(),
            ),
        },
        _ => DatabaseQuerySafety {
            classification: "unknown".to_string(),
            requires_confirmation: true,
            confirmed: false,
            message: Some(
                "Unrecognized SQL statement type requires confirmation before execution."
                    .to_string(),
            ),
        },
    }
}

pub(super) fn sql_with_limit(sql: &str, limit: u32) -> String {
    let trimmed = sql.trim().trim_end_matches(';');
    let lower = trimmed.to_ascii_lowercase();
    if (lower.starts_with("select") || lower.starts_with("with")) && !lower.contains(" limit ") {
        format!("{} LIMIT {}", trimmed, limit)
    } else {
        trimmed.to_string()
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
