use unfour_command_bus::{
    classify_sql_safety, sql_is_proven_read_for_driver, sql_is_single_statement_for_driver,
    DatabaseDialect,
};
use unfour_core::models::DatabaseQuerySafety;

use super::*;

// --- SQL validation ---

#[cfg(test)]
pub(super) fn validate_readonly_sql(sql: &str) -> Result<(), ToolCallError> {
    validate_readonly_sql_for_driver(sql, None)
}

/// Read-only SQL must be a single statement the database engine can prove is a
/// read. `driver` is the saved connection driver when the statement is not a
/// proven read in every dialect; `None` keeps the all-dialect fail-closed check.
pub(super) fn validate_readonly_sql_for_driver(
    sql: &str,
    driver: Option<&str>,
) -> Result<(), ToolCallError> {
    if sql.trim().is_empty() {
        return Err(ToolCallError::Execution {
            code: "READONLY_SQL_REJECTED",
            message: "SQL cannot be empty.",
        });
    }
    if sql_is_proven_read(sql, driver) {
        Ok(())
    } else {
        Err(ToolCallError::Execution {
            code: "READONLY_SQL_REJECTED",
            message: "Only read-only SQL is permitted. Wrapped writes, SELECT INTO, and assignment PRAGMA statements are not read-only.",
        })
    }
}

pub(super) fn sql_is_proven_read(sql: &str, driver: Option<&str>) -> bool {
    sql_is_proven_read_for_driver(sql, None)
        || driver.is_some_and(|driver| sql_is_proven_read_for_driver(sql, Some(driver)))
}

pub(super) fn validate_single_statement_for_execute(
    sql: &str,
    driver: Option<&str>,
) -> Result<(), ToolCallError> {
    let single = match driver {
        Some(driver) => sql_is_single_statement_for_driver(sql, Some(driver)),
        None => sql_is_single_statement_for_driver(sql, None),
    };
    if single {
        Ok(())
    } else {
        Err(ToolCallError::Execution {
            code: "UNSUPPORTED_OPERATION",
            message: "db_execute accepts exactly one SQL statement; split multi-statement SQL into separate confirmed calls.",
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SqlRisk {
    pub(super) statement_type: &'static str,
    pub(super) requires_confirmation: bool,
    pub(super) confirmation_code: &'static str,
    pub(super) reason: &'static str,
}

pub(super) fn classify_sql_risk(sql: &str, driver: Option<&str>) -> SqlRisk {
    let safety = authoritative_safety(sql, driver);
    if safety.classification == "read" && !safety.requires_confirmation {
        return SqlRisk {
            statement_type: "read",
            requires_confirmation: false,
            confirmation_code: "READ_SQL",
            reason: "Read-only SQL.",
        };
    }

    let stripped = strip_leading_comments(sql);
    let keyword = stripped
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let lower = stripped.to_ascii_lowercase();
    // Ordinary dev/test writes keep their previous confirmation rules. Anything
    // the engine cannot prove is one of those statements, including wrappers,
    // still requires confirmation before a write flag is sent.
    match (safety.classification.as_str(), keyword.as_str()) {
        ("mutation", "delete") if !contains_where(&lower) => SqlRisk {
            statement_type: "mutation",
            requires_confirmation: true,
            confirmation_code: "DELETE_WITHOUT_WHERE",
            reason: "DELETE without WHERE is dangerous and may affect an entire table.",
        },
        ("mutation", "update") if !contains_where(&lower) => SqlRisk {
            statement_type: "mutation",
            requires_confirmation: true,
            confirmation_code: "UPDATE_WITHOUT_WHERE",
            reason: "UPDATE without WHERE is dangerous and may affect an entire table.",
        },
        ("mutation", "insert" | "update" | "delete" | "replace") => SqlRisk {
            statement_type: "mutation",
            requires_confirmation: false,
            confirmation_code: "MUTATION_SQL",
            reason: "Data mutation SQL.",
        },
        ("schema-change", "drop") => SqlRisk {
            statement_type: "schema-change",
            requires_confirmation: true,
            confirmation_code: "DROP_SQL",
            reason: "DROP can remove schema objects or databases.",
        },
        ("schema-change", "truncate") => SqlRisk {
            statement_type: "schema-change",
            requires_confirmation: true,
            confirmation_code: "TRUNCATE_SQL",
            reason: "TRUNCATE can delete all rows in a table.",
        },
        ("schema-change", "alter") => SqlRisk {
            statement_type: "schema-change",
            requires_confirmation: true,
            confirmation_code: "ALTER_SQL",
            reason: "ALTER changes table or database structure.",
        },
        ("schema-change", "create" | "vacuum" | "reindex") => SqlRisk {
            statement_type: "schema-change",
            requires_confirmation: false,
            confirmation_code: "SCHEMA_SQL",
            reason: "Schema-changing SQL.",
        },
        _ => SqlRisk {
            statement_type: statement_type(&safety.classification),
            requires_confirmation: true,
            confirmation_code: "UNKNOWN_SQL",
            reason: "This SQL is not a proven read and requires confirmation before execution.",
        },
    }
}

fn authoritative_safety(sql: &str, driver: Option<&str>) -> DatabaseQuerySafety {
    let dialects = match driver.map(DatabaseDialect::for_driver) {
        Some(dialect) => vec![dialect],
        None => vec![
            DatabaseDialect::Sqlite,
            DatabaseDialect::Postgres,
            DatabaseDialect::Mysql,
            DatabaseDialect::Generic,
        ],
    };
    let mut read = None;
    for dialect in dialects {
        let safety = classify_sql_safety(sql, dialect);
        if safety.classification != "read" || safety.requires_confirmation {
            return safety;
        }
        read = Some(safety);
    }
    read.unwrap_or_else(|| DatabaseQuerySafety {
        classification: "unknown".to_string(),
        requires_confirmation: true,
        confirmed: false,
        message: None,
    })
}

fn statement_type(classification: &str) -> &'static str {
    match classification {
        "read" => "read",
        "mutation" => "mutation",
        "schema-change" => "schema-change",
        "transaction-control" => "transaction-control",
        _ => "unknown",
    }
}

pub(super) fn contains_where(lower_sql: &str) -> bool {
    lower_sql
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|token| token == "where")
}

/// Strip leading SQL line comments (`--`) and block comments (`/* ... */`).
pub(super) fn strip_leading_comments(sql: &str) -> String {
    let mut s = sql.trim();
    loop {
        if s.starts_with("--") {
            // Line comment: skip to end of line.
            if let Some(pos) = s.find('\n') {
                s = s[pos + 1..].trim();
            } else {
                return String::new();
            }
        } else if s.starts_with("/*") {
            // Block comment: skip to closing `*/`.
            if let Some(pos) = s.find("*/") {
                s = s[pos + 2..].trim();
            } else {
                return String::new();
            }
        } else {
            break;
        }
    }
    s.to_string()
}
