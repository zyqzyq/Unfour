use serde_json::json;

use super::fixtures::registry;

#[test]
fn ssh_tool_is_registered() {
    assert!(registry()
        .definitions()
        .iter()
        .any(|d| d.name == "unfour.ssh.run_diagnostic"));
    assert!(registry()
        .definitions()
        .iter()
        .any(|d| d.name == "unfour.ssh.create_connection"));
}

#[test]
fn create_connection_stores_secret_and_returns_safe_summary() {
    let result = registry()
        .call(
            "unfour.ssh.create_connection",
            json!({
                "name": "Dev SSH",
                "host": "ssh.example.test",
                "port": 2222,
                "username": "developer",
                "authKind": "password",
                "secret": "test-ssh-password"
            }),
        )
        .expect("create connection should succeed");

    assert_eq!(result["isError"], false);
    let content = &result["structuredContent"];
    assert_eq!(content["credentialStored"], true);
    assert_eq!(content["credentialSource"], "created");
    assert_eq!(content["connection"]["id"], "created-ssh-1");
    assert_eq!(content["connection"]["name"], "Dev SSH");
    assert_eq!(content["connection"]["host"], "ssh.example.test");
    assert_eq!(content["connection"]["port"], 2222);
    assert_eq!(content["connection"]["username"], "developer");
    assert_eq!(content["connection"]["authKind"], "password");

    let serialized = serde_json::to_string(content).unwrap();
    assert!(!serialized.contains("test-ssh-password"));
    assert!(!serialized.contains("credentialRef"));
    assert!(!serialized.contains("ssh-password:cred-1"));
}

#[test]
fn create_connection_rejects_secret_and_credential_ref_together() {
    let result = registry().call(
        "unfour.ssh.create_connection",
        json!({
            "name": "Dev SSH",
            "host": "ssh.example.test",
            "username": "developer",
            "authKind": "password",
            "credentialRef": "unfour:ws-active:ssh-password:cred-1",
            "secret": "test-ssh-password"
        }),
    );
    assert!(result.is_err());
}
