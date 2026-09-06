//! Classify inbound Protocol 5 entities before canonical parsing.
//!
//! The transport intentionally preserves raw entity names. This module is the
//! tolerant-reader boundary that distinguishes malformed known data from data
//! introduced by a newer client, which can be skipped without blocking the
//! rest of a remote page.

use serde_json::Value;

use crate::{
    RemoteChange, SnapshotItem, SyncEntityType, SyncError, SyncOperation, PAYLOAD_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteEntityDisposition {
    Apply(SyncEntityType),
    SkipUnknownEntity,
    SkipUnsupportedPayload,
}

pub(crate) fn remote_change_disposition(
    change: &RemoteChange,
) -> Result<RemoteEntityDisposition, SyncError> {
    remote_entity_disposition(
        &change.entity_type,
        change.operation,
        change.payload_schema_version,
        change.payload.as_ref(),
    )
}

pub(crate) fn snapshot_item_disposition(
    item: &SnapshotItem,
) -> Result<RemoteEntityDisposition, SyncError> {
    remote_entity_disposition(
        &item.entity_type,
        SyncOperation::Upsert,
        item.payload_schema_version,
        Some(&item.payload),
    )
}

fn remote_entity_disposition(
    raw_entity_type: &str,
    operation: SyncOperation,
    payload_schema_version: i64,
    payload: Option<&Value>,
) -> Result<RemoteEntityDisposition, SyncError> {
    let Ok(entity_type) = SyncEntityType::parse(raw_entity_type) else {
        return Ok(RemoteEntityDisposition::SkipUnknownEntity);
    };
    if payload_schema_version < 1 {
        return Err(SyncError::InvalidData);
    }
    // A known tombstone is schema-independent. Its envelope is sufficient to
    // apply the delete even when the deleted payload came from a future schema.
    if operation == SyncOperation::Delete {
        return payload
            .is_none()
            .then_some(RemoteEntityDisposition::Apply(entity_type))
            .ok_or(SyncError::InvalidData);
    }
    if payload_schema_version > PAYLOAD_SCHEMA_VERSION {
        return Ok(RemoteEntityDisposition::SkipUnsupportedPayload);
    }
    let payload = payload.ok_or(SyncError::InvalidData)?;
    if payload_has_unknown_fields(entity_type, payload)?
        || payload_has_unsupported_subtype(entity_type, payload)?
    {
        return Ok(RemoteEntityDisposition::SkipUnsupportedPayload);
    }
    Ok(RemoteEntityDisposition::Apply(entity_type))
}

fn payload_has_unknown_fields(
    entity_type: SyncEntityType,
    payload: &Value,
) -> Result<bool, SyncError> {
    let object = payload.as_object().ok_or(SyncError::InvalidData)?;
    let allowed: &[&str] = match entity_type {
        SyncEntityType::Workspace => &[
            "name",
            "environmentType",
            "mcpPolicy",
            "createdAt",
            "updatedAt",
            "deletedAt",
        ],
        SyncEntityType::Connection => &[
            "id",
            "workspaceId",
            "connectionType",
            "name",
            "host",
            "port",
            "config",
            "createdAt",
            "updatedAt",
        ],
        SyncEntityType::WorkspaceVariable | SyncEntityType::WorkspaceEnvironmentVariable => &[
            "key",
            "value",
            "isSecret",
            "isEnabled",
            "description",
            "sortOrder",
            "createdAt",
            "updatedAt",
            "deletedAt",
        ],
        SyncEntityType::WorkspaceEnvironment => {
            &["name", "sortOrder", "createdAt", "updatedAt", "deletedAt"]
        }
        SyncEntityType::ApiCollection => &["name", "description", "createdAt", "updatedAt"],
        SyncEntityType::ApiFolder => &[
            "collectionId",
            "parentFolderId",
            "name",
            "sortOrder",
            "createdAt",
            "updatedAt",
        ],
        SyncEntityType::ApiRequest => &[
            "collectionId",
            "parentFolderId",
            "name",
            "sortOrder",
            "authJson",
            "method",
            "url",
            "headers",
            "query",
            "body",
            "bodyKind",
            "settingsJson",
            "preRequestScript",
            "postResponseScript",
            "scriptSchemaVersion",
            "createdAt",
            "updatedAt",
        ],
        SyncEntityType::SshTask => &["name", "description", "sortOrder", "createdAt", "updatedAt"],
        SyncEntityType::SshTaskStep => &[
            "taskId",
            "name",
            "stepType",
            "position",
            "enabled",
            "configVersion",
            "configJson",
            "createdAt",
            "updatedAt",
        ],
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Ok(true);
    }
    if entity_type == SyncEntityType::Connection {
        let Some(config) = object.get("config") else {
            return Ok(false);
        };
        let config = config.as_object().ok_or(SyncError::InvalidData)?;
        let kind = string_field(config, "kind")?;
        let allowed_config: &[&str] = match kind {
            Some("ssh") => &["kind", "username", "authMethod"],
            Some("database") => &[
                "kind",
                "driver",
                "databaseName",
                "username",
                "sslMode",
                "readOnly",
            ],
            _ => return Ok(false),
        };
        if config
            .keys()
            .any(|key| !allowed_config.contains(&key.as_str()))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn payload_has_unsupported_subtype(
    entity_type: SyncEntityType,
    payload: &Value,
) -> Result<bool, SyncError> {
    let object = payload.as_object().ok_or(SyncError::InvalidData)?;
    match entity_type {
        SyncEntityType::Workspace => Ok(string_field(object, "environmentType")?
            .is_some_and(|value| !matches!(value, "dev" | "test" | "prod"))
            || string_field(object, "mcpPolicy")?.is_some_and(|value| {
                !matches!(
                    value,
                    "auto" | "disabled" | "read_only" | "guarded" | "full_access"
                )
            })),
        SyncEntityType::Connection => connection_has_unsupported_subtype(object),
        SyncEntityType::ApiRequest => api_request_has_unsupported_subtype(object),
        SyncEntityType::SshTaskStep => ssh_task_step_has_unsupported_subtype(object),
        SyncEntityType::WorkspaceVariable
        | SyncEntityType::WorkspaceEnvironment
        | SyncEntityType::WorkspaceEnvironmentVariable
        | SyncEntityType::ApiCollection
        | SyncEntityType::ApiFolder
        | SyncEntityType::SshTask => Ok(false),
    }
}

fn connection_has_unsupported_subtype(
    object: &serde_json::Map<String, Value>,
) -> Result<bool, SyncError> {
    let connection_type = string_field(object, "connectionType")?;
    if connection_type.is_some_and(|value| !matches!(value, "ssh" | "database")) {
        return Ok(true);
    }
    let Some(config) = object.get("config") else {
        return Ok(false);
    };
    let config = config.as_object().ok_or(SyncError::InvalidData)?;
    let config_kind = string_field(config, "kind")?;
    if config_kind.is_some_and(|value| !matches!(value, "ssh" | "database")) {
        return Ok(true);
    }
    if connection_type.is_some() && config_kind.is_some() && connection_type != config_kind {
        return Err(SyncError::InvalidData);
    }
    match config_kind.or(connection_type) {
        Some("ssh") => Ok(string_field(config, "authMethod")?
            .is_some_and(|value| !matches!(value, "password" | "private-key" | "none"))),
        Some("database") => {
            let driver_is_unknown = string_field(config, "driver")?
                .is_some_and(|value| !matches!(value, "sqlite" | "postgres" | "mysql"));
            let ssl_mode_is_unknown = string_field(config, "sslMode")?.is_some_and(|value| {
                !matches!(
                    value,
                    "disable" | "prefer" | "require" | "verify-ca" | "verify-full"
                )
            });
            Ok(driver_is_unknown || ssl_mode_is_unknown)
        }
        None => Ok(false),
        Some(_) => unreachable!("unknown connection subtype returned above"),
    }
}

fn api_request_has_unsupported_subtype(
    object: &serde_json::Map<String, Value>,
) -> Result<bool, SyncError> {
    if string_field(object, "bodyKind")?.is_some_and(|value| {
        !matches!(
            value.to_ascii_lowercase().as_str(),
            "none"
                | "json"
                | "text"
                | "raw:text"
                | "form-urlencoded"
                | "x-www-form-urlencoded"
                | "urlencoded"
        )
    }) {
        return Ok(true);
    }
    if let Some(version) = integer_field(object, "scriptSchemaVersion")? {
        if version < 1 {
            return Err(SyncError::InvalidData);
        }
        if version > 1 {
            return Ok(true);
        }
    }
    let Some(auth_json) = string_field(object, "authJson")? else {
        return Ok(false);
    };
    if auth_json == unfour_core::redaction::REDACTED_VALUE {
        return Ok(false);
    }
    let auth: Value = serde_json::from_str(auth_json).map_err(|_| SyncError::InvalidData)?;
    let auth = auth.as_object().ok_or(SyncError::InvalidData)?;
    match auth.get("type") {
        None => Ok(false),
        Some(Value::String(value)) => Ok(!matches!(
            value.as_str(),
            "none" | "bearer" | "basic" | "api-key"
        )),
        Some(_) => Err(SyncError::InvalidData),
    }
}

fn ssh_task_step_has_unsupported_subtype(
    object: &serde_json::Map<String, Value>,
) -> Result<bool, SyncError> {
    if string_field(object, "stepType")?
        .is_some_and(|step_type| !matches!(step_type, "command" | "upload" | "download"))
    {
        return Ok(true);
    }
    if let Some(version) = integer_field(object, "configVersion")? {
        if version < 1 {
            return Err(SyncError::InvalidData);
        }
        if version > 1 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, SyncError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(SyncError::InvalidData),
    }
}

fn integer_field(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<i64>, SyncError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_i64().map(Some).ok_or(SyncError::InvalidData),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_entity_payload_schema_is_deferred_without_changing_protocol_5() {
        let change = RemoteChange {
            cursor: 1,
            operation_id: "future-schema".into(),
            entity_type: SyncEntityType::WorkspaceVariable.as_str().into(),
            entity_id: "variable-future".into(),
            parent_entity_id: Some("workspace".into()),
            operation: SyncOperation::Upsert,
            server_version: 1,
            payload_schema_version: PAYLOAD_SCHEMA_VERSION + 1,
            payload: Some(serde_json::json!({"future": true})),
            deleted_at: None,
        };

        assert_eq!(
            remote_change_disposition(&change).unwrap(),
            RemoteEntityDisposition::SkipUnsupportedPayload
        );
        assert_eq!(crate::PROTOCOL_VERSION, 5);
    }
}
