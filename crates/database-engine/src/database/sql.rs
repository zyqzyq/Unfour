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

#[cfg(test)]
pub(super) fn classify_query(sql: &str) -> DatabaseQuerySafety {
    classify_query_for_dialect(sql, DatabaseDialect::Generic)
}

pub(super) fn classify_query_for_dialect(
    sql: &str,
    dialect: DatabaseDialect,
) -> DatabaseQuerySafety {
    let words = super::script_parser::safety_words(sql, dialect).unwrap_or_default();
    classify_words(&words)
}

/// Shared SQL safety classification for execution and adapters.
///
/// A lexical failure becomes an unconfirmed unknown statement. Callers that
/// need a read-only decision must use [`sql_is_proven_read`] so that failure
/// stays fail-closed.
pub fn classify_sql_safety(sql: &str, dialect: DatabaseDialect) -> DatabaseQuerySafety {
    classify_query_for_dialect(sql, dialect)
}

/// True only when this dialect sees exactly one statement and classifies it as
/// a read that does not require confirmation. Tokenizer errors, multi-statement
/// scripts, and unrecognized statements are not reads.
pub fn sql_is_proven_read(sql: &str, dialect: DatabaseDialect) -> bool {
    if !sql_is_single_statement(sql, dialect) {
        return false;
    }
    let safety = classify_sql_safety(sql, dialect);
    safety.classification == "read" && !safety.requires_confirmation
}

/// `None` requires every supported dialect to prove the statement is read-only.
/// A known driver is classified only with that driver's grammar.
pub fn sql_is_proven_read_for_driver(sql: &str, driver: Option<&str>) -> bool {
    match driver.map(DatabaseDialect::for_driver) {
        Some(dialect) => sql_is_proven_read(sql, dialect),
        None => SAFETY_DIALECTS
            .into_iter()
            .all(|dialect| sql_is_proven_read(sql, dialect)),
    }
}

pub fn sql_is_single_statement(sql: &str, dialect: DatabaseDialect) -> bool {
    matches!(
        super::script_parser::split_script(sql, dialect),
        Ok(statements) if statements.len() == 1
    )
}

pub fn sql_is_single_statement_for_driver(sql: &str, driver: Option<&str>) -> bool {
    match driver.map(DatabaseDialect::for_driver) {
        Some(dialect) => sql_is_single_statement(sql, dialect),
        None => SAFETY_DIALECTS
            .into_iter()
            .all(|dialect| sql_is_single_statement(sql, dialect)),
    }
}

/// True only for one parsed UPDATE/DELETE with an outer WHERE expression.
/// Without a saved driver every supported dialect must prove the predicate;
/// comments, literals, nested queries and unrecognized grammar are no proof.
pub fn sql_has_outer_write_where_for_driver(sql: &str, driver: Option<&str>) -> bool {
    let proven = |dialect| {
        sql_is_single_statement(sql, dialect)
            && super::script_parser::has_outer_write_where(sql, dialect)
    };
    match driver.map(DatabaseDialect::for_driver) {
        Some(dialect) => proven(dialect),
        None => SAFETY_DIALECTS.into_iter().all(proven),
    }
}

const SAFETY_DIALECTS: [DatabaseDialect; 4] = [
    DatabaseDialect::Sqlite,
    DatabaseDialect::Postgres,
    DatabaseDialect::Mysql,
    DatabaseDialect::Generic,
];

fn classify_words(sql: &str) -> DatabaseQuerySafety {
    let lowered = sql.to_ascii_lowercase();
    let tokens: Vec<&str> = lowered.split_whitespace().collect();
    let safety = classify_token_slice(&tokens);
    // Keyword scan is a backstop for a wrapper the structured parse treated as
    // a read. It never promotes an unrecognized statement to a read, and it is
    // not applied to a plain SELECT so column names are not treated as writes.
    if safety.classification == "read"
        && !safety.requires_confirmation
        && matches!(leading_word(&tokens), "explain" | "with")
    {
        if let Some(flag) = detect_select_into(sql).or_else(|| detect_wrapped_write(sql)) {
            return flag;
        }
    }
    safety
}

fn classify_token_slice(tokens: &[&str]) -> DatabaseQuerySafety {
    let text = tokens.join(" ");
    match leading_word(tokens) {
        // `select` leads, but PostgreSQL `SELECT INTO table` and MySQL
        // `SELECT ... INTO OUTFILE|DUMPFILE` are writes despite the keyword.
        "select" => detect_select_into(&text).unwrap_or_else(read_safety),
        // SHOW, DESCRIBE, and DESC inspect metadata. A following statement is
        // rejected by the single-statement gate before this classification.
        "show" | "describe" | "desc" => read_safety(),
        // PRAGMA can change writable_schema, foreign_keys, journal_mode, etc.
        // Only explicitly known inspection pragmas are safe without confirmation.
        "pragma" => {
            let name = tokens
                .iter()
                .copied()
                .filter(|token| *token != "(" && *token != ")")
                .nth(1)
                .unwrap_or("");
            if [
                "table_info",
                "table_xinfo",
                "index_list",
                "index_info",
                "index_xinfo",
                "foreign_key_list",
                "database_list",
                "compile_options",
            ]
            .contains(&name)
                && !text.contains('=')
            {
                read_safety()
            } else {
                unknown_sql()
            }
        }
        // EXPLAIN and WITH are reads only when every nested statement is itself
        // a proven read. Unparseable wrappers stay unconfirmed.
        "explain" => classify_explain(tokens),
        "with" => classify_with(tokens),
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
        _ => unknown_sql(),
    }
}

