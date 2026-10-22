use super::sql_policy::{connect, workspace, SqliteFile};
use super::*;
use crate::response::error_json;
use crate::tools::ToolRegistry;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use unfour_core::models::*;

/// Change the real active workspace immediately after returning the policy's
/// workspace snapshot. No timing/sleeps or concurrent scheduler are needed.
struct SwitchingBus {
    inner: Arc<LocalCommandBusAdapter>,
    next: String,
    switched: AtomicBool,
    switch_on_read: usize,
    current_reads: AtomicUsize,
    scopes: Mutex<Vec<String>>,
    executions: Mutex<Vec<DatabaseQueryInput>>,
}

impl SwitchingBus {
    fn scope(&self, id: &str) {
        self.scopes.lock().unwrap().push(id.into());
    }
}

impl CommandBusAdapter for SwitchingBus {
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        if command == ReadCommand::CurrentWorkspace {
            self.current_reads.fetch_add(1, Ordering::SeqCst);
        }
        let result = self.inner.execute_read(command)?;
        if matches!(result, ReadCommandResult::Workspaces(_))
            || (matches!(result, ReadCommandResult::CurrentWorkspace(_))
                && self.current_reads.load(Ordering::SeqCst) == self.switch_on_read)
        {
            if !self.switched.swap(true, Ordering::SeqCst) {
                self.inner
                    .run(self.inner.bus.set_active_workspace(self.next.clone()))
                    .unwrap();
            }
        }
        Ok(result)
    }

    fn execute_saved_api_request(
        &self,
        _: &str,
        _: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        unreachable!()
    }

    fn list_db_connections(
        &self,
        id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        self.scope(id);
        self.inner.list_db_connections(id)
    }

    fn get_db_schema(
        &self,
        id: &str,
        connection: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        self.scope(id);
        self.inner.get_db_schema(id, connection)
    }

    fn list_db_tables(
        &self,
        id: &str,
        connection: &str,
        catalog: Option<&str>,
        schema: Option<&str>,
        limit: u32,
    ) -> Result<DatabaseTableList, CommandBusAdapterError> {
        self.scope(id);
        self.inner
            .list_db_tables(id, connection, catalog, schema, limit)
    }

    fn get_db_table_structure(
        &self,
        input: DatabaseTableStructureInput,
    ) -> Result<DatabaseTableStructure, CommandBusAdapterError> {
        self.scope(&input.workspace_id);
        self.inner.get_db_table_structure(input)
    }

    fn execute_db_query(
        &self,
        input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        self.scope(&input.workspace_id);
        self.executions.lock().unwrap().push(input.clone());
        self.inner.execute_db_query(input)
    }

    fn save_db_connection(
        &self,
        input: DatabaseConnectionInput,
    ) -> Result<DatabaseConnection, CommandBusAdapterError> {
        self.scope(&input.workspace_id);
        self.inner.save_db_connection(input)
    }

    fn test_db_connection(
        &self,
        id: &str,
        connection: &str,
    ) -> Result<DatabaseTestResult, CommandBusAdapterError> {
        self.scope(id);
        self.inner.test_db_connection(id, connection)
    }

    fn list_db_history(
        &self,
        id: &str,
        limit: i64,
    ) -> Result<Vec<DbQueryHistoryEntry>, CommandBusAdapterError> {
        self.scope(id);
        self.inner.list_db_history(id, limit)
    }

    fn delete_db_connection(
        &self,
        id: &str,
        connection: &str,
    ) -> Result<(), CommandBusAdapterError> {
        self.scope(id);
        self.inner.delete_db_connection(id, connection)
    }
}

fn switching(inner: Arc<LocalCommandBusAdapter>, from: &str, to: &str) -> Arc<SwitchingBus> {
    inner
        .run(inner.bus.set_active_workspace(from.into()))
        .unwrap();
    Arc::new(SwitchingBus {
        inner,
        next: to.into(),
        switched: AtomicBool::new(false),
        switch_on_read: 1,
        current_reads: AtomicUsize::new(0),
        scopes: Mutex::new(Vec::new()),
        executions: Mutex::new(Vec::new()),
    })
}

