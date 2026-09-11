use super::*;
use sqlparser::dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::tokenizer::{Location, Token, TokenWithSpan, Tokenizer};

pub(super) struct ScriptStatement {
    pub index: usize,
    pub sql: String,
    pub start: usize,
    pub end: usize,
}

fn dialect(driver: &str) -> Box<dyn Dialect> {
    match driver {
        "postgres" => Box::new(PostgreSqlDialect {}),
        "mysql" => Box::new(MySqlDialect {}),
        "sqlite" => Box::new(SQLiteDialect {}),
        _ => Box::new(GenericDialect {}),
    }
}

pub(super) fn tokens(sql: &str, driver: &str) -> AppResult<Vec<TokenWithSpan>> {
    Tokenizer::new(dialect(driver).as_ref(), sql)
        .tokenize_with_location()
        .map_err(|error| AppError::Validation(format!("SQL lexical error: {error}")))
}

/// Words outside quoted strings/identifiers/comments, for conservative safety
/// classification only. Execution never serializes or rewrites these tokens.
pub(super) fn safety_words(sql: &str, driver: &str) -> AppResult<String> {
    Ok(tokens(sql, driver)?
        .into_iter()
        .filter_map(|item| match item.token {
            Token::Word(word) if word.quote_style.is_none() => Some(word.value),
            Token::Eq => Some("=".into()),
            Token::LParen => Some("(".into()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" "))
}

fn byte_offset(source: &str, lines: &[Vec<usize>], location: Location) -> usize {
    lines
        .get(location.line.saturating_sub(1) as usize)
        .and_then(|line| line.get(location.column.saturating_sub(1) as usize))
        .copied()
        .unwrap_or(source.len())
}

pub(super) fn split_script(source: &str, driver: &str) -> AppResult<Vec<ScriptStatement>> {
    if source.len() > 1_000_000 {
        return Err(AppError::Validation("SQL script exceeds 1 MB".into()));
    }
    if driver == "mysql" && (source.contains("/*!") || source.to_ascii_uppercase().contains("/*M!"))
    {
        return Err(AppError::Unsupported(
            "MySQL executable comments require expansion into explicit SQL before execution".into(),
        ));
    }
    let items = tokens(source, driver)?;
    // Index character positions once: repeatedly scanning a long line for
    // every token would make large INSERT statements quadratic.
    let mut lines = vec![Vec::new()];
    for (offset, ch) in source.char_indices() {
        lines.last_mut().unwrap().push(offset);
        if ch == '\n' {
            lines.push(Vec::new());
        }
    }
    lines.last_mut().unwrap().push(source.len());
    let mut statements = Vec::new();
    let mut start = None;
    let mut end = 0;
    for item in items {
        if matches!(item.token, Token::Whitespace(_)) {
            continue;
        }
        if start.is_none() && matches!(item.token, Token::SemiColon) {
            continue;
        }
        let token_start = byte_offset(source, &lines, item.span.start);
        let token_end = byte_offset(source, &lines, item.span.end);
        let statement_start = *start.get_or_insert(token_start);
        // DELIMITER is a client directive, not SQL. Reject the whole script
        // before any execution instead of splitting a routine into unsafe pieces.
        if let Token::Word(word) = &item.token {
            if token_start == statement_start && word.value.eq_ignore_ascii_case("delimiter") {
                return Err(AppError::Unsupported(
                    "MySQL DELIMITER scripts are not supported by this editor. Use a MySQL script client for routine definitions; no statement was executed.".into(),
                ));
            }
        }
        end = token_end;
        if matches!(item.token, Token::SemiColon) {
            if driver == "sqlite" {
                let fragment = std::ffi::CString::new(&source[statement_start..end])
                    .map_err(|_| AppError::Validation("SQL contains NUL".into()))?;
                // SQLite's own grammar-aware completeness check recognizes trigger
                // bodies (including CASE/END) without a second hand-written lexer.
                if unsafe { libsqlite3_sys::sqlite3_complete(fragment.as_ptr()) } == 0 {
                    continue;
                }
            }
            statements.push(ScriptStatement {
                index: statements.len() + 1,
                sql: source[statement_start..end].to_string(),
                start: source[..statement_start].encode_utf16().count(),
                end: source[..end].encode_utf16().count(),
            });
            start = None;
            if statements.len() > 1_000 {
                return Err(AppError::Validation(
                    "SQL script exceeds 1000 statements".into(),
                ));
            }
        }
    }
    if let Some(start) = start {
        statements.push(ScriptStatement {
            index: statements.len() + 1,
            sql: source[start..end].to_string(),
            start: source[..start].encode_utf16().count(),
            end: source[..end].encode_utf16().count(),
        });
    }
    if statements.len() > 1_000 {
        return Err(AppError::Validation(
            "SQL script exceeds 1000 statements".into(),
        ));
    }
    Ok(statements)
}
