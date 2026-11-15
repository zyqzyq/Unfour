//! Collection and folder authoring, ordering, and checked subtree deletion.

use crate::{transaction::CommandActivity, CommandBus};
use unfour_core::domain::CommandContext;
use unfour_core::models::{
    ApiCollection, ApiCollectionExportArtifact, ApiCollectionExportFormat, ApiCollectionFolder,
    ApiCollectionImportResult,
};
use unfour_core::AppResult;

impl CommandBus {
    pub async fn api_collection_list(&self, workspace_id: String) -> AppResult<Vec<ApiCollection>> {
        self.api_client.list_collections(workspace_id).await
    }

    pub async fn api_collection_export(
        &self,
        workspace_id: String,
        collection_id: String,
        format: ApiCollectionExportFormat,
    ) -> AppResult<ApiCollectionExportArtifact> {
        let environments = self
            .workspace_environments_list(workspace_id.clone())
            .await?;
        self.api_client
            .export_collection_openapi(workspace_id, collection_id, format, environments)
            .await
    }

    pub async fn api_collection_import_preview(
        &self,
        workspace_id: String,
        content: &str,
    ) -> AppResult<unfour_core::models::CollectionImportPreview> {
        self.api_client
            .preview_collection_import(workspace_id, content)
            .await
    }

