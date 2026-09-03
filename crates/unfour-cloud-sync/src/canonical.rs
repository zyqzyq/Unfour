use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqliteConnection;
use unfour_core::domain::{
    ConnectionSnapshotConfig, DomainEntityKey, DomainEntityType, DomainMutation, DomainSnapshot,
    ExternalApiCollectionApply, ExternalApiCollectionUpsert, ExternalApiFolderApply,
    ExternalApiFolderUpsert, ExternalApiRequestApply, ExternalApiRequestUpsert, ExternalApplyPage,
    ExternalConnectionApply, ExternalConnectionUpsert, ExternalDelete, ExternalSshTaskApply,
    ExternalSshTaskStepApply, ExternalSshTaskStepUpsert, ExternalSshTaskUpsert,
    ExternalVariableValue, ExternalWorkspaceApply, ExternalWorkspaceEnvironmentApply,
    ExternalWorkspaceEnvironmentUpsert, ExternalWorkspaceEnvironmentVariableApply,
    ExternalWorkspaceEnvironmentVariableUpsert, ExternalWorkspaceUpsert,
    ExternalWorkspaceVariableApply, ExternalWorkspaceVariableUpsert, SnapshotVariableValue,
};
use unfour_core::models::KeyValue;

use crate::{
    RemoteChange, SnapshotItem, SyncEntityType, SyncError, SyncOperation, PAYLOAD_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspacePayload {
    name: String,
    environment_type: String,
    mcp_policy: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

/// Protocol-v4 Connection payload. This is deliberately an allowlist rather
/// than a serialized local connection so device-local credentials and paths
/// cannot cross the Cloud Sync boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConnectionPayload {
    id: String,
    workspace_id: String,
    connection_type: String,
    name: String,
    host: Option<String>,
    port: Option<u16>,
    config: ConnectionSnapshotConfig,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VariablePayload {
    key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    is_secret: bool,
    is_enabled: bool,
    description: Option<String>,
    sort_order: i64,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

impl std::fmt::Debug for VariablePayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VariablePayload")
            .field("key", &self.key)
            .field("value", &self.value.as_ref().map(|_| "[REDACTED]"))
            .field("is_secret", &self.is_secret)
            .field("is_enabled", &self.is_enabled)
            .field("description", &self.description)
            .field("sort_order", &self.sort_order)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("deleted_at", &self.deleted_at)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnvironmentPayload {
    name: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApiCollectionPayload {
    name: String,
    description: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApiFolderPayload {
    collection_id: String,
    parent_folder_id: Option<String>,
    name: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApiRequestPayload {
    collection_id: String,
    parent_folder_id: Option<String>,
    name: String,
    sort_order: i64,
    auth_json: String,
    method: String,
    url: String,
    headers: Vec<KeyValue>,
    query: Vec<KeyValue>,
    body: Option<String>,
    body_kind: String,
    #[serde(default = "default_api_request_settings_json")]
    settings_json: String,
    pre_request_script: Option<String>,
    post_response_script: Option<String>,
    script_schema_version: i64,
    created_at: String,
    updated_at: String,
}

fn default_api_request_settings_json() -> String {
    r#"{"timeoutMs":null}"#.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SshTaskPayload {
    name: String,
    description: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SshTaskStepPayload {
    task_id: String,
    name: String,
    step_type: String,
    position: i64,
    enabled: bool,
    config_version: i64,
    config_json: Value,
    created_at: String,
    updated_at: String,
}

fn snapshot_value(
    value: SnapshotVariableValue,
    is_secret: bool,
) -> Result<Option<String>, SyncError> {
    match (is_secret, value) {
        (true, SnapshotVariableValue::SecretRedacted) => Ok(None),
        (false, SnapshotVariableValue::Plain(value)) => Ok(Some(value)),
        _ => Err(SyncError::InvalidData),
    }
}

/// Converts a Core snapshot into the Cloud Sync intrinsic payload. The transaction
/// hook uses `canonical_intent_on` below so intrinsic fields are captured from
/// the same committed SQLite view; this helper is for safe UI display.
pub fn canonical_payload(snapshot: DomainSnapshot) -> Result<Option<Value>, SyncError> {
    let value = match snapshot {
        DomainSnapshot::Workspace(snapshot) => serde_json::to_value(WorkspacePayload {
            name: snapshot.name,
            environment_type: snapshot.environment_type,
            mcp_policy: snapshot.mcp_policy,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
            deleted_at: None,
        }),
        DomainSnapshot::Connection(snapshot) => serde_json::to_value(ConnectionPayload {
            id: snapshot.id,
            workspace_id: snapshot.workspace_id,
            connection_type: snapshot.connection_type,
            name: snapshot.name,
            host: snapshot.host,
            port: snapshot.port,
            config: snapshot.config,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        }),
        DomainSnapshot::WorkspaceVariable(snapshot) => {
            let value = snapshot_value(snapshot.value, snapshot.is_secret)?;
            serde_json::to_value(VariablePayload {
                key: snapshot.key,
                value,
                is_secret: snapshot.is_secret,
                is_enabled: snapshot.is_enabled,
                description: snapshot.description,
                sort_order: snapshot.sort_order,
                created_at: snapshot.created_at,
                updated_at: snapshot.updated_at,
                deleted_at: None,
            })
        }
        DomainSnapshot::WorkspaceEnvironment(snapshot) => {
            serde_json::to_value(EnvironmentPayload {
                name: snapshot.name,
                sort_order: snapshot.sort_order,
                created_at: snapshot.created_at,
                updated_at: snapshot.updated_at,
                deleted_at: None,
            })
        }
        DomainSnapshot::WorkspaceEnvironmentVariable(snapshot) => {
            let value = snapshot_value(snapshot.value, snapshot.is_secret)?;
            serde_json::to_value(VariablePayload {
                key: snapshot.key,
                value,
                is_secret: snapshot.is_secret,
                is_enabled: snapshot.is_enabled,
                description: snapshot.description,
                sort_order: snapshot.sort_order,
                created_at: snapshot.created_at,
                updated_at: snapshot.updated_at,
                deleted_at: None,
            })
        }
        DomainSnapshot::ApiCollection(snapshot) => serde_json::to_value(ApiCollectionPayload {
            name: snapshot.name,
            description: snapshot.description,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        }),
        DomainSnapshot::ApiFolder(snapshot) => serde_json::to_value(ApiFolderPayload {
            collection_id: snapshot.collection_id,
            parent_folder_id: snapshot.parent_folder_id,
            name: snapshot.name,
            sort_order: snapshot.sort_order,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        }),
        DomainSnapshot::ApiRequest(snapshot) => serde_json::to_value(ApiRequestPayload {
            collection_id: snapshot.collection_id,
            parent_folder_id: snapshot.parent_folder_id,
            name: snapshot.name,
            sort_order: snapshot.sort_order,
            auth_json: snapshot.auth_json,
            method: snapshot.method,
            url: snapshot.url,
            headers: snapshot.headers,
            query: snapshot.query,
            body: snapshot.body,
            body_kind: snapshot.body_kind,
            settings_json: snapshot.settings_json,
            pre_request_script: snapshot.pre_request_script,
            post_response_script: snapshot.post_response_script,
            script_schema_version: snapshot.script_schema_version,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        }),
        DomainSnapshot::SshTask(snapshot) => serde_json::to_value(SshTaskPayload {
            name: snapshot.name,
            description: snapshot.description,
            sort_order: snapshot.sort_order,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        }),
        DomainSnapshot::SshTaskStep(snapshot) => serde_json::to_value(SshTaskStepPayload {
            task_id: snapshot.task_id,
            name: snapshot.name,
            step_type: snapshot.step_type,
            position: snapshot.position,
            enabled: snapshot.enabled,
            config_version: snapshot.config_version,
            config_json: snapshot.config_json,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        }),
        DomainSnapshot::Tombstone(_) => return Ok(None),
    }
    .map_err(|_| SyncError::InvalidData)?;
    Ok(Some(value))
}

#[derive(Clone)]
pub(crate) struct CanonicalIntent {
    pub entity_type: SyncEntityType,
    pub parent_entity_id: Option<String>,
    pub operation: SyncOperation,
    pub payload_json: Option<String>,
    pub deleted_at: Option<String>,
}

pub(crate) struct CanonicalSnapshotIntent {
    pub entity: DomainEntityKey,
    pub revision: i64,
    pub intent: CanonicalIntent,
}

/// Builds wire content exclusively from Core's domain snapshot. API request
/// and SSH Task config fields have already crossed Core's canonical/redaction
/// boundary before Pro sees them.
pub(crate) fn canonical_snapshot_intent(
    snapshot: DomainSnapshot,
) -> Result<CanonicalSnapshotIntent, SyncError> {
    let (mut entity, revision, deleted_at) = match &snapshot {
        DomainSnapshot::Workspace(value) => (
            DomainEntityKey::new(DomainEntityType::Workspace, &value.id, &value.id),
            value.revision,
            None,
        ),
        DomainSnapshot::Connection(value) => (
            DomainEntityKey::new(DomainEntityType::Connection, &value.workspace_id, &value.id),
            value.revision,
            None,
        ),
        DomainSnapshot::WorkspaceVariable(value) => (
            DomainEntityKey::new(
                DomainEntityType::WorkspaceVariable,
                &value.workspace_id,
                &value.id,
            ),
            value.revision,
            None,
        ),
        DomainSnapshot::WorkspaceEnvironment(value) => (
            DomainEntityKey::new(
                DomainEntityType::WorkspaceEnvironment,
                &value.workspace_id,
                &value.id,
            ),
            value.revision,
            None,
        ),
        DomainSnapshot::WorkspaceEnvironmentVariable(value) => (
            DomainEntityKey::new(
                DomainEntityType::WorkspaceEnvironmentVariable,
                &value.workspace_id,
                &value.id,
            )
            .with_parent_entity_id(&value.environment_id),
            value.revision,
            None,
        ),
        DomainSnapshot::ApiCollection(value) => (
            DomainEntityKey::new(
                DomainEntityType::ApiCollection,
                &value.workspace_id,
                &value.id,
            ),
            value.revision,
            None,
        ),
        DomainSnapshot::ApiFolder(value) => (
            DomainEntityKey::new(DomainEntityType::ApiFolder, &value.workspace_id, &value.id)
                .with_parent_entity_id(
                    value
                        .parent_folder_id
                        .as_deref()
                        .unwrap_or(&value.collection_id),
                ),
            value.revision,
            None,
        ),
        DomainSnapshot::ApiRequest(value) => (
            DomainEntityKey::new(DomainEntityType::ApiRequest, &value.workspace_id, &value.id)
                .with_parent_entity_id(
                    value
                        .parent_folder_id
                        .as_deref()
                        .unwrap_or(&value.collection_id),
                ),
            value.revision,
            None,
        ),
        DomainSnapshot::SshTask(value) => (
            DomainEntityKey::new(DomainEntityType::SshTask, &value.workspace_id, &value.id),
            value.revision,
            None,
        ),
        DomainSnapshot::SshTaskStep(value) => (
            DomainEntityKey::new(
                DomainEntityType::SshTaskStep,
                &value.workspace_id,
                &value.id,
            )
            .with_parent_entity_id(&value.task_id),
            value.revision,
            None,
        ),
        DomainSnapshot::Tombstone(value) => (
            value.entity.clone(),
            value.revision,
            Some(value.deleted_at.clone()),
        ),
    };
    let entity_type = SyncEntityType::from(entity.entity_type);
    entity.parent_entity_id = canonical_parent(
        entity_type,
        &entity.workspace_id,
        entity.parent_entity_id.as_deref(),
    )?;
    let operation = if deleted_at.is_some() {
        SyncOperation::Delete
    } else {
        SyncOperation::Upsert
    };
    let payload_json = canonical_payload(snapshot)?
        .map(|payload| serde_json::to_string(&payload).map_err(|_| SyncError::InvalidData))
        .transpose()?;
    Ok(CanonicalSnapshotIntent {
        entity: entity.clone(),
        revision,
        intent: CanonicalIntent {
            entity_type,
            parent_entity_id: entity.parent_entity_id,
            operation,
            payload_json,
            deleted_at,
        },
    })
}

/// Captures an immutable canonical intent inside the CommandBus transaction.
/// Secret rows deliberately omit `value`; no reusable secret can enter SQLite.
pub(crate) async fn canonical_intent_on(
    connection: &mut SqliteConnection,
    mutation: &DomainMutation,
) -> Result<CanonicalIntent, SyncError> {
    let entity_type = SyncEntityType::from(mutation.entity.entity_type);
    let operation = SyncOperation::from(mutation.operation);
    if matches!(
        entity_type,
        SyncEntityType::Connection
            | SyncEntityType::ApiCollection
            | SyncEntityType::ApiFolder
            | SyncEntityType::ApiRequest
            | SyncEntityType::SshTask
            | SyncEntityType::SshTaskStep
    ) {
        return Ok(CanonicalIntent {
            entity_type,
            parent_entity_id: canonical_parent(
                entity_type,
                &mutation.entity.workspace_id,
                mutation.entity.parent_entity_id.as_deref(),
            )?,
            operation,
            payload_json: None,
            deleted_at: None,
        });
    }
    if operation == SyncOperation::Delete {
        let deleted_at = match entity_type {
            SyncEntityType::Workspace => sqlx::query_scalar::<_, String>(
                "SELECT deleted_at FROM workspaces WHERE id = ?1",
            )
            .bind(&mutation.entity.entity_id)
            .fetch_one(&mut *connection)
            .await?,
            SyncEntityType::WorkspaceVariable => sqlx::query_scalar::<_, String>(
                "SELECT deleted_at FROM workspace_variables WHERE id = ?1 AND workspace_id = ?2",
            )
            .bind(&mutation.entity.entity_id)
            .bind(&mutation.entity.workspace_id)
            .fetch_one(&mut *connection)
            .await?,
            SyncEntityType::WorkspaceEnvironment => sqlx::query_scalar::<_, String>(
                "SELECT deleted_at FROM workspace_environments WHERE id = ?1 AND workspace_id = ?2",
            )
            .bind(&mutation.entity.entity_id)
            .bind(&mutation.entity.workspace_id)
            .fetch_one(&mut *connection)
            .await?,
            SyncEntityType::WorkspaceEnvironmentVariable => sqlx::query_scalar::<_, String>(
                "SELECT deleted_at FROM workspace_environment_variables WHERE id = ?1 AND workspace_id = ?2",
            )
            .bind(&mutation.entity.entity_id)
            .bind(&mutation.entity.workspace_id)
            .fetch_one(&mut *connection)
            .await?,
            SyncEntityType::Connection
            | SyncEntityType::ApiCollection
            | SyncEntityType::ApiFolder
            | SyncEntityType::ApiRequest
            | SyncEntityType::SshTask
            | SyncEntityType::SshTaskStep => {
                unreachable!("Core domain intents are snapshot-materialized")
            }
        };
        return Ok(CanonicalIntent {
            entity_type,
            parent_entity_id: protocol_parent(connection, mutation).await?,
            operation,
            payload_json: None,
            deleted_at: Some(deleted_at),
        });
    }

    let payload = match entity_type {
        SyncEntityType::Workspace => {
            let row: (String, String, String, String, String, Option<String>) = sqlx::query_as(
                r#"SELECT name, environment_type, mcp_policy,
                           created_at, updated_at, deleted_at
                    FROM workspaces WHERE id = ?1"#,
            )
            .bind(&mutation.entity.entity_id)
            .fetch_one(&mut *connection)
            .await?;
            serde_json::to_value(WorkspacePayload {
                name: row.0,
                environment_type: row.1,
                mcp_policy: row.2,
                created_at: row.3,
                updated_at: row.4,
                deleted_at: row.5,
            })
        }
        SyncEntityType::WorkspaceVariable => {
            let row: (
                String,
                String,
                bool,
                bool,
                Option<String>,
                i64,
                String,
                String,
                Option<String>,
            ) = sqlx::query_as(
                r#"SELECT key, value, is_secret, is_enabled, description, sort_order,
                           created_at, updated_at, deleted_at
                    FROM workspace_variables WHERE id = ?1 AND workspace_id = ?2"#,
            )
            .bind(&mutation.entity.entity_id)
            .bind(&mutation.entity.workspace_id)
            .fetch_one(&mut *connection)
            .await?;
            serde_json::to_value(VariablePayload {
                key: row.0,
                value: (!row.2).then_some(row.1),
                is_secret: row.2,
                is_enabled: row.3,
                description: row.4,
                sort_order: row.5,
                created_at: row.6,
                updated_at: row.7,
                deleted_at: row.8,
            })
        }
        SyncEntityType::WorkspaceEnvironment => {
            let row: (String, i64, String, String, Option<String>) = sqlx::query_as(
                r#"SELECT environment.name, environment.sort_order,
                           environment.created_at, environment.updated_at, environment.deleted_at
                    FROM workspace_environments AS environment
                    WHERE environment.id = ?1 AND environment.workspace_id = ?2"#,
            )
            .bind(&mutation.entity.entity_id)
            .bind(&mutation.entity.workspace_id)
            .fetch_one(&mut *connection)
            .await?;
            serde_json::to_value(EnvironmentPayload {
                name: row.0,
                sort_order: row.1,
                created_at: row.2,
                updated_at: row.3,
                deleted_at: row.4,
            })
        }
        SyncEntityType::WorkspaceEnvironmentVariable => {
            let row: (
                String,
                String,
                bool,
                bool,
                Option<String>,
                i64,
                String,
                String,
                Option<String>,
            ) = sqlx::query_as(
                r#"SELECT key, value, is_secret, is_enabled, description, sort_order,
                           created_at, updated_at, deleted_at
                    FROM workspace_environment_variables
                    WHERE id = ?1 AND workspace_id = ?2"#,
            )
            .bind(&mutation.entity.entity_id)
            .bind(&mutation.entity.workspace_id)
            .fetch_one(&mut *connection)
            .await?;
            serde_json::to_value(VariablePayload {
                key: row.0,
                value: (!row.2).then_some(row.1),
                is_secret: row.2,
                is_enabled: row.3,
                description: row.4,
                sort_order: row.5,
                created_at: row.6,
                updated_at: row.7,
                deleted_at: row.8,
            })
        }
        SyncEntityType::Connection
        | SyncEntityType::ApiCollection
        | SyncEntityType::ApiFolder
        | SyncEntityType::ApiRequest
        | SyncEntityType::SshTask
        | SyncEntityType::SshTaskStep => {
            unreachable!("Core domain intents are snapshot-materialized")
        }
    }
    .map_err(|_| SyncError::InvalidData)?;
    Ok(CanonicalIntent {
        entity_type,
        parent_entity_id: protocol_parent(connection, mutation).await?,
        operation,
        payload_json: Some(serde_json::to_string(&payload).map_err(|_| SyncError::InvalidData)?),
        deleted_at: None,
    })
}

async fn protocol_parent(
    connection: &mut SqliteConnection,
    mutation: &DomainMutation,
) -> Result<Option<String>, SyncError> {
    Ok(match mutation.entity.entity_type {
        DomainEntityType::Workspace => None,
        DomainEntityType::Connection => None,
        DomainEntityType::WorkspaceVariable
        | DomainEntityType::WorkspaceEnvironment
        | DomainEntityType::ApiCollection => Some(mutation.entity.workspace_id.clone()),
        DomainEntityType::SshTask => None,
        DomainEntityType::WorkspaceEnvironmentVariable => {
            if let Some(parent) = &mutation.entity.parent_entity_id {
                Some(parent.clone())
            } else {
                Some(
                    sqlx::query_scalar::<_, String>(
                        "SELECT environment_id FROM workspace_environment_variables WHERE id = ?1 AND workspace_id = ?2",
                    )
                    .bind(&mutation.entity.entity_id)
                    .bind(&mutation.entity.workspace_id)
                    .fetch_one(&mut *connection)
                    .await?,
                )
            }
        }
        DomainEntityType::ApiFolder
        | DomainEntityType::ApiRequest
        | DomainEntityType::SshTaskStep => canonical_parent(
            SyncEntityType::from(mutation.entity.entity_type),
            &mutation.entity.workspace_id,
            mutation.entity.parent_entity_id.as_deref(),
        )?,
    })
}

fn canonical_parent(
    entity_type: SyncEntityType,
    workspace_id: &str,
    parent: Option<&str>,
) -> Result<Option<String>, SyncError> {
    match entity_type {
        SyncEntityType::Workspace => Ok(None),
        SyncEntityType::Connection => Ok(None),
        SyncEntityType::WorkspaceVariable
        | SyncEntityType::WorkspaceEnvironment
        | SyncEntityType::ApiCollection => Ok(Some(workspace_id.to_string())),
        SyncEntityType::SshTask => Ok(None),
        SyncEntityType::WorkspaceEnvironmentVariable
        | SyncEntityType::ApiFolder
        | SyncEntityType::ApiRequest
        | SyncEntityType::SshTaskStep => parent
            .filter(|value| !value.trim().is_empty())
            .map(|value| Some(value.to_string()))
            .ok_or(SyncError::InvalidData),
    }
}

fn external_value(
    is_secret: bool,
    value: Option<String>,
) -> Result<ExternalVariableValue, SyncError> {
    match (is_secret, value) {
        (true, None) => Ok(ExternalVariableValue::PreserveLocal),
        (false, Some(value)) => Ok(ExternalVariableValue::Set(value)),
        _ => Err(SyncError::InvalidData),
    }
}

pub fn parse_snapshot_item(
    workspace_id: &str,
    item: &SnapshotItem,
) -> Result<ExternalApplyPage, SyncError> {
    parse_remote_change(
        workspace_id,
        &RemoteChange {
            cursor: 0,
            operation_id: "snapshot".to_string(),
            entity_type: item.entity_type,
            entity_id: item.entity_id.clone(),
            parent_entity_id: item.parent_entity_id.clone(),
            operation: SyncOperation::Upsert,
            server_version: item.server_version,
            payload_schema_version: item.payload_schema_version,
            payload: Some(item.payload.clone()),
            deleted_at: None,
        },
    )
}

pub(crate) fn snapshot_workspace_name(
    workspace_id: &str,
    item: &SnapshotItem,
) -> Result<Option<String>, SyncError> {
    if item.entity_type != SyncEntityType::Workspace {
        return Ok(None);
    }
    let page = parse_snapshot_item(workspace_id, item)?;
    match page.workspaces.into_iter().next() {
        Some(ExternalWorkspaceApply::Upsert(workspace)) => Ok(Some(workspace.name)),
        _ => Err(SyncError::InvalidData),
    }
}

/// Strict Cloud Sync payload parser. Most entity identities come only from the
/// envelope. Protocol-v4 Connection payloads repeat their aggregate identity;
/// those fields must exactly match the envelope and target workspace.
pub fn parse_remote_change(
    workspace_id: &str,
    change: &RemoteChange,
) -> Result<ExternalApplyPage, SyncError> {
    if change.entity_id.trim().is_empty()
        || change.server_version < 1
        || change.payload_schema_version != PAYLOAD_SCHEMA_VERSION
    {
        return Err(SyncError::InvalidData);
    }
    validate_parent(
        workspace_id,
        change.entity_type,
        &change.entity_id,
        change.parent_entity_id.as_deref(),
    )?;
    if change.operation == SyncOperation::Delete {
        if change.payload.is_some() {
            return Err(SyncError::InvalidData);
        }
        let deleted_at = change.deleted_at.clone().ok_or(SyncError::InvalidData)?;
        let delete = ExternalDelete {
            entity: change.key(workspace_id),
            deleted_at,
        };
        return Ok(delete_page(change.entity_type, delete));
    }
    if change.deleted_at.is_some() {
        return Err(SyncError::InvalidData);
    }
    let payload = change.payload.clone().ok_or(SyncError::InvalidData)?;
    let mut page = ExternalApplyPage::default();
    match change.entity_type {
        SyncEntityType::Workspace => {
            let payload: WorkspacePayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            if payload.deleted_at.is_some() {
                return Err(SyncError::InvalidData);
            }
            page.workspaces
                .push(ExternalWorkspaceApply::Upsert(ExternalWorkspaceUpsert {
                    id: change.entity_id.clone(),
                    name: payload.name,
                    environment_type: payload.environment_type,
                    mcp_policy: payload.mcp_policy,
                    created_at: payload.created_at,
                    updated_at: payload.updated_at,
                }));
        }
        SyncEntityType::Connection => {
            let payload: ConnectionPayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            if payload.id != change.entity_id || payload.workspace_id != workspace_id {
                return Err(SyncError::InvalidData);
            }
            page.connections
                .push(ExternalConnectionApply::Upsert(ExternalConnectionUpsert {
                    id: payload.id,
                    workspace_id: payload.workspace_id,
                    connection_type: payload.connection_type,
                    name: payload.name,
                    host: payload.host,
                    port: payload.port,
                    config: payload.config,
                    created_at: payload.created_at,
                    updated_at: payload.updated_at,
                }));
        }
        SyncEntityType::WorkspaceVariable => {
            let payload: VariablePayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            if payload.deleted_at.is_some() {
                return Err(SyncError::InvalidData);
            }
            page.workspace_variables
                .push(ExternalWorkspaceVariableApply::Upsert(
                    ExternalWorkspaceVariableUpsert {
                        id: change.entity_id.clone(),
                        workspace_id: workspace_id.to_string(),
                        key: payload.key,
                        value: external_value(payload.is_secret, payload.value)?,
                        is_secret: payload.is_secret,
                        is_enabled: payload.is_enabled,
                        description: payload.description,
                        sort_order: payload.sort_order,
                        created_at: payload.created_at,
                        updated_at: payload.updated_at,
                    },
                ));
        }
        SyncEntityType::WorkspaceEnvironment => {
            let payload: EnvironmentPayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            if payload.deleted_at.is_some() {
                return Err(SyncError::InvalidData);
            }
            page.workspace_environments
                .push(ExternalWorkspaceEnvironmentApply::Upsert(
                    ExternalWorkspaceEnvironmentUpsert {
                        id: change.entity_id.clone(),
                        workspace_id: workspace_id.to_string(),
                        name: payload.name,
                        sort_order: payload.sort_order,
                        created_at: payload.created_at,
                        updated_at: payload.updated_at,
                    },
                ));
        }
        SyncEntityType::WorkspaceEnvironmentVariable => {
            let payload: VariablePayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            if payload.deleted_at.is_some() {
                return Err(SyncError::InvalidData);
            }
            page.workspace_environment_variables.push(
                ExternalWorkspaceEnvironmentVariableApply::Upsert(
                    ExternalWorkspaceEnvironmentVariableUpsert {
                        id: change.entity_id.clone(),
                        workspace_id: workspace_id.to_string(),
                        environment_id: change
                            .parent_entity_id
                            .clone()
                            .ok_or(SyncError::InvalidData)?,
                        key: payload.key,
                        value: external_value(payload.is_secret, payload.value)?,
                        is_secret: payload.is_secret,
                        is_enabled: payload.is_enabled,
                        description: payload.description,
                        sort_order: payload.sort_order,
                        created_at: payload.created_at,
                        updated_at: payload.updated_at,
                    },
                ),
            );
        }
        SyncEntityType::ApiCollection => {
            let payload: ApiCollectionPayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            page.api_collections
                .push(ExternalApiCollectionApply::Upsert(
                    ExternalApiCollectionUpsert {
                        id: change.entity_id.clone(),
                        workspace_id: workspace_id.to_string(),
                        name: payload.name,
                        description: payload.description,
                        created_at: payload.created_at,
                        updated_at: payload.updated_at,
                    },
                ));
        }
        SyncEntityType::ApiFolder => {
            let payload: ApiFolderPayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            let effective_parent = payload
                .parent_folder_id
                .as_deref()
                .unwrap_or(&payload.collection_id);
            if change.parent_entity_id.as_deref() != Some(effective_parent) {
                return Err(SyncError::InvalidData);
            }
            page.api_folders
                .push(ExternalApiFolderApply::Upsert(ExternalApiFolderUpsert {
                    id: change.entity_id.clone(),
                    workspace_id: workspace_id.to_string(),
                    collection_id: payload.collection_id,
                    parent_folder_id: payload.parent_folder_id,
                    name: payload.name,
                    sort_order: payload.sort_order,
                    created_at: payload.created_at,
                    updated_at: payload.updated_at,
                }));
        }
        SyncEntityType::ApiRequest => {
            let payload: ApiRequestPayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            let effective_parent = payload
                .parent_folder_id
                .as_deref()
                .unwrap_or(&payload.collection_id);
            if change.parent_entity_id.as_deref() != Some(effective_parent) {
                return Err(SyncError::InvalidData);
            }
            page.api_requests
                .push(ExternalApiRequestApply::Upsert(Box::new(
                    ExternalApiRequestUpsert {
                        id: change.entity_id.clone(),
                        workspace_id: workspace_id.to_string(),
                        collection_id: payload.collection_id,
                        parent_folder_id: payload.parent_folder_id,
                        name: payload.name,
                        sort_order: payload.sort_order,
                        auth_json: payload.auth_json,
                        method: payload.method,
                        url: payload.url,
                        headers: payload.headers,
                        query: payload.query,
                        body: payload.body,
                        body_kind: payload.body_kind,
                        settings_json: payload.settings_json,
                        pre_request_script: payload.pre_request_script,
                        post_response_script: payload.post_response_script,
                        script_schema_version: payload.script_schema_version,
                        created_at: payload.created_at,
                        updated_at: payload.updated_at,
                    },
                )));
        }
        SyncEntityType::SshTask => {
            let payload: SshTaskPayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            page.ssh_tasks
                .push(ExternalSshTaskApply::Upsert(ExternalSshTaskUpsert {
                    id: change.entity_id.clone(),
                    workspace_id: workspace_id.to_string(),
                    name: payload.name,
                    description: payload.description,
                    sort_order: payload.sort_order,
                    created_at: payload.created_at,
                    updated_at: payload.updated_at,
                }));
        }
        SyncEntityType::SshTaskStep => {
            let payload: SshTaskStepPayload =
                serde_json::from_value(payload).map_err(|_| SyncError::InvalidData)?;
            if change.parent_entity_id.as_deref() != Some(payload.task_id.as_str()) {
                return Err(SyncError::InvalidData);
            }
            page.ssh_task_steps.push(ExternalSshTaskStepApply::Upsert(
                ExternalSshTaskStepUpsert {
                    id: change.entity_id.clone(),
                    workspace_id: workspace_id.to_string(),
                    task_id: payload.task_id,
                    name: payload.name,
                    step_type: payload.step_type,
                    position: payload.position,
                    enabled: payload.enabled,
                    config_version: payload.config_version,
                    config_json: payload.config_json,
                    created_at: payload.created_at,
                    updated_at: payload.updated_at,
                },
            ));
        }
    }
    Ok(page)
}

fn validate_parent(
    workspace_id: &str,
    entity_type: SyncEntityType,
    entity_id: &str,
    parent: Option<&str>,
) -> Result<(), SyncError> {
    let valid = match entity_type {
        SyncEntityType::Workspace => entity_id == workspace_id && parent.is_none(),
        SyncEntityType::Connection => parent.is_none(),
        SyncEntityType::WorkspaceVariable | SyncEntityType::WorkspaceEnvironment => {
            parent == Some(workspace_id)
        }
        SyncEntityType::WorkspaceEnvironmentVariable => {
            parent.is_some_and(|value| !value.trim().is_empty())
        }
        SyncEntityType::ApiCollection => parent == Some(workspace_id),
        SyncEntityType::ApiFolder | SyncEntityType::ApiRequest => {
            parent.is_some_and(|value| !value.trim().is_empty())
        }
        SyncEntityType::SshTask => parent.is_none(),
        SyncEntityType::SshTaskStep => parent.is_some_and(|value| !value.trim().is_empty()),
    };
    valid.then_some(()).ok_or(SyncError::InvalidData)
}

fn delete_page(entity_type: SyncEntityType, delete: ExternalDelete) -> ExternalApplyPage {
    let mut page = ExternalApplyPage::default();
    match entity_type {
        SyncEntityType::Workspace => page.workspaces.push(ExternalWorkspaceApply::Delete(delete)),
        SyncEntityType::Connection => page
            .connections
            .push(ExternalConnectionApply::Delete(delete)),
        SyncEntityType::WorkspaceVariable => page
            .workspace_variables
            .push(ExternalWorkspaceVariableApply::Delete(delete)),
        SyncEntityType::WorkspaceEnvironment => page
            .workspace_environments
            .push(ExternalWorkspaceEnvironmentApply::Delete(delete)),
        SyncEntityType::WorkspaceEnvironmentVariable => page
            .workspace_environment_variables
            .push(ExternalWorkspaceEnvironmentVariableApply::Delete(delete)),
        SyncEntityType::ApiCollection => page
            .api_collections
            .push(ExternalApiCollectionApply::Delete(delete)),
        SyncEntityType::ApiFolder => page
            .api_folders
            .push(ExternalApiFolderApply::Delete(delete)),
        SyncEntityType::ApiRequest => page
            .api_requests
            .push(ExternalApiRequestApply::Delete(delete)),
        SyncEntityType::SshTask => page.ssh_tasks.push(ExternalSshTaskApply::Delete(delete)),
        SyncEntityType::SshTaskStep => page
            .ssh_task_steps
            .push(ExternalSshTaskStepApply::Delete(delete)),
    }
    page
}

#[cfg(test)]
#[path = "canonical/tests.rs"]
mod tests;