fn unknown_sql() -> DatabaseQuerySafety {
    DatabaseQuerySafety {
        classification: "unknown".to_string(),
        requires_confirmation: true,
        confirmed: false,
        message: Some(
            "Unrecognized SQL statement type requires confirmation before execution.".to_string(),
        ),
    }
}

fn leading_word<'a>(tokens: &[&'a str]) -> &'a str {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "(" && *token != ")")
        .unwrap_or("")
}

fn skip_parens(tokens: &[&str], open: usize) -> Option<usize> {
    if tokens.get(open) != Some(&"(") {
        return None;
    }
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match *token {
            "(" => depth += 1,
            ")" => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn classify_explain(tokens: &[&str]) -> DatabaseQuerySafety {
    let Some(start) = tokens.iter().position(|token| *token == "explain") else {
        return unknown_sql();
    };
    if tokens[..start]
        .iter()
        .any(|token| *token != "(" && *token != ")")
    {
        return unknown_sql();
    }
    let mut index = start + 1;
    loop {
        match tokens.get(index).copied() {
            Some("(") => {
                index = match skip_parens(tokens, index) {
                    Some(next) => next,
                    None => return unknown_sql(),
                };
            }
            Some("analyze" | "verbose") => index += 1,
            Some("query") if tokens.get(index + 1) == Some(&"plan") => index += 2,
            _ => break,
        }
    }
    if index >= tokens.len() {
        return unknown_sql();
    }
    classify_token_slice(&tokens[index..])
}

fn classify_with(tokens: &[&str]) -> DatabaseQuerySafety {
    let Some(start) = tokens.iter().position(|token| *token == "with") else {
        return unknown_sql();
    };
    if tokens[..start]
        .iter()
        .any(|token| *token != "(" && *token != ")")
    {
        return unknown_sql();
    }
    let mut index = start + 1;
    if tokens.get(index) == Some(&"recursive") {
        index += 1;
    }
    let mut bodies = Vec::new();
    loop {
        if index >= tokens.len() || matches!(tokens[index], "(" | ")") {
            return unknown_sql();
        }
        index += 1;
        if tokens.get(index) == Some(&"(") {
            index = match skip_parens(tokens, index) {
                Some(next) => next,
                None => return unknown_sql(),
            };
        }
        if tokens.get(index) == Some(&"not") {
            index += 1;
        }
        if tokens.get(index) == Some(&"materialized") {
            index += 1;
        }
        if tokens.get(index) != Some(&"as") {
            return unknown_sql();
        }
        index += 1;
        if tokens.get(index) == Some(&"not") {
            index += 1;
        }
        if tokens.get(index) == Some(&"materialized") {
            index += 1;
        }
        if tokens.get(index) != Some(&"(") {
            return unknown_sql();
        }
        let open = index;
        let Some(after) = skip_parens(tokens, open) else {
            return unknown_sql();
        };
        bodies.push(&tokens[open + 1..after - 1]);
        index = after;
        if tokens.get(index) == Some(&",") {
            index += 1;
            continue;
        }
        if looks_like_cte(tokens, index) {
            continue;
        }
        break;
    }
    if index >= tokens.len() {
        return unknown_sql();
    }
    for part in bodies.into_iter().chain(std::iter::once(&tokens[index..])) {
        let safety = classify_token_slice(part);
        if safety.classification != "read" || safety.requires_confirmation {
            return safety;
        }
    }
    read_safety()
}

fn looks_like_cte(tokens: &[&str], index: usize) -> bool {
    let Some(name) = tokens.get(index).copied() else {
        return false;
    };
    if matches!(
        name,
        "(" | ")"
            | "select"
            | "insert"
            | "update"
            | "delete"
            | "replace"
            | "merge"
            | "upsert"
            | "with"
            | "explain"
            | "show"
            | "describe"
            | "desc"
            | "pragma"
            | "create"
            | "alter"
            | "drop"
            | "truncate"
            | "values"
            | "table"
            | "call"
            | "copy"
            | "begin"
            | "commit"
            | "rollback"
    ) {
        return false;
    }
    let mut cursor = index + 1;
    if tokens.get(cursor) == Some(&"(") {
        cursor = match skip_parens(tokens, cursor) {
            Some(next) => next,
            None => return false,
        };
    }
    if tokens.get(cursor) == Some(&"not") {
        cursor += 1;
    }
    if tokens.get(cursor) == Some(&"materialized") {
        cursor += 1;
    }
    tokens.get(cursor) == Some(&"as")
}

pub(super) fn display_driver(driver: &str) -> &'static str {
    match driver {
        "postgres" => "PostgreSQL",
        "mysql" => "MySQL/MariaDB",
        "sqlite" => "SQLite",
        _ => "Database",
    }
}