    pub async fn api_collection_import(
        &self,
        workspace_id: String,
        content: String,
    ) -> AppResult<ApiCollectionImportResult> {
        let context = CommandContext::local("api.collection.import");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let content_bytes = content.len();
        self.execute_domain_command_with_activity(
            context,
            move |result: &ApiCollectionImportResult| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.import",
                target: result
                    .collection
                    .as_ref()
                    .map(|collection| collection.id.clone()),
                details: serde_json::json!({
                    "contentBytes": content_bytes,
                    "folderCount": result.folder_count,
                    "requestCount": result.request_count,
                }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .import_collection_on(connection, &executor_context, workspace_id, content)
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_create(
        &self,
        workspace_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        let context = CommandContext::local("api.collection.create");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |collection: &ApiCollection| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.create",
                target: Some(collection.id.clone()),
                details: serde_json::json!({ "name": collection.name }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .create_collection_on(connection, &executor_context, workspace_id, name)
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_rename(
        &self,
        workspace_id: String,
        collection_id: String,
        name: String,
    ) -> AppResult<ApiCollection> {
        self.api_collection_rename_if_current(workspace_id, collection_id, name, None)
            .await
    }

    pub async fn api_collection_rename_if_current(
        &self,
        workspace_id: String,
        collection_id: String,
        name: String,
        expected_updated_at: Option<String>,
    ) -> AppResult<ApiCollection> {
        let context = CommandContext::local("api.collection.rename");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |collection: &ApiCollection| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.rename",
                target: Some(collection.id.clone()),
                details: serde_json::json!({ "name": collection.name }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .rename_collection_if_current_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            collection_id,
                            name,
                            expected_updated_at,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_delete(
        &self,
        workspace_id: String,
        collection_id: String,
    ) -> AppResult<Vec<ApiCollection>> {
        let context = CommandContext::local("api.collection.delete");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_collection_id = collection_id.clone();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.delete",
                target: Some(activity_collection_id),
                details: serde_json::json!({ "softDelete": true, "cascade": true }),
            }),
            move |connection| {
                Box::pin(async move {
                    service
                        .delete_collection_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            collection_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_folders_list(
        &self,
        workspace_id: String,
        collection_id: Option<String>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        self.api_client
            .list_collection_folders(workspace_id, collection_id)
            .await
    }

    pub async fn api_collection_folder_create(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        name: String,
    ) -> AppResult<ApiCollectionFolder> {
        let context = CommandContext::local("api.collection.folder.create");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |folder: &ApiCollectionFolder| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.create",
                target: Some(folder.id.clone()),
                details: serde_json::json!({
                    "collectionId": folder.collection_id,
                    "parentFolderId": folder.parent_folder_id,
                }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .create_collection_folder_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            collection_id,
                            parent_folder_id,
                            name,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_folder_rename(
        &self,
        workspace_id: String,
        folder_id: String,
        name: String,
    ) -> AppResult<ApiCollectionFolder> {
        self.api_collection_folder_rename_if_current(workspace_id, folder_id, name, None)
            .await
    }

    pub async fn api_collection_folder_rename_if_current(
        &self,
        workspace_id: String,
        folder_id: String,
        name: String,
        expected_updated_at: Option<String>,
    ) -> AppResult<ApiCollectionFolder> {
        let context = CommandContext::local("api.collection.folder.rename");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |folder: &ApiCollectionFolder| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.rename",
                target: Some(folder.id.clone()),
                details: serde_json::json!({ "name": folder.name }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .rename_collection_folder_if_current_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            folder_id,
                            name,
                            expected_updated_at,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_folder_delete(
        &self,
        workspace_id: String,
        folder_id: String,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        self.api_collection_folder_delete_checked(workspace_id, folder_id, None)
            .await
    }

    pub async fn api_folder_delete_snapshot(
        &self,
        workspace_id: String,
        folder_id: String,
    ) -> AppResult<unfour_core::models::ApiFolderDeleteSnapshot> {
        self.api_client
            .folder_delete_snapshot(&workspace_id, &folder_id)
            .await
    }

    pub async fn api_collection_folder_delete_checked(
        &self,
        workspace_id: String,
        folder_id: String,
        expected: Option<unfour_core::models::ApiFolderDeleteSnapshot>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        let context = CommandContext::local("api.collection.folder.delete");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_folder_id = folder_id.clone();
        self.execute_domain_command(
            context,
            Some(CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.delete",
                target: Some(activity_folder_id),
                details: serde_json::json!({ "softDelete": true, "recursive": true }),
            }),
            move |connection| {
                Box::pin(async move {
                    if let Some(expected) = expected {
                        if service
                            .folder_delete_snapshot_on(connection, &workspace_id, &folder_id)
                            .await?
                            != expected
                        {
                            return Err(unfour_core::AppError::RowConflict(
                                "API folder subtree changed".into(),
                            ));
                        }
                    }
                    service
                        .delete_collection_folder_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            folder_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_folder_move(
        &self,
        workspace_id: String,
        folder_id: String,
        target_parent_folder_id: Option<String>,
    ) -> AppResult<ApiCollectionFolder> {
        let context = CommandContext::local("api.collection.folder.move");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |folder: &ApiCollectionFolder| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.move",
                target: Some(folder.id.clone()),
                details: serde_json::json!({ "parentFolderId": folder.parent_folder_id }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .move_collection_folder_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            folder_id,
                            target_parent_folder_id,
                        )
                        .await
                })
            },
        )
        .await
    }

    pub async fn api_collection_folders_reorder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        folder_ids: Vec<String>,
    ) -> AppResult<Vec<ApiCollectionFolder>> {
        let context = CommandContext::local("api.collection.folder.reorder");
        let executor_context = context.clone();
        let service = self.api_client.clone();
        let activity_workspace_id = workspace_id.clone();
        let activity_collection_id = collection_id.clone();
        self.execute_domain_command_with_activity(
            context,
            move |folders: &Vec<ApiCollectionFolder>| CommandActivity {
                workspace_id: Some(activity_workspace_id),
                action: "api.collection.folder.reorder",
                target: Some(activity_collection_id),
                details: serde_json::json!({ "folderCount": folders.len() }),
            },
            move |connection| {
                Box::pin(async move {
                    service
                        .reorder_collection_folders_on(
                            connection,
                            &executor_context,
                            workspace_id,
                            collection_id,
                            parent_folder_id,
                            folder_ids,
                        )
                        .await
                })
            },
        )
        .await
    }
}
