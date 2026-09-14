//! Workspace-scoped connection maintenance and diagnostic projections.
//! Domain validation, persistence, credential use and execution stay in CommandBus.
use super::confirmation::ensure_confirmed_if_guarded;
use super::policy::ToolPolicyEvaluation;
use super::{
    object_with_allowed_keys, RegisteredTool, ToolAnnotations, ToolCallError, ToolDefinition,
    ToolHandler,
};
use crate::command_bus_adapter::{CommandBusAdapter, CommandBusAdapterError};
use serde_json::{json, Map, Value};
use unfour_core::models::{
    DatabaseConnectionInput, SshConnection, SshConnectionInput, SshHostKeyInput,
};

fn object(properties: Value) -> Value {
    let required: Vec<_> = properties.as_object().unwrap().keys().cloned().collect();
    json!({"type":"object", "properties":properties, "required":required, "additionalProperties":false})
}

pub(super) fn registered_tools() -> Vec<RegisteredTool> {
    let source = json!({"type":"string", "const":"command-bus"});
    let mut tools = Vec::new();
    for (name, title, handler, annotations, extra, output) in [
        (
            "unfour.db.update_connection",
            "Update Database Connection",
            update_db as ToolHandler,
            ToolAnnotations::local_write(),
            json!({
                "name":{"type":"string"}, "driver":{"type":"string","enum":["sqlite","postgres","mysql"]},
                "host":{"type":["string","null"]}, "port":{"type":["integer","null"],"minimum":1,"maximum":65535},
                "database":{"type":["string","null"]}, "username":{"type":["string","null"]},
                "sslMode":{"type":["string","null"],"enum":[null,"disable","prefer","require","verify-ca","verify-full"]},
                "sqlitePath":{"type":["string","null"]}, "readOnly":{"type":"boolean"}
            }),
            object(
                json!({"connectionId":{"type":"string"},"workspaceId":{"type":"string"},"source":source}),
            ),
        ),
        (
            "unfour.ssh.update_connection",
            "Update SSH Connection",
            update_ssh as ToolHandler,
            ToolAnnotations::local_write(),
            json!({
                "name":{"type":"string"},"host":{"type":"string"},"port":{"type":"integer","minimum":1,"maximum":65535},
                "username":{"type":"string"},"authKind":{"type":"string","enum":["password","private-key","none"]},
                "keyPath":{"type":["string","null"]}
            }),
            object(
                json!({"connectionId":{"type":"string"},"workspaceId":{"type":"string"},"source":source}),
            ),
        ),
        (
            "unfour.db.delete_connection",
            "Delete Database Connection",
            delete_db as ToolHandler,
            ToolAnnotations::local_write_destructive(),
            confirmation_schema(),
            object(
                json!({"connectionId":{"type":"string"},"workspaceId":{"type":"string"},"deleted":{"const":true},"source":source}),
            ),
        ),
        (
            "unfour.ssh.delete_connection",
            "Delete SSH Connection",
            delete_ssh as ToolHandler,
            ToolAnnotations::local_write_destructive(),
            confirmation_schema(),
            object(
                json!({"connectionId":{"type":"string"},"workspaceId":{"type":"string"},"deleted":{"const":true},"source":source}),
            ),
        ),
        (
            "unfour.ssh.test_connection",
            "Test SSH Connection",
            test_ssh as ToolHandler,
            ToolAnnotations::remote_action(),
            json!({}),
            object(
                json!({"connectionId":{"type":"string"},"workspaceId":{"type":"string"},"ok":{"type":"boolean"},"message":{"type":"string"},"source":source}),
            ),
        ),
        (
            "unfour.ssh.get_host_key",
            "Get SSH Host Fingerprint",
            host_key as ToolHandler,
            ToolAnnotations::local_read(),
            json!({}),
            object(
                json!({"connectionId":{"type":"string"},"workspaceId":{"type":"string"},"known":{"type":"boolean"},"fingerprint":{"type":["string","null"]},"source":source}),
            ),
        ),
    ] {
        let mut properties = extra.as_object().unwrap().clone();
        properties.insert("workspaceId".into(), json!({"type":"string","minLength":1}));
        properties.insert(
            "connectionId".into(),
            json!({"type":"string","minLength":1}),
        );
        tools.push(RegisteredTool { definition: ToolDefinition {
            name, title,
            description: match name {
                "unfour.db.update_connection" | "unfour.ssh.update_connection" => "Updates saved connection metadata through CommandBus. The ID must exist in the selected workspace. Omitted fields and stored credentials are preserved. Secret rotation is not exposed; the result contains only connectionId, workspaceId and source.",
                "unfour.db.delete_connection" | "unfour.ssh.delete_connection" => "Soft-deletes a saved connection through CommandBus. Guarded policy requires confirmation bound to workspace, connection and revision; read-only policy rejects deletion.",
                "unfour.ssh.test_connection" => "Tests a saved SSH connection using the existing CommandBus connection test and host-key checks. Requires native SSH. Returns only success status and a safe generic message, never credentials or raw engine error text.",
                _ => "Reads only the stored host fingerprint for a saved connection in the selected workspace. Returns known and a nullable fingerprint without contacting the host, changing trust, or exposing raw known_hosts.",
            },
            input_schema: json!({"type":"object","properties":properties,"required":["connectionId"],"additionalProperties":false}),
            output_schema: output, annotations,
        }, handler });
    }
    tools.push(RegisteredTool { definition: ToolDefinition {
        name:"unfour.db.list_history", title:"List Database Query History",
        description:"Lists workspace-scoped query history (default 50, maximum 200). SQL literals and quoted tokens are masked; statements with comments, dollar quoting or sensitive markers are withheld. Returns execution summaries, never raw errors or result rows.",
        input_schema:json!({"type":"object","properties":{"workspaceId":{"type":"string","minLength":1},"limit":{"type":"integer","minimum":1,"maximum":200,"default":50}},"additionalProperties":false}),
        output_schema: object(json!({"workspaceId":{"type":"string"},"count":{"type":"integer","minimum":0},"source":source,"history":{"type":"array","items":object(json!({
            "id":{"type":"string"},"connectionId":{"type":["string","null"]},"connectionName":{"type":"string"},
            "sql":{"type":"string"},"sqlRedacted":{"type":"boolean"},"status":{"type":"string"},
            "rowCount":{"type":["integer","null"]},"affectedRows":{"type":["integer","null"]},
            "durationMs":{"type":["integer","null"]},"hasError":{"type":"boolean"},"executedAt":{"type":"string"}
        }))}})), annotations:ToolAnnotations::local_read(),
    }, handler:list_history });
    tools
}

