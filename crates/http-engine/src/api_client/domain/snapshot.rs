use sqlx::SqliteConnection;
use unfour_core::domain::{
    ApiCollectionSnapshot, ApiFolderSnapshot, ApiRequestSnapshot, DomainEntityKey,
    DomainEntityType, DomainSnapshot, TombstoneSnapshot,
};
use unfour_core::{AppError, AppResult};

use super::secrets::{snapshot_auth_json, snapshot_body, snapshot_key_values, snapshot_url};
use super::{collection_on, effective_parent, folder_on, request_on, ApiClientService};

impl ApiClientService {
    pub async fn read_domain_snapshot(&self, key: &DomainEntityKey) -> AppResult<DomainSnapshot> {
        let mut connection = self.db.pool().acquire().await?;
        self.read_domain_snapshot_on(&mut connection, key).await
    }

    pub async fn read_domain_snapshot_on(
        &self,
        connection: &mut SqliteConnection,
        key: &DomainEntityKey,
    ) -> AppResult<DomainSnapshot> {
        match key.entity_type {
            DomainEntityType::ApiCollection => {
                let collection =
                    collection_on(connection, &key.workspace_id, &key.entity_id, true).await?;
                if let Some(deleted_at) = collection.deleted_at {
                    return Ok(tombstone(key.clone(), deleted_at, collection.revision));
                }
                Ok(DomainSnapshot::ApiCollection(ApiCollectionSnapshot {
                    id: collection.id,
                    workspace_id: collection.workspace_id,
                    name: collection.name,
                    description: collection.description,
                    created_at: collection.created_at,
                    updated_at: collection.updated_at,
                    revision: collection.revision,
                }))
            }
            DomainEntityType::ApiFolder => {
                let folder = folder_on(connection, &key.workspace_id, &key.entity_id, true).await?;
                let parent =
                    effective_parent(&folder.collection_id, folder.parent_folder_id.as_deref());
                if let Some(deleted_at) = folder.deleted_at {
                    let mut key = key.clone();
                    key.parent_entity_id = Some(parent.to_string());
                    return Ok(tombstone(key, deleted_at, folder.revision));
                }
                Ok(DomainSnapshot::ApiFolder(ApiFolderSnapshot {
                    id: folder.id,
                    workspace_id: folder.workspace_id,
                    collection_id: folder.collection_id,
                    parent_folder_id: folder.parent_folder_id,
                    name: folder.name,
                    sort_order: folder.sort_order,
                    created_at: folder.created_at,
                    updated_at: folder.updated_at,
                    revision: folder.revision,
                }))
            }
            DomainEntityType::ApiRequest => {
                let request =
                    request_on(connection, &key.workspace_id, &key.entity_id, true).await?;
                let parent =
                    effective_parent(&request.collection_id, request.parent_folder_id.as_deref());
                if let Some(deleted_at) = request.deleted_at {
                    let mut key = key.clone();
                    key.parent_entity_id = Some(parent.to_string());
                    return Ok(tombstone(key, deleted_at, request.revision));
                }
                Ok(DomainSnapshot::ApiRequest(ApiRequestSnapshot {
                    id: request.id,
                    workspace_id: request.workspace_id,
                    collection_id: request.collection_id,
                    parent_folder_id: request.parent_folder_id,
                    name: request.name,
                    sort_order: request.sort_order,
                    auth_json: snapshot_auth_json(&request.auth_json),
                    method: request.method,
                    url: snapshot_url(&request.url),
                    headers: snapshot_key_values(&request.headers_json)?,
                    query: snapshot_key_values(&request.query_json)?,
                    body: snapshot_body(request.body.as_deref(), &request.body_kind),
                    body_kind: request.body_kind,
                    pre_request_script: request.pre_request_script,
                    post_response_script: request.post_response_script,
                    script_schema_version: request.script_schema_version,
                    created_at: request.created_at,
                    updated_at: request.updated_at,
                    revision: request.revision,
                }))
            }
            _ => Err(AppError::Validation(
                "API snapshot requires an API domain entity type".to_string(),
            )),
        }
    }
}

fn tombstone(key: DomainEntityKey, deleted_at: String, revision: i64) -> DomainSnapshot {
    DomainSnapshot::Tombstone(TombstoneSnapshot {
        entity: key,
        deleted_at,
        revision,
    })
}
