use super::*;
use crate::{
    output_schema::assert_success_matches_output_schema, response::content_json, ToolRegistry,
};
use serde_json::{json, Value};
use unfour_core::models::*;

fn workspace(adapter: &LocalCommandBusAdapter, env: &str, policy: &str) -> String {
    adapter
        .run(adapter.bus.create_workspace_with_options(
            format!("MCP {env} {policy} {}", unfour_core::id::new_id()),
            Some(env.into()),
            Some(policy.into()),
        ))
        .unwrap()
        .id
}
fn db(adapter: &LocalCommandBusAdapter, ws: &str) -> DatabaseConnection {
    adapter
        .save_db_connection(DatabaseConnectionInput {
            id: None,
            workspace_id: ws.into(),
            name: "database".into(),
            driver: "sqlite".into(),
            host: None,
            port: None,
            database: None,
            username: None,
            ssl_mode: None,
            sqlite_path: Some(":memory:".into()),
            credential_ref: None,
            read_only: false,
        })
        .unwrap()
}
fn ssh(adapter: &LocalCommandBusAdapter, ws: &str) -> SshConnection {
    adapter
        .save_ssh_connection(SshConnectionInput {
            id: None,
            workspace_id: ws.into(),
            name: "host".into(),
            host: "localhost".into(),
            port: Some(22),
            username: "test".into(),
            auth_kind: "none".into(),
            key_path: None,
            credential_ref: None,
            secret: None,
        })
        .unwrap()
}
fn success(registry: &ToolRegistry, name: &str, args: Value) -> Value {
    let result = registry.call(name, args).unwrap();
    assert_success_matches_output_schema(registry, name, &result);
    let text = result.to_string();
    for secret in [
        "credentialRef",
        "credential_ref",
        "password",
        "keyPath",
        "raw-secret",
    ] {
        assert!(!text.contains(secret), "{text}");
    }
    content_json(&result)
}

#[test]
fn connection_maintenance_is_scoped_and_deletes_require_current_confirmation() {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let ws = workspace(&adapter, "test", "auto");
    let other = workspace(&adapter, "dev", "auto");
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    for (kind, id) in [("db", db(&adapter, &ws).id), ("ssh", ssh(&adapter, &ws).id)] {
        let update = format!("unfour.{kind}.update_connection");
        let delete = format!("unfour.{kind}.delete_connection");
        success(
            &registry,
            &update,
            json!({"workspaceId":ws,"connectionId":id,"name":"renamed"}),
        );
        let mismatch = registry
            .call(
                &update,
                json!({"workspaceId":other,"connectionId":id,"name":"wrong"}),
            )
            .unwrap();
        assert_eq!(
            content_json(&mismatch)["error"]["code"],
            "CONNECTION_NOT_FOUND"
        );
        let arguments = json!({"workspaceId":ws,"connectionId":id});
        let required = registry.call(&delete, arguments.clone()).unwrap();
        assert_eq!(
            content_json(&required)["error"]["code"],
            "CONFIRMATION_REQUIRED"
        );
        let token = content_json(&required)["confirmation_text"].clone();
        assert!(registry.call(&delete,json!({"workspaceId":ws,"connectionId":id,"confirm":true,"confirmation_text":"wrong"})).unwrap()["isError"].as_bool().unwrap());
        success(
            &registry,
            &update,
            json!({"workspaceId":ws,"connectionId":id,"name":"revised"}),
        );
        let stale=registry.call(&delete,json!({"workspaceId":ws,"connectionId":id,"confirm":true,"confirmation_text":token})).unwrap();
        assert_eq!(
            content_json(&stale)["error"]["code"],
            "CONFIRMATION_REQUIRED"
        );
        let token = content_json(&stale)["confirmation_text"].clone();
        success(
            &registry,
            &delete,
            json!({"workspaceId":ws,"connectionId":id,"confirm":true,"confirmation_text":token}),
        );
        let gone = registry
            .call(
                &update,
                json!({"workspaceId":ws,"connectionId":id,"name":"cannot revive"}),
            )
            .unwrap();
        assert_eq!(content_json(&gone)["error"]["code"], "CONNECTION_NOT_FOUND");
    }
    adapter.shutdown();
}