#[test]
fn database_workspace_switch_after_write_policy_snapshot_never_writes_foreign_sqlite() {
    let file_a = SqliteFile::new();
    let file_b = SqliteFile::new();
    let inner = LocalCommandBusAdapter::ephemeral().unwrap();
    let a = workspace(&inner, "dev", "guarded");
    let b = workspace(&inner, "prod", "read_only");
    connect(&inner, &a, &file_a.path);
    let foreign = connect(&inner, &b, &file_b.path);
    let mut bus = switching(inner.clone(), &a, &b);
    Arc::get_mut(&mut bus).unwrap().switch_on_read = 2;
    let registry = ToolRegistry::with_command_bus(bus.clone());
    let result = registry.call("unfour.db.execute", json!({
        "connectionId": foreign, "sql": "UPDATE items SET name = 'unauthorized' WHERE id = 1"
    })).unwrap();
    assert_eq!(
        file_b.names(),
        vec!["kept"],
        "A's permission must never write B: {result}"
    );
    assert_eq!(file_a.names(), vec!["kept"]);
    assert_eq!(error_json(&result)["error"]["code"], "NOT_FOUND");
    assert_eq!(bus.current_reads.load(Ordering::SeqCst), 1);
    inner.shutdown();
}

#[test]
fn database_workspace_switch_cannot_change_authorized_sql_target() {
    for explicit in [false, true] {
        for (policy_a, policy_b) in [
            ("full_access", "read_only"),
            ("guarded", "disabled"),
            ("read_only", "full_access"),
            ("disabled", "full_access"),
        ] {
            for foreign in [false, true] {
                let file_a = SqliteFile::new();
                let file_b = SqliteFile::new();
                let inner = LocalCommandBusAdapter::ephemeral().unwrap();
                let a = workspace(&inner, "dev", policy_a);
                let b = workspace(&inner, "prod", policy_b);
                let ca = connect(&inner, &a, &file_a.path);
                let cb = connect(&inner, &b, &file_b.path);
                let bus = switching(inner.clone(), &a, &b);
                let registry = ToolRegistry::with_command_bus(bus.clone());
                let mut args = json!({"connectionId": if foreign { &cb } else { &ca }, "sql": "UPDATE items SET name = 'changed' WHERE id = 1"});
                if explicit {
                    args["workspaceId"] = json!(a);
                }
                let result = registry.call("unfour.db.execute", args).unwrap();
                let allowed = matches!(policy_a, "full_access" | "guarded");
                if !allowed {
                    assert_eq!(
                        error_json(&result)["error"]["code"],
                        "WORKSPACE_POLICY_BLOCKED"
                    );
                    assert!(
                        bus.executions.lock().unwrap().is_empty(),
                        "denial must precede execution"
                    );
                } else if foreign {
                    assert_eq!(
                        error_json(&result)["error"]["code"],
                        "NOT_FOUND",
                        "{result}"
                    );
                } else {
                    assert_eq!(result["isError"], false, "{result}");
                }
                assert_eq!(
                    file_a.names(),
                    if allowed && !foreign {
                        vec!["changed"]
                    } else {
                        vec!["kept"]
                    }
                );
                assert_eq!(file_b.names(), vec!["kept"]);
                assert!(
                    bus.scopes.lock().unwrap().iter().all(|id| id == &a),
                    "all driver/execution lookups must use A"
                );
                assert_eq!(
                    bus.current_reads.load(Ordering::SeqCst),
                    usize::from(!explicit)
                );
                inner.shutdown();
            }
        }
    }
}

#[test]
fn database_workspace_switch_keeps_read_tools_and_connection_creation_on_snapshot() {
    for explicit in [false, true] {
        let file = SqliteFile::new();
        let inner = LocalCommandBusAdapter::ephemeral().unwrap();
        let a = workspace(&inner, "dev", "full_access");
        let b = workspace(&inner, "prod", "disabled");
        let connection = connect(&inner, &a, &file.path);
        for (tool, extra) in [
            ("list_connections", json!({})),
            ("list_history", json!({})),
            ("list_tables", json!({})),
            ("describe_table", json!({"tableName":"items"})),
            ("query_readonly", json!({"sql":"SELECT name FROM items"})),
            ("explain", json!({"sql":"SELECT name FROM items"})),
            ("execute", json!({"sql":"SELECT name FROM items"})),
            ("test_connection", json!({})),
            ("update_connection", json!({"name":"renamed"})),
            (
                "create_connection",
                json!({"name":"created","driver":"sqlite","sqlitePath":file.path}),
            ),
        ] {
            let bus = switching(inner.clone(), &a, &b);
            let registry = ToolRegistry::with_command_bus(bus.clone());
            let mut args = extra;
            if !matches!(
                tool,
                "list_connections" | "list_history" | "create_connection"
            ) {
                args["connectionId"] = json!(connection);
            }
            if explicit {
                args["workspaceId"] = json!(a);
            }
            let result = registry.call(&format!("unfour.db.{tool}"), args).unwrap();
            assert_eq!(result["isError"], false, "{tool}: {result}");
            assert!(!bus.scopes.lock().unwrap().is_empty(), "{tool}");
            assert!(
                bus.scopes.lock().unwrap().iter().all(|id| id == &a),
                "{tool}"
            );
            assert_eq!(
                bus.current_reads.load(Ordering::SeqCst),
                usize::from(!explicit),
                "{tool}"
            );
        }
        inner.shutdown();
    }
}

