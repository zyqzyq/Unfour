use serde::{Deserialize, Serialize};

use crate::{AppError, AppResult};

use super::{
    CommandContext, DomainEntityKey, DomainEntityType, DomainMutation, ExternalDelete,
    MutationOperation,
};

pub const SSH_CONNECTION_TYPE: &str = "ssh";
pub const DATABASE_CONNECTION_TYPE: &str = "database";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConnectionSnapshotConfig {
    Ssh {
        username: String,
        auth_method: String,
    },
    Database {
        driver: String,
        database_name: Option<String>,
        username: Option<String>,
        ssl_mode: Option<String>,
        read_only: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSnapshot {
    pub id: String,
    pub workspace_id: String,
    pub connection_type: String,
    pub name: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub config: ConnectionSnapshotConfig,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalConnectionUpsert {
    pub id: String,
    pub workspace_id: String,
    pub connection_type: String,
    pub name: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub config: ConnectionSnapshotConfig,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "record", rename_all = "camelCase")]
pub enum ExternalConnectionApply {
    Upsert(ExternalConnectionUpsert),
    Delete(ExternalDelete),
}

pub fn connection_entity_key(
    workspace_id: impl Into<String>,
    connection_id: impl Into<String>,
) -> DomainEntityKey {
    DomainEntityKey::new(DomainEntityType::Connection, workspace_id, connection_id)
}

pub fn connection_mutation(
    context: &CommandContext,
    operation: MutationOperation,
    workspace_id: &str,
    connection_id: &str,
    revision: i64,
) -> DomainMutation {
    DomainMutation::new(
        context.origin,
        operation,
        connection_entity_key(workspace_id, connection_id),
        revision,
    )
}

pub fn validate_connection_domain_key(key: &DomainEntityKey) -> AppResult<()> {
    if key.entity_type != DomainEntityType::Connection
        || key.parent_entity_id.is_some()
        || key.workspace_id.trim().is_empty()
        || key.entity_id.trim().is_empty()
    {
        return Err(AppError::Validation(
            "connection domain key must use entity type Connection without a parent and include ids"
                .to_string(),
        ));
    }
    Ok(())
}

pub fn validate_external_connection_upsert(record: &ExternalConnectionUpsert) -> AppResult<()> {
    if [
        record.id.as_str(),
        record.workspace_id.as_str(),
        record.connection_type.as_str(),
        record.created_at.as_str(),
        record.updated_at.as_str(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err(AppError::Validation(
            "external connection upsert requires ids, type, and timestamps".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_external_connection_delete(delete: &ExternalDelete) -> AppResult<()> {
    validate_connection_domain_key(&delete.entity)?;
    if delete.deleted_at.trim().is_empty() {
        return Err(AppError::Validation(
            "external connection delete requires deleted_at".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_mutations_always_use_the_aggregate_entity() {
        let mutation = connection_mutation(
            &CommandContext::local("connection.test"),
            MutationOperation::Upsert,
            "workspace-one",
            "connection-one",
            2,
        );

        assert_eq!(mutation.entity.entity_type, DomainEntityType::Connection);
        assert_eq!(mutation.entity.workspace_id, "workspace-one");
        assert_eq!(mutation.entity.entity_id, "connection-one");
        assert!(mutation.entity.parent_entity_id.is_none());
        assert_eq!(mutation.revision, 2);
    }

    #[test]
    fn connection_delete_validation_rejects_subtype_shaped_keys() {
        let delete = ExternalDelete {
            entity: connection_entity_key("workspace-one", "connection-one")
                .with_parent_entity_id("subtype"),
            deleted_at: "2026-08-21T00:00:00Z".to_string(),
        };

        assert!(validate_external_connection_delete(&delete).is_err());
    }
}