fn confirmation_schema() -> Value {
    json!({"confirm":{"type":"boolean"},"confirmation_text":{"type":"string"}})
}
fn error(e: CommandBusAdapterError) -> ToolCallError {
    ToolCallError::Execution {
        code: e.code,
        message: e.message,
    }
}
fn invalid() -> ToolCallError {
    ToolCallError::InvalidArguments("Invalid connection fields.".into())
}
fn missing() -> ToolCallError {
    ToolCallError::Execution {
        code: "CONNECTION_NOT_FOUND",
        message: "The connection was not found in this workspace.",
    }
}
fn args(value: Value, keys: &[&str]) -> Result<Map<String, Value>, ToolCallError> {
    let mut allowed = vec!["workspaceId", "connectionId"];
    allowed.extend(keys);
    let arguments = object_with_allowed_keys(value, &allowed)?;
    validate_workspace(&arguments)?;
    Ok(arguments)
}
fn validate_workspace(arguments: &Map<String, Value>) -> Result<(), ToolCallError> {
    if arguments
        .get("workspaceId")
        .is_some_and(|v| v.as_str().is_none_or(|s| s.trim().is_empty()))
    {
        return Err(invalid());
    }
    Ok(())
}
fn id(a: &Map<String, Value>) -> Result<String, ToolCallError> {
    a.get("connectionId")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_owned())
        .ok_or_else(invalid)
}
fn result(e: &ToolPolicyEvaluation, id: &str) -> Value {
    json!({"connectionId":id,"workspaceId":e.workspace.workspace_id,"source":"command-bus"})
}
fn ssh(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    id: &str,
) -> Result<SshConnection, ToolCallError> {
    bus.list_ssh_connections(&e.workspace.workspace_id)
        .map_err(error)?
        .into_iter()
        .find(|c| {
            c.id == id && c.workspace_id == e.workspace.workspace_id && c.deleted_at.is_none()
        })
        .ok_or_else(missing)
}
fn ssh_input(c: SshConnection) -> SshConnectionInput {
    SshConnectionInput {
        id: Some(c.id),
        workspace_id: c.workspace_id,
        name: c.name,
        host: c.host,
        port: Some(c.port),
        username: c.username,
        auth_kind: c.auth_kind,
        key_path: c.key_path,
        credential_ref: c.credential_ref,
        secret: None,
    }
}
fn merge(mut input: Value, a: Map<String, Value>) -> Value {
    for (k, v) in a {
        if k != "workspaceId" && k != "connectionId" {
            input[&k] = v;
        }
    }
    input
}
fn update_db(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        value,
        &[
            "name",
            "driver",
            "host",
            "port",
            "database",
            "username",
            "sslMode",
            "sqlitePath",
            "readOnly",
        ],
    )?;
    let id = id(&a)?;
    let c = bus
        .list_db_connections(&e.workspace.workspace_id)
        .map_err(error)?
        .into_iter()
        .find(|c| {
            c.id == id && c.workspace_id == e.workspace.workspace_id && c.deleted_at.is_none()
        })
        .ok_or_else(missing)?;
    let input = DatabaseConnectionInput {
        id: Some(c.id),
        workspace_id: c.workspace_id,
        name: c.name,
        driver: c.driver,
        host: c.host,
        port: c.port,
        database: c.database,
        username: c.username,
        ssl_mode: c.ssl_mode,
        sqlite_path: c.sqlite_path,
        credential_ref: c.credential_ref,
        read_only: c.read_only,
    };
    let input = serde_json::from_value(merge(
        serde_json::to_value(input).map_err(|_| invalid())?,
        a,
    ))
    .map_err(|_| invalid())?;
    bus.save_db_connection(input).map_err(error)?;
    Ok(result(e, &id))
}
fn update_ssh(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(
        value,
        &["name", "host", "port", "username", "authKind", "keyPath"],
    )?;
    let id = id(&a)?;
    if a.get("port").is_some_and(Value::is_null) {
        return Err(invalid());
    }
    let input = ssh_input(ssh(bus, e, &id)?);
    let input = serde_json::from_value(merge(
        serde_json::to_value(input).map_err(|_| invalid())?,
        a,
    ))
    .map_err(|_| invalid())?;
    bus.save_ssh_connection(input).map_err(error)?;
    Ok(result(e, &id))
}
fn delete(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
    is_ssh: bool,
) -> Result<Value, ToolCallError> {
    let a = args(value, &["confirm", "confirmation_text"])?;
    let id = id(&a)?;
    let revision = if is_ssh {
        ssh(bus, e, &id)?.revision
    } else {
        bus.list_db_connections(&e.workspace.workspace_id)
            .map_err(error)?
            .into_iter()
            .find(|c| {
                c.id == id && c.workspace_id == e.workspace.workspace_id && c.deleted_at.is_none()
            })
            .ok_or_else(missing)?
            .revision
    };
    ensure_confirmed_if_guarded(
        e,
        &a,
        if is_ssh {
            "SSH_DELETE_CONNECTION"
        } else {
            "DB_DELETE_CONNECTION"
        },
        "Delete this saved connection?",
        json!({"workspaceId":e.workspace.workspace_id,"connectionId":id,"revision":revision}),
    )?;
    if is_ssh {
        bus.delete_ssh_connection(&e.workspace.workspace_id, &id)
    } else {
        bus.delete_db_connection(&e.workspace.workspace_id, &id)
    }
    .map_err(error)?;
    let mut r = result(e, &id);
    r["deleted"] = json!(true);
    Ok(r)
}
fn delete_db(
    b: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    v: Value,
) -> Result<Value, ToolCallError> {
    delete(b, e, v, false)
}
fn delete_ssh(
    b: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    v: Value,
) -> Result<Value, ToolCallError> {
    delete(b, e, v, true)
}
fn test_ssh(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(value, &[])?;
    let id = id(&a)?;
    let tested = bus
        .test_ssh_connection(ssh_input(ssh(bus, e, &id)?))
        .map_err(error)?;
    let mut r = result(e, &id);
    r["ok"] = json!(tested.ok);
    // Engine failure text can contain credential/server-supplied data.
    r["message"] = json!(if tested.ok {
        "SSH connection succeeded."
    } else {
        "SSH connection failed. Inspect local diagnostics."
    });
    Ok(r)
}
fn host_key(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = args(value, &[])?;
    let id = id(&a)?;
    let c = ssh(bus, e, &id)?;
    let key = bus
        .get_ssh_host_key(SshHostKeyInput {
            workspace_id: e.workspace.workspace_id.clone(),
            host: c.host,
            port: c.port,
        })
        .map_err(error)?;
    let mut r = result(e, &id);
    r["known"] = json!(key.is_some());
    r["fingerprint"] = json!(key.map(|k| k.fingerprint));
    Ok(r)
}

