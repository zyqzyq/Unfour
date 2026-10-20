use super::*;

#[tokio::test]
async fn flow_schema_upgrade_preserves_pending_attempts_and_conflict_fences() {
    let pool = test_pool().await;
    unfour_local_storage::LocalDb::from_pool(pool.clone())
        .migrate()
        .await
        .unwrap();
    let migrator = cloud_sync_migrator();
    for migration in migrator
        .iter()
        .filter(|migration| migration.version < 20261020010000)
    {
        sqlx::raw_sql(&migration.sql).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql(r#"
        INSERT INTO workspaces(id,name,is_default,environment_type,mcp_policy,created_at,updated_at,revision)
        VALUES ('ws','Workspace',0,'dev','auto','now','now',1);
        INSERT INTO cloud_sync_workspace_bindings(account_id,local_workspace_id,cloud_workspace_id,sync_enabled,state,created_at,updated_at)
        VALUES ('account','ws','cloud',1,'active','now','now');
        INSERT INTO cloud_sync_outbox(account_id,local_workspace_id,cloud_workspace_id,entity_type,entity_id,operation_id,operation,base_version,payload_schema_version,canonical_payload_json,content_revision,status,attempt_count,next_attempt_at,lease_owner,lease_started_at,lease_expires_at,last_error,created_at,updated_at)
        VALUES ('account','ws','cloud','workspaceVariable','var','operation','upsert',7,1,'{"key":"A","value":"B"}',9,'uncertain',3,'later','worker','start','expires','retry','created','updated');
        INSERT INTO cloud_sync_entity_state(account_id,cloud_workspace_id,entity_type,entity_id,server_version,last_operation_id,sync_status,conflict_remote_payload_json,conflict_remote_operation,conflict_parent_entity_id,conflict_operation_id,conflict_payload_schema_version,applied_reader_revision,updated_at)
        VALUES ('account','cloud','workspaceVariable','var',12,'last-op','conflict','{"key":"A","value":"C"}','upsert','ws','remote-op',2,3,'updated');
    "#).execute(&pool).await.unwrap();
    let before = rows(&pool).await;
    sqlx::raw_sql(include_str!(
        "../../migrations/20261020010000_core_cloud_sync_flow_definition.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(rows(&pool).await, before);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
    for table in ["cloud_sync_outbox", "cloud_sync_entity_state"] {
        sqlx::query(&format!("UPDATE {table} SET entity_type='flow'"))
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            sqlx::query(&format!("UPDATE {table} SET entity_type='notRegistered'"))
                .execute(&pool)
                .await
                .is_err()
        );
    }
    assert!(sqlx::query(
        "UPDATE cloud_sync_outbox SET canonical_payload_json=NULL, status='pending'"
    )
    .execute(&pool)
    .await
    .is_err());
}

async fn rows(pool: &SqlitePool) -> (String, String) {
    // JSON arrays retain NULLs, all lease fields, immutable intent and fences.
    let outbox = sqlx::query_scalar("SELECT json_array(account_id,local_workspace_id,cloud_workspace_id,entity_type,entity_id,operation_id,parent_entity_id,operation,base_version,payload_schema_version,canonical_payload_json,deleted_at,content_revision,status,attempt_count,next_attempt_at,lease_owner,lease_started_at,lease_expires_at,last_error,created_at,updated_at) FROM cloud_sync_outbox").fetch_one(pool).await.unwrap();
    let state = sqlx::query_scalar("SELECT json_array(account_id,cloud_workspace_id,entity_type,entity_id,server_version,last_operation_id,sync_status,conflict_remote_payload_json,conflict_remote_operation,conflict_parent_entity_id,conflict_deleted_at,conflict_operation_id,updated_at,conflict_payload_schema_version,applied_reader_revision) FROM cloud_sync_entity_state").fetch_one(pool).await.unwrap();
    (outbox, state)
}
