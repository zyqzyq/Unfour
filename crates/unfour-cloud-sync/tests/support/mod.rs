use unfour_cloud_sync::{Clock, IdGenerator, SyncEntityAdapters, SyncError, SyncRepository};
use unfour_database_engine::DatabaseService;
use unfour_http_engine::ApiClientService;
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;
use unfour_ssh_engine::SshService;
use unfour_workspace_engine::WorkspaceService;

#[allow(clippy::too_many_arguments)]
pub async fn create_registry_binding(
    repository: &SyncRepository,
    db: &LocalDb,
    account_id: &str,
    generation: u64,
    workspace_id: &str,
    cloud_workspace_id: &str,
    ids: &dyn IdGenerator,
    clock: &dyn Clock,
) -> Result<(), SyncError> {
    let secrets = SecretStore::in_memory("cloud-sync-test-binding");
    let workspace = WorkspaceService::new(db.clone());
    let api_client = ApiClientService::new(db.clone());
    let ssh = SshService::new(db.clone(), secrets.clone());
    let database = DatabaseService::new(db.clone()).with_secret_store(secrets);
    repository
        .create_binding_with_initial_outbox(
            account_id,
            generation,
            workspace_id,
            cloud_workspace_id,
            0,
            &SyncEntityAdapters::new(&workspace, &api_client, &ssh, &database),
            ids,
            clock,
        )
        .await
}