fn safe_sql(sql: &str) -> String {
    // This is a display projection, not an execution parser. Withhold dialect
    // constructs we cannot safely mask; never execute this modified SQL.
    if sql.contains(['$', '#'])
        || sql.contains("--")
        || sql.contains("/*")
        || super::ssh_risk::redact_command_display(sql) != sql
    {
        "[redacted SQL]".into()
    } else {
        let mut chars = sql.chars().peekable();
        let mut result = String::new();
        while let Some(c) = chars.next() {
            if matches!(c, '\'' | '"' | '`') {
                result.push('?');
                let mut closed = false;
                while let Some(next) = chars.next() {
                    if next == '\\' {
                        chars.next();
                    } else if next == c {
                        if chars.peek() == Some(&c) {
                            chars.next();
                        } else {
                            closed = true;
                            break;
                        }
                    }
                }
                if !closed {
                    return "[redacted SQL]".into();
                }
            } else if c.is_ascii_digit() {
                result.push('?');
                while chars
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-'))
                {
                    chars.next();
                }
            } else {
                result.push(c);
            }
        }
        result.chars().take(4096).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_sql_masks_unlabelled_literals_and_dialect_constructs() {
        for sql in [
            "select 'canary'",
            "select E'canary\\\'more'",
            "select \"canary\"",
            "select `canary`",
            "select $$canary$$",
            "select 123456",
            "select /*canary*/ current_user",
            "select current_user --canary",
            "select 'unterminated-canary",
        ] {
            let masked = safe_sql(sql);
            assert!(!masked.contains("canary"), "{masked}");
            assert!(!masked.contains("123456"));
        }
        assert_eq!(
            safe_sql("SELECT count(*) FROM users"),
            "SELECT count(*) FROM users"
        );
        assert_eq!(safe_sql("SELECT 1"), "SELECT ?");
    }
}
fn list_history(
    bus: &dyn CommandBusAdapter,
    e: &ToolPolicyEvaluation,
    value: Value,
) -> Result<Value, ToolCallError> {
    let a = object_with_allowed_keys(value, &["workspaceId", "limit"])?;
    validate_workspace(&a)?;
    let limit = match a.get("limit") {
        None => 50,
        Some(v) => v
            .as_i64()
            .filter(|n| (1..=200).contains(n))
            .ok_or_else(invalid)?,
    };
    let history:Vec<_>=bus.list_db_history(&e.workspace.workspace_id,limit).map_err(error)?.into_iter()
        .filter(|h|h.workspace_id==e.workspace.workspace_id).take(limit as usize).map(|h|{
            let sql=safe_sql(&h.sql);json!({"id":h.id,"connectionId":h.connection_id,"connectionName":super::ssh_risk::redact_command_display(&h.connection_name),"sqlRedacted":sql!=h.sql,"sql":sql,"status":h.status,"rowCount":h.row_count,"affectedRows":h.affected_rows,"durationMs":h.duration_ms,"hasError":h.error.is_some(),"executedAt":h.executed_at})
        }).collect();
    Ok(
        json!({"workspaceId":e.workspace.workspace_id,"count":history.len(),"history":history,"source":"command-bus"}),
    )
}
