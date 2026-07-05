use super::*;

#[test]
fn unknown_tool_returns_error() {
    let result = api_registry().call("unfour.api.nonexistent", json!({}));
    assert!(result.is_err());
    match result.unwrap_err() {
        ToolCallError::UnknownTool(name) => assert_eq!(name, "unfour.api.nonexistent"),
        other => panic!("expected UnknownTool, got {:?}", other),
    }
}

#[test]
fn body_truncation_works_at_20kb() {
    let large_body = "x".repeat(30_000);
    let (truncated, was_truncated) = truncate_body(&large_body, MAX_BODY_PREVIEW_BYTES);
    assert!(was_truncated);
    assert_eq!(truncated.len(), MAX_BODY_PREVIEW_BYTES);
}