#[test]
fn new_tools_fail_closed_for_prod_readonly_and_disabled_workspaces() {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    for (env, policy) in [("prod", "auto"), ("dev", "read_only"), ("dev", "disabled")] {
        let ws = workspace(&adapter, env, policy);
        for (kind, id) in [("db", db(&adapter, &ws).id), ("ssh", ssh(&adapter, &ws).id)] {
            for operation in ["update_connection", "delete_connection"] {
                let result = registry
                    .call(
                        &format!("unfour.{kind}.{operation}"),
                        json!({"workspaceId":ws,"connectionId":id,"confirm":true}),
                    )
                    .unwrap();
                assert_eq!(
                    content_json(&result)["error"]["code"],
                    "WORKSPACE_POLICY_BLOCKED"
                );
            }
        }
        if policy != "disabled" {
            success(
                &registry,
                "unfour.db.list_history",
                json!({"workspaceId":ws}),
            );
            success(
                &registry,
                "unfour.ssh.get_host_key",
                json!({"workspaceId":ws,"connectionId":ssh(&adapter,&ws).id}),
            );
        }
    }
    adapter.shutdown();
}

#[test]
fn database_history_masks_literals_and_limits_workspace_results() {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let ws = workspace(&adapter, "dev", "auto");
    let other = workspace(&adapter, "dev", "auto");
    for (index, scope, sql) in [
        (0, &ws, "SELECT * FROM users"),
        (1, &ws, "SELECT 'raw-secret'"),
        (2, &other, "SELECT hidden FROM other_workspace"),
    ] {
        adapter
            .run(
                adapter
                    .bus
                    .record_database_query_history(DbQueryHistoryRecordInput {
                        id: format!("history-{index}"),
                        workspace_id: scope.clone(),
                        connection_id: None,
                        connection_name: "diagnostic".into(),
                        sql: sql.into(),
                        status: "success".into(),
                        classification: Some("read".into()),
                        row_count: Some(1),
                        affected_rows: None,
                        duration_ms: Some(12),
                        error: Some("raw-secret server error".into()),
                        executed_at: format!("2026-09-14T00:00:0{index}Z"),
                    }),
            )
            .unwrap();
    }
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    let history = success(
        &registry,
        "unfour.db.list_history",
        json!({"workspaceId":ws}),
    );
    assert_eq!(history["count"], 2);
    assert!(!history.to_string().contains("other_workspace"));
    assert!(history["history"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["sql"] == "SELECT * FROM users"));
    assert_eq!(
        success(
            &registry,
            "unfour.db.list_history",
            json!({"workspaceId":ws,"limit":1})
        )["count"],
        1
    );
    for limit in [json!(0), json!(201), json!(-1), json!("50")] {
        assert!(registry
            .call(
                "unfour.db.list_history",
                json!({"workspaceId":ws,"limit":limit})
            )
            .is_err());
    }
    adapter.shutdown();
}

#[test]
fn all_new_tool_schemas_and_annotations_are_explicit() {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let registry = ToolRegistry::with_command_bus(adapter.clone());
    for name in [
        "unfour.db.list_history",
        "unfour.db.update_connection",
        "unfour.db.delete_connection",
        "unfour.ssh.update_connection",
        "unfour.ssh.delete_connection",
        "unfour.ssh.test_connection",
        "unfour.ssh.get_host_key",
    ] {
        let tool = registry
            .definitions()
            .into_iter()
            .find(|t| t.name == name)
            .unwrap();
        jsonschema::validator_for(&tool.input_schema).unwrap();
        jsonschema::validator_for(&tool.output_schema).unwrap();
        assert_eq!(tool.input_schema["additionalProperties"], false);
        assert_eq!(tool.output_schema["additionalProperties"], false);
        assert_eq!(tool.annotations.destructive_hint, name.contains("delete"));
        assert_eq!(
            tool.annotations.open_world_hint,
            name == "unfour.ssh.test_connection"
        );
        assert_eq!(
            tool.annotations.read_only_hint,
            name.ends_with("list_history") || name.ends_with("get_host_key")
        );
        assert_eq!(
            tool.annotations.idempotent_hint,
            tool.annotations.read_only_hint
        );
    }
    adapter.shutdown();
}

