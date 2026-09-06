mod collections;
mod delete;
mod folders;
mod helpers;
mod requests;

use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityKey, DomainEntityType,
    ExternalApiCollectionApply, ExternalApiCollectionUpsert, ExternalApiFolderApply,
    ExternalApiFolderUpsert, ExternalApiRequestApply, ExternalApiRequestUpsert, ExternalApplyPage,
    ExternalApplyReport, ExternalDelete, MutationOperation, MutationOrigin,
};
use unfour_core::{AppError, AppResult};

use self::collections::upsert_collection;
use self::delete::{apply_collection_delete, apply_folder_delete, apply_request_delete};
use self::folders::apply_folder_upserts;
use self::requests::upsert_request;
use super::{effective_parent, mutation, ApiClientService};

impl ApiClientService {
    pub async fn apply_external_page_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        page: ExternalApplyPage,
    ) -> AppResult<DomainCommandResult<ExternalApplyReport>> {
        if context.origin != MutationOrigin::External {
            return Err(AppError::Config(
                "external apply requires an External command context".to_string(),
            ));
        }
        let (collection_upserts, collection_deletes) = split_collections(page.api_collections);
        let (folder_upserts, folder_deletes) = split_folders(page.api_folders);
        let (request_upserts, request_deletes) = split_requests(page.api_requests);
        let mut mutations = Vec::new();
        let mut materialized_entities = Vec::new();

        for record in collection_upserts {
            let workspace_id = record.workspace_id.clone();
            let id = record.id.clone();
            upsert_collection(connection, record.clone()).await?.record(
                &mut mutations,
                &mut materialized_entities,
                DomainEntityKey::new(DomainEntityType::ApiCollection, &workspace_id, &id),
                |revision| {
                    mutation(
                        context,
                        DomainEntityType::ApiCollection,
                        MutationOperation::Upsert,
                        &workspace_id,
                        &id,
                        None,
                        revision,
                    )
                },
            );
        }
        apply_folder_upserts(
            connection,
            context,
            folder_upserts,
            &mut mutations,
            &mut materialized_entities,
        )
        .await?;
        for record in request_upserts {
            let workspace_id = record.workspace_id.clone();
            let id = record.id.clone();
            let parent =
                effective_parent(&record.collection_id, record.parent_folder_id.as_deref())
                    .to_string();
            upsert_request(connection, record).await?.record(
                &mut mutations,
                &mut materialized_entities,
                DomainEntityKey::new(DomainEntityType::ApiRequest, &workspace_id, &id)
                    .with_parent_entity_id(&parent),
                |revision| {
                    mutation(
                        context,
                        DomainEntityType::ApiRequest,
                        MutationOperation::Upsert,
                        &workspace_id,
                        &id,
                        Some(&parent),
                        revision,
                    )
                },
            );
        }

        for delete in request_deletes {
            apply_request_delete(
                connection,
                context,
                delete,
                &mut mutations,
                &mut materialized_entities,
            )
            .await?;
        }
        for delete in folder_deletes {
            apply_folder_delete(
                connection,
                context,
                delete,
                &mut mutations,
                &mut materialized_entities,
            )
            .await?;
        }
        for delete in collection_deletes {
            apply_collection_delete(
                connection,
                context,
                delete,
                &mut mutations,
                &mut materialized_entities,
            )
            .await?;
        }

        let report = ExternalApplyReport {
            applied_count: mutations.len(),
            mutations: mutations.clone(),
            secret_material_outcomes: Vec::new(),
            materialized_entities,
        };
        Ok(DomainCommandResult::new(report, mutations))
    }
}

fn split_collections(
    changes: Vec<ExternalApiCollectionApply>,
) -> (Vec<ExternalApiCollectionUpsert>, Vec<ExternalDelete>) {
    let mut upserts = Vec::new();
    let mut deletes = Vec::new();
    for change in changes {
        match change {
            ExternalApiCollectionApply::Upsert(record) => upserts.push(record),
            ExternalApiCollectionApply::Delete(delete) => deletes.push(delete),
        }
    }
    (upserts, deletes)
}

fn split_folders(
    changes: Vec<ExternalApiFolderApply>,
) -> (Vec<ExternalApiFolderUpsert>, Vec<ExternalDelete>) {
    let mut upserts = Vec::new();
    let mut deletes = Vec::new();
    for change in changes {
        match change {
            ExternalApiFolderApply::Upsert(record) => upserts.push(record),
            ExternalApiFolderApply::Delete(delete) => deletes.push(delete),
        }
    }
    (upserts, deletes)
}

fn split_requests(
    changes: Vec<ExternalApiRequestApply>,
) -> (Vec<ExternalApiRequestUpsert>, Vec<ExternalDelete>) {
    let mut upserts = Vec::new();
    let mut deletes = Vec::new();
    for change in changes {
        match change {
            ExternalApiRequestApply::Upsert(record) => upserts.push(*record),
            ExternalApiRequestApply::Delete(delete) => deletes.push(delete),
        }
    }
    (upserts, deletes)
}
