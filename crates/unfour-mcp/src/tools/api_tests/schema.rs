use super::*;

// --- Schema tests ---

#[test]
fn api_tools_are_registered() {
    let definitions = api_registry().definitions();
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.api.list_collections"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.api.list_requests"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.api.get_request"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.api.send_request"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.api.create_environment"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.api.update_environment"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.api.delete_environment"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.api.set_environment_variable"));
    assert!(definitions
        .iter()
        .any(|d| d.name == "unfour.api.delete_environment_variable"));
}

#[test]
fn api_tools_have_valid_input_schemas() {
    let definitions = api_registry().definitions();
    for name in &[
        "unfour.api.list_collections",
        "unfour.api.list_requests",
        "unfour.api.get_request",
        "unfour.api.send_request",
        "unfour.api.create_environment",
        "unfour.api.update_environment",
        "unfour.api.delete_environment",
        "unfour.api.set_environment_variable",
        "unfour.api.delete_environment_variable",
    ] {
        let def = definitions.iter().find(|d| d.name == *name).unwrap();
        assert_eq!(
            def.input_schema["type"], "object",
            "{} should have object input schema",
            name
        );
    }
}