#[test]
fn database_workspace_switch_binds_export_confirmation_and_deletion() {
    for explicit in [false, true] {
        let file = SqliteFile::new();
        let inner = LocalCommandBusAdapter::ephemeral().unwrap();
        let a = workspace(&inner, "test", "guarded");
        let b = workspace(&inner, "prod", "disabled");
        let connection = connect(&inner, &a, &file.path);
        for tool in ["export_table", "delete_connection"] {
            let bus = switching(inner.clone(), &a, &b);
            let registry = ToolRegistry::with_command_bus(bus.clone());
            let mut args = json!({"connectionId":connection});
            if tool == "export_table" {
                args["tableName"] = json!("items");
                args["content"] = json!("data");
                args["format"] = json!("json");
            }
            if explicit {
                args["workspaceId"] = json!(a);
            }
            let required = registry
                .call(&format!("unfour.db.{tool}"), args.clone())
                .unwrap();
            let error = error_json(&required);
            assert_eq!(
                error["error"]["code"], "CONFIRMATION_REQUIRED",
                "{required}"
            );
            // The wire response exposes a fingerprint, not the private payload.
            // Its token must match an explicit A call even though B is active.
            args["workspaceId"] = json!(a);
            let baseline = ToolRegistry::with_command_bus(inner.clone())
                .call(&format!("unfour.db.{tool}"), args)
                .unwrap();
            assert_eq!(
                error["confirmation_text"],
                error_json(&baseline)["confirmation_text"]
            );
            assert_eq!(
                bus.current_reads.load(Ordering::SeqCst),
                usize::from(!explicit)
            );
        }
        // Connection deletion can also be exercised under guarded with the
        // exact token from the unchanged content-bound confirmation protocol.
        let bus = switching(inner.clone(), &a, &b);
        let registry = ToolRegistry::with_command_bus(bus.clone());
        let mut args = json!({"connectionId":connection});
        if explicit {
            args["workspaceId"] = json!(a);
        }
        let required = registry
            .call("unfour.db.delete_connection", args.clone())
            .unwrap();
        args["confirm"] = json!(true);
        args["confirmation_text"] = error_json(&required)["confirmation_text"].clone();
        let retry_bus = switching(inner.clone(), &a, &b);
        let registry = ToolRegistry::with_command_bus(retry_bus.clone());
        let deleted = registry.call("unfour.db.delete_connection", args).unwrap();
        assert_eq!(deleted["isError"], false, "{deleted}");
        assert!(inner.list_db_connections(&a).unwrap().is_empty());
        assert!(bus.scopes.lock().unwrap().iter().all(|id| id == &a));
        assert!(retry_bus.scopes.lock().unwrap().iter().all(|id| id == &a));
        inner.shutdown();
    }
}

#[test]
fn database_workspace_switch_keeps_native_readonly_constraints() {
    let file = SqliteFile::new();
    let inner = LocalCommandBusAdapter::ephemeral().unwrap();
    let a = workspace(&inner, "prod", "read_only");
    let b = workspace(&inner, "dev", "full_access");
    let connection = connect(&inner, &a, &file.path);
    for explicit in [false, true] {
        for tool in ["execute", "query_readonly", "explain"] {
            let bus = switching(inner.clone(), &a, &b);
            let registry = ToolRegistry::with_command_bus(bus.clone());
            let mut args = json!({"connectionId":connection, "sql":"SELECT name FROM items"});
            if explicit {
                args["workspaceId"] = json!(a);
            }
            let result = registry.call(&format!("unfour.db.{tool}"), args).unwrap();
            assert_eq!(result["isError"], false, "{result}");
            let executions = bus.executions.lock().unwrap();
            assert_eq!(executions.len(), 1);
            assert_eq!(executions[0].workspace_id, a);
            assert!(executions[0].enforce_read_only);
            assert_eq!(executions[0].confirm_mutation, None);
        }
    }
    assert_eq!(file.names(), vec!["kept"]);
    inner.shutdown();
}
