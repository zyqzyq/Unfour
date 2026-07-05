use super::*;

// --- list_tables tests ---

#[test]
fn list_tables_returns_table_summaries() {
    let result = registry()
        .call("unfour.db.list_tables", json!({ "connectionId": "conn-1" }))
        .expect("should succeed");

    let content = &result["structuredContent"];
    assert_eq!(content["connectionId"], "conn-1");
    assert_eq!(content["totalTables"], 5);
    assert_eq!(content["count"], 5);
    assert_eq!(content["truncated"], false);

    let first = &content["tables"][0];
    assert_eq!(first["name"], "users");
    assert_eq!(first["schema"], "public");
    assert_eq!(first["kind"], "table");
    assert_eq!(first["columnCount"], 3);
}

#[test]
fn list_tables_respects_limit() {
    let result = registry()
        .call(
            "unfour.db.list_tables",
            json!({ "connectionId": "conn-1", "limit": 2 }),
        )
        .expect("should succeed");

    let content = &result["structuredContent"];
    assert_eq!(content["count"], 2);
    assert_eq!(content["totalTables"], 5);
    assert_eq!(content["truncated"], true);
}

#[test]
fn list_tables_requires_connection_id() {
    let result = registry().call("unfour.db.list_tables", json!({}));
    assert!(result.is_err(), "should fail without connectionId");
}

#[test]
fn list_tables_clamps_limit_to_500() {
    let result = registry()
        .call(
            "unfour.db.list_tables",
            json!({ "connectionId": "conn-1", "limit": 9999 }),
        )
        .expect("should succeed");

    let content = &result["structuredContent"];
    // We have 5 tables, limit clamped to 500, so all 5 returned.
    assert_eq!(content["count"], 5);
    assert_eq!(content["truncated"], false);
}

// --- describe_table tests ---

#[test]
fn describe_table_returns_columns() {
    let result = registry()
        .call(
            "unfour.db.describe_table",
            json!({ "connectionId": "conn-1", "tableName": "users" }),
        )
        .expect("should succeed");

    let content = &result["structuredContent"];
    assert_eq!(content["connectionId"], "conn-1");
    let table = &content["table"];
    assert_eq!(table["name"], "users");
    assert_eq!(table["schema"], "public");
    assert_eq!(table["kind"], "table");
    assert_eq!(table["columnCount"], 3);

    let id_col = &table["columns"][0];
    assert_eq!(id_col["name"], "id");
    assert_eq!(id_col["dataType"], "integer");
    assert_eq!(id_col["nullable"], false);
    assert_eq!(id_col["primaryKey"], true);
}

#[test]
fn describe_table_with_schema_filter() {
    let result = registry()
        .call(
            "unfour.db.describe_table",
            json!({ "connectionId": "conn-1", "tableName": "events", "schema": "analytics" }),
        )
        .expect("should succeed");

    let content = &result["structuredContent"];
    assert_eq!(content["table"]["name"], "events");
    assert_eq!(content["table"]["schema"], "analytics");
    assert_eq!(content["table"]["kind"], "view");
}

#[test]
fn describe_table_not_found_returns_error() {
    let result = registry()
        .call(
            "unfour.db.describe_table",
            json!({ "connectionId": "conn-1", "tableName": "nonexistent" }),
        )
        .expect("should return error result");

    assert_eq!(result["isError"], true);
    assert_eq!(
        result["structuredContent"]["error"]["code"],
        "TABLE_NOT_FOUND"
    );
}

#[test]
fn describe_table_requires_table_name() {
    let result = registry().call(
        "unfour.db.describe_table",
        json!({ "connectionId": "conn-1" }),
    );
    assert!(result.is_err(), "should fail without tableName");
}
