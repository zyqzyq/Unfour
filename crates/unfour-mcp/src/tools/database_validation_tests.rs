use serde_json::json;

use super::*;

// --- SQL validation unit tests ---

#[test]
fn validate_readonly_sql_case_insensitive() {
    assert!(validate_readonly_sql("SELECT 1").is_ok());
    assert!(validate_readonly_sql("select 1").is_ok());
    assert!(validate_readonly_sql("Select 1").is_ok());
    assert!(validate_readonly_sql("  SELECT 1  ").is_ok());
}

#[test]
fn validate_readonly_sql_rejects_empty() {
    assert!(validate_readonly_sql("").is_err());
    assert!(validate_readonly_sql("   ").is_err());
}

#[test]
fn validate_readonly_sql_rejects_transaction_control() {
    assert!(validate_readonly_sql("BEGIN").is_err());
    assert!(validate_readonly_sql("COMMIT").is_err());
    assert!(validate_readonly_sql("ROLLBACK").is_err());
}

#[test]
fn validate_readonly_sql_strips_leading_comments() {
    // Block comment followed by valid SELECT.
    assert!(validate_readonly_sql("/* comment */ SELECT 1").is_ok());
    // Line comment followed by valid SELECT.
    assert!(validate_readonly_sql("-- comment\nSELECT 1").is_ok());
    // Block comment followed by forbidden INSERT.
    assert!(validate_readonly_sql("/* comment */ INSERT INTO t VALUES (1)").is_err());
    // Multiple comments then valid query.
    assert!(validate_readonly_sql("-- a\n-- b\nSELECT 1").is_ok());
    assert!(validate_readonly_sql("/* a */ /* b */ SELECT 1").is_ok());
}

#[test]
fn validate_readonly_sql_rejects_writes_behind_explain_and_with() {
    // Genuinely read-only EXPLAIN/CTE statements remain allowed.
    assert!(validate_readonly_sql("EXPLAIN SELECT * FROM users").is_ok());
    assert!(validate_readonly_sql("EXPLAIN ANALYZE SELECT * FROM users").is_ok());
    assert!(validate_readonly_sql("WITH c AS (SELECT 1) SELECT * FROM c").is_ok());

    // EXPLAIN ANALYZE <write> and data-modifying CTEs execute in PostgreSQL,
    // so a read-only tool must reject them.
    assert!(validate_readonly_sql("EXPLAIN ANALYZE DELETE FROM users").is_err());
    assert!(validate_readonly_sql("EXPLAIN ANALYZE INSERT INTO users VALUES (1)").is_err());
    assert!(
        validate_readonly_sql("WITH d AS (DELETE FROM users RETURNING *) SELECT * FROM d").is_err()
    );
    assert!(validate_readonly_sql("EXPLAIN DROP TABLE users").is_err());
}

#[test]
fn export_filter_values_accept_string_number_and_null_only() {
    let arguments = json!({
        "filters": [
            { "column": "code", "op": "eq", "values": ["42P01"] },
            { "column": "count", "op": "eq", "values": [23000] },
            { "column": "deleted_at", "op": "eq", "values": [null] },
            { "column": "status", "op": "in", "values": ["open", 2] }
        ]
    });
    let filters = parse_export_filters(arguments.as_object().unwrap()).expect("valid filters");
    assert_eq!(filters[0].values, vec![Some("42P01".to_string())]);
    assert_eq!(filters[1].values, vec![Some("23000".to_string())]);
    assert_eq!(filters[2].values, vec![None]);
    assert_eq!(
        filters[3].values,
        vec![Some("open".to_string()), Some("2".to_string())]
    );

    for values in [json!([true]), json!([false]), json!([{"nested": 1}])] {
        let rejected = json!({
            "filters": [{ "column": "active", "op": "eq", "values": values }]
        });
        let error = parse_export_filters(rejected.as_object().unwrap())
            .expect_err("unsupported filter value");
        assert!(matches!(
            error,
            ToolCallError::InvalidArguments(message)
                if message == "export filter values must be strings, numbers, or null"
        ));
    }
}

// --- Truncation unit tests ---

#[test]
fn truncate_query_rows_preserves_small_results() {
    let rows = vec![
        vec![Some("1".to_string()), Some("a".to_string())],
        vec![Some("2".to_string()), Some("b".to_string())],
    ];
    let (kept, truncated) = truncate_query_rows(rows.clone(), 1024);
    assert_eq!(kept.len(), 2);
    assert!(!truncated);
}

#[test]
fn truncate_query_rows_truncates_at_limit() {
    let big = "x".repeat(500);
    let rows: Vec<Vec<Option<String>>> = (0..50)
        .map(|i| vec![Some(i.to_string()), Some(big.clone())])
        .collect();
    let (kept, truncated) = truncate_query_rows(rows, 1024);
    assert!(truncated);
    assert!(kept.len() < 50);
    assert!(!kept.is_empty());
}