struct CredentialProbe {
    adapter: Arc<LocalCommandBusAdapter>,
    db: DatabaseConnection,
    ssh: SshConnection,
    saved_db: std::sync::Mutex<Option<DatabaseConnectionInput>>,
    saved_ssh: std::sync::Mutex<Option<SshConnectionInput>>,
}
impl CommandBusAdapter for CredentialProbe {
    fn execute_read(&self, c: ReadCommand) -> Result<ReadCommandResult, CommandBusAdapterError> {
        self.adapter.execute_read(c)
    }
    fn execute_saved_api_request(
        &self,
        id: &str,
        timeout: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.adapter.execute_saved_api_request(id, timeout)
    }
    fn list_db_connections(
        &self,
        _: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        Ok(vec![self.db.clone()])
    }
    fn get_db_schema(&self, w: &str, c: &str) -> Result<DatabaseSchema, CommandBusAdapterError> {
        self.adapter.get_db_schema(w, c)
    }
    fn execute_db_query(
        &self,
        input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        self.adapter.execute_db_query(input)
    }
    fn save_db_connection(
        &self,
        input: DatabaseConnectionInput,
    ) -> Result<DatabaseConnection, CommandBusAdapterError> {
        *self.saved_db.lock().unwrap() = Some(input);
        Ok(self.db.clone())
    }
    fn list_ssh_connections(&self, _: &str) -> Result<Vec<SshConnection>, CommandBusAdapterError> {
        Ok(vec![self.ssh.clone()])
    }
    fn save_ssh_connection(
        &self,
        input: SshConnectionInput,
    ) -> Result<SshConnection, CommandBusAdapterError> {
        *self.saved_ssh.lock().unwrap() = Some(input);
        Ok(self.ssh.clone())
    }
    fn test_ssh_connection(
        &self,
        input: SshConnectionInput,
    ) -> Result<SshTestResult, CommandBusAdapterError> {
        assert_eq!(
            input.credential_ref.as_deref(),
            Some("raw-secret-reference")
        );
        Ok(SshTestResult {
            ok: false,
            message: "raw-secret credentialRef password keyPath".into(),
        })
    }
    fn get_ssh_host_key(
        &self,
        input: SshHostKeyInput,
    ) -> Result<Option<SshHostFingerprintInfo>, CommandBusAdapterError> {
        assert_eq!(input.workspace_id, self.ssh.workspace_id);
        assert_eq!(input.host, self.ssh.host);
        assert_eq!(input.port, self.ssh.port);
        Ok(Some(SshHostFingerprintInfo {
            workspace_id: input.workspace_id,
            host: input.host,
            port: input.port,
            fingerprint: "SHA256:diagnosticFingerprint".into(),
            created_at: "2026-09-14T00:00:00Z".into(),
        }))
    }
}

#[test]
fn updates_preserve_credentials_and_ssh_diagnostics_return_safe_schema_results() {
    let adapter = LocalCommandBusAdapter::ephemeral().unwrap();
    let ws = workspace(&adapter, "dev", "auto");
    let mut db = db(&adapter, &ws);
    db.credential_ref = Some("raw-secret-reference".into());
    let mut ssh = ssh(&adapter, &ws);
    ssh.credential_ref = Some("raw-secret-reference".into());
    ssh.key_path = Some("private-key-path".into());
    let probe = Arc::new(CredentialProbe {
        adapter: adapter.clone(),
        db,
        ssh,
        saved_db: Default::default(),
        saved_ssh: Default::default(),
    });
    let registry = ToolRegistry::with_command_bus(probe.clone());
    success(
        &registry,
        "unfour.db.update_connection",
        json!({"workspaceId":ws,"connectionId":probe.db.id,"name":"changed"}),
    );
    success(
        &registry,
        "unfour.ssh.update_connection",
        json!({"workspaceId":ws,"connectionId":probe.ssh.id,"name":"changed"}),
    );
    assert_eq!(
        probe
            .saved_db
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .credential_ref,
        probe.db.credential_ref
    );
    let ssh_input = probe.saved_ssh.lock().unwrap();
    assert_eq!(
        ssh_input.as_ref().unwrap().credential_ref,
        probe.ssh.credential_ref
    );
    assert_eq!(ssh_input.as_ref().unwrap().key_path, probe.ssh.key_path);
    assert_eq!(
        success(
            &registry,
            "unfour.ssh.test_connection",
            json!({"workspaceId":ws,"connectionId":probe.ssh.id})
        )["ok"],
        false
    );
    let key = success(
        &registry,
        "unfour.ssh.get_host_key",
        json!({"workspaceId":ws,"connectionId":probe.ssh.id}),
    );
    assert_eq!(key["known"], true);
    assert_eq!(key["fingerprint"], "SHA256:diagnosticFingerprint");
    for (kind, id) in [("db", &probe.db.id), ("ssh", &probe.ssh.id)] {
        assert!(registry
            .call(
                &format!("unfour.{kind}.update_connection"),
                json!({"workspaceId":ws,"connectionId":id,"credentialRef":"forbidden"})
            )
            .is_err());
    }
    adapter.shutdown();
}
