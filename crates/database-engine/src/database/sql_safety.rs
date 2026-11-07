//! SQL safety classification shared by desktop, MCP and Flow.
use super::*;

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
