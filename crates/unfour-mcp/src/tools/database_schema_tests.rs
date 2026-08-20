use serde_json::json;

use super::super::ToolDefinition;
use super::registered_tools;

fn definitions() -> Vec<ToolDefinition> {
    registered_tools()
        .into_iter()
        .map(|tool| tool.definition)
        .collect()
}

// --- Schema / registration tests ---

#[test]
fn db_tools_are_registered() {
    let definitions = definitions();
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.db.create_connection"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.db.list_connections"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.db.list_tables"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.db.describe_table"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.db.query_readonly"));
    assert!(definitions.iter().any(|d| d.name == "unfour.db.execute"));
    assert!(definitions.iter().any(|d| d.name == "unfour.db.explain"));
}

#[test]
fn db_create_connection_input_schema() {
    let definitions = definitions();
    let tool = definitions
        .iter()
        .find(|d| d.name == "unfour.db.create_connection")
        .unwrap();
    assert_eq!(tool.input_schema["type"], "object");
    let required = tool.input_schema["required"].as_array().unwrap();
    assert_eq!(required, &vec![json!("name"), json!("driver")]);
    assert!(tool.input_schema["properties"]["password"].is_object());
    assert!(tool.input_schema["properties"]["credentialRef"].is_object());
    assert_eq!(
        tool.input_schema["properties"]["driver"]["enum"]
            .as_array()
            .unwrap(),
        &vec![json!("sqlite"), json!("postgres"), json!("mysql")]
    );
}

#[test]
fn db_list_connections_input_schema() {
    let definitions = definitions();
    let tool = definitions
        .iter()
        .find(|d| d.name == "unfour.db.list_connections")
        .unwrap();
    assert_eq!(tool.input_schema["type"], "object");
    assert!(tool.input_schema["properties"]["workspaceId"].is_object());
}

#[test]
fn db_list_tables_input_schema() {
    let definitions = definitions();
    let tool = definitions
        .iter()
        .find(|d| d.name == "unfour.db.list_tables")
        .unwrap();
    assert_eq!(tool.input_schema["type"], "object");
    assert_eq!(
        tool.input_schema["required"].as_array().unwrap(),
        &vec![json!("connectionId")]
    );
}

#[test]
fn db_describe_table_input_schema() {
    let definitions = definitions();
    let tool = definitions
        .iter()
        .find(|d| d.name == "unfour.db.describe_table")
        .unwrap();
    assert_eq!(tool.input_schema["type"], "object");
    let required = tool.input_schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("connectionId")));
    assert!(required.contains(&json!("tableName")));
}

#[test]
fn db_query_readonly_input_schema_includes_catalog_schema_timeout() {
    let definitions = definitions();
    let tool = definitions
        .iter()
        .find(|d| d.name == "unfour.db.query_readonly")
        .unwrap();
    let properties = &tool.input_schema["properties"];
    assert!(properties["catalog"].is_object());
    assert!(properties["schema"].is_object());
    assert!(properties["timeoutMs"].is_object());
}

#[test]
fn db_list_tables_output_schema_includes_catalog() {
    let definitions = definitions();
    let tool = definitions
        .iter()
        .find(|d| d.name == "unfour.db.list_tables")
        .unwrap();
    let item_properties = &tool.output_schema["properties"]["tables"]["items"]["properties"];
    assert!(item_properties["catalog"].is_object());
    assert!(item_properties["schema"].is_object());
}

#[test]
fn db_describe_table_output_schema_includes_catalog() {
    let definitions = definitions();
    let tool = definitions
        .iter()
        .find(|d| d.name == "unfour.db.describe_table")
        .unwrap();
    let table_properties = &tool.output_schema["properties"]["table"]["properties"];
    assert!(table_properties["catalog"].is_object());
    assert!(table_properties["schema"].is_object());
}

#[test]
fn db_execute_output_schema_includes_truncated_and_transaction() {
    let definitions = definitions();
    let tool = definitions
        .iter()
        .find(|d| d.name == "unfour.db.execute")
        .unwrap();
    let properties = &tool.output_schema["properties"];
    assert!(properties["truncated"].is_object());
    assert!(properties["transaction"].is_object());
}
