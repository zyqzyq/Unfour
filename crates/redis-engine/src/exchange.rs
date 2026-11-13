//! Portable connection definitions. These operations share the caller's SQLite
//! transaction and never open a network connection or read credential material.
use sqlx::{FromRow, SqliteConnection};
use unfour_core::domain::{
    connection_entity_key, connection_mutation, validate_connection_domain_key,
    validate_external_connection_delete, validate_external_connection_upsert, CommandContext,
    ConnectionSnapshot, ConnectionSnapshotConfig, DomainCommandResult, DomainEntityKey,
    DomainMutation, DomainSnapshot, ExternalConnectionApply, ExternalConnectionUpsert,
    MutationOperation, MutationOrigin, TombstoneSnapshot, REDIS_CONNECTION_TYPE,
};
use unfour_core::{AppError, AppResult};

#[derive(FromRow)]
struct Definition {
    name: String,
    host: String,
    port: u16,
    username: Option<String>,
    db_index: u32,
    tls: bool,
    created_at: String,
    updated_at: String,
    revision: i64,
    deleted_at: Option<String>,
}

async fn definition(
    db: &mut SqliteConnection,
    key: &DomainEntityKey,
) -> AppResult<Option<Definition>> {
    Ok(sqlx::query_as(
        "SELECT c.name, c.host, c.port, r.username, r.db_index, r.tls,
                c.created_at, c.updated_at, c.revision, c.deleted_at
         FROM connections c JOIN redis_connections r
           ON r.connection_id=c.id AND r.workspace_id=c.workspace_id
         WHERE c.workspace_id=? AND c.id=? AND c.connection_type='redis'",
    )
    .bind(&key.workspace_id)
    .bind(&key.entity_id)
    .fetch_optional(db)
    .await?)
}

pub async fn snapshot_on(
    db: &mut SqliteConnection,
    key: &DomainEntityKey,
) -> AppResult<DomainSnapshot> {
    validate_connection_domain_key(key)?;
    let row = definition(db, key)
        .await?
        .ok_or_else(|| AppError::NotFound("Redis connection".into()))?;
    if let Some(deleted_at) = row.deleted_at {
        return Ok(DomainSnapshot::Tombstone(TombstoneSnapshot {
            entity: key.clone(),
            deleted_at,
            revision: row.revision,
        }));
    }
    Ok(DomainSnapshot::Connection(ConnectionSnapshot {
        id: key.entity_id.clone(),
        workspace_id: key.workspace_id.clone(),
        connection_type: REDIS_CONNECTION_TYPE.into(),
        name: row.name,
        host: Some(row.host),
        port: Some(row.port),
        created_at: row.created_at,
        updated_at: row.updated_at,
        revision: row.revision,
        config: ConnectionSnapshotConfig::Redis {
            username: row.username,
            db_index: row.db_index,
            tls: row.tls,
        },
    }))
}

pub async fn export_on(
    db: &mut SqliteConnection,
    workspace: &str,
) -> AppResult<Vec<DomainSnapshot>> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM connections WHERE workspace_id=? AND connection_type='redis' AND deleted_at IS NULL ORDER BY id",
    ).bind(workspace).fetch_all(&mut *db).await?;
    let mut records = Vec::with_capacity(ids.len());
    for id in ids {
        records.push(snapshot_on(db, &connection_entity_key(workspace, id)).await?);
    }
    Ok(records)
}

pub async fn credential_refs_on(
    db: &mut SqliteConnection,
    workspace: &str,
) -> AppResult<Vec<(String, String)>> {
    Ok(sqlx::query_as(
        "SELECT id, credential_ref FROM connections WHERE workspace_id=? AND connection_type='redis'
         AND deleted_at IS NULL AND credential_ref IS NOT NULL ORDER BY id",
    ).bind(workspace).fetch_all(db).await?)
}

pub async fn restore_credential_on(
    db: &mut SqliteConnection,
    workspace: &str,
    id: &str,
    reference: &str,
) -> AppResult<()> {
    let count = sqlx::query(
        "UPDATE connections SET credential_ref=? WHERE workspace_id=? AND id=?
        AND connection_type='redis' AND deleted_at IS NULL AND credential_ref IS NULL",
    )
    .bind(reference)
    .bind(workspace)
    .bind(id)
    .execute(db)
    .await?
    .rows_affected();
    if count != 1 {
        return Err(AppError::Validation("WORKSPACE_BUNDLE_INVALID".into()));
    }
    Ok(())
}

async fn tombstone_on(
    db: &mut SqliteConnection,
    context: &CommandContext,
    key: &DomainEntityKey,
    stamp: &str,
) -> AppResult<Option<DomainMutation>> {
    let revision: Option<i64> = sqlx::query_scalar(
        "UPDATE connections SET deleted_at=?, updated_at=?, credential_ref=NULL, revision=revision+1,
         sync_status='local' WHERE workspace_id=? AND id=? AND connection_type='redis' AND deleted_at IS NULL RETURNING revision",
    ).bind(stamp).bind(stamp).bind(&key.workspace_id).bind(&key.entity_id).fetch_optional(db).await?;
    Ok(revision.map(|rev| {
        connection_mutation(
            context,
            MutationOperation::Delete,
            &key.workspace_id,
            &key.entity_id,
            rev,
        )
    }))
}

pub async fn cascade_on(
    db: &mut SqliteConnection,
    context: &CommandContext,
    workspace: &str,
    stamp: &str,
) -> AppResult<(Vec<DomainMutation>, Vec<DomainEntityKey>)> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM connections WHERE workspace_id=? AND connection_type='redis' ORDER BY id",
    )
    .bind(workspace)
    .fetch_all(&mut *db)
    .await?;
    let mut mutations = Vec::new();
    let mut materialized = Vec::with_capacity(ids.len());
    for id in ids {
        let key = connection_entity_key(workspace, id);
        if let Some(mutation) = tombstone_on(db, context, &key, stamp).await? {
            mutations.push(mutation);
        }
        materialized.push(key);
    }
    Ok((mutations, materialized))
}

pub async fn apply_on(
    db: &mut SqliteConnection,
    context: &CommandContext,
    change: ExternalConnectionApply,
) -> AppResult<DomainCommandResult<()>> {
    if context.origin != MutationOrigin::External {
        return Err(AppError::Validation(
            "external Redis connection requires external origin".into(),
        ));
    }
    match change {
        ExternalConnectionApply::Delete(delete) => {
            validate_external_connection_delete(&delete)?;
            let mutation = tombstone_on(db, context, &delete.entity, &delete.deleted_at).await?;
            Ok(DomainCommandResult::new((), mutation.into_iter().collect()))
        }
        ExternalConnectionApply::Upsert(record) => upsert_on(db, context, record).await,
    }
}

async fn upsert_on(
    db: &mut SqliteConnection,
    context: &CommandContext,
    record: ExternalConnectionUpsert,
) -> AppResult<DomainCommandResult<()>> {
    let invalid = || AppError::Validation("REDIS_INVALID_INPUT".into());
    validate_external_connection_upsert(&record)?;
    if record.connection_type != REDIS_CONNECTION_TYPE {
        return Err(invalid());
    }
    let ConnectionSnapshotConfig::Redis {
        username,
        db_index,
        tls,
    } = record.config
    else {
        return Err(invalid());
    };
    let host = record.host.ok_or_else(invalid)?;
    let port = record.port.ok_or_else(invalid)?;
    crate::connections::validate_connection(&record.name, &host, port, username.as_deref())?;
    let live: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=? AND deleted_at IS NULL)",
    )
    .bind(&record.workspace_id)
    .fetch_one(&mut *db)
    .await?;
    if !live {
        return Err(AppError::NotFound("workspace".into()));
    }
    let key = connection_entity_key(&record.workspace_id, &record.id);
    let existing = definition(db, &key).await?;
    if let Some(current) = &existing {
        if current.deleted_at.is_none()
            && current.name == record.name
            && current.host == host
            && current.port == port
            && current.username == username
            && current.db_index == db_index
            && current.tls == tls
            && current.created_at == record.created_at
            && current.updated_at == record.updated_at
        {
            return Ok(DomainCommandResult::unchanged(()));
        }
    }
    // Preserve device credentials only while the authentication target matches.
    let retain_secret = existing.is_some_and(|old| {
        old.deleted_at.is_none()
            && old.host == host
            && old.port == port
            && old.username == username
            && old.tls == tls
    });
    let revision: Option<i64> = sqlx::query_scalar(
        "INSERT INTO connections (id,workspace_id,connection_type,name,host,port,created_at,updated_at)
         VALUES(?,?,'redis',?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,host=excluded.host,
         port=excluded.port,created_at=excluded.created_at,updated_at=excluded.updated_at,deleted_at=NULL,
         revision=connections.revision+1,sync_status='local',credential_ref=CASE WHEN ? THEN connections.credential_ref ELSE NULL END
         WHERE connections.workspace_id=excluded.workspace_id AND connections.connection_type='redis' RETURNING revision",
    ).bind(&record.id).bind(&record.workspace_id).bind(&record.name).bind(host).bind(port)
        .bind(&record.created_at).bind(&record.updated_at).bind(retain_secret).fetch_optional(&mut *db).await?;
    let revision = revision.ok_or_else(invalid)?;
    sqlx::query("INSERT INTO redis_connections (workspace_id,connection_id,username,db_index,tls) VALUES(?,?,?,?,?)
        ON CONFLICT(connection_id) DO UPDATE SET username=excluded.username,db_index=excluded.db_index,tls=excluded.tls")
        .bind(&record.workspace_id).bind(&record.id).bind(username).bind(db_index).bind(tls).execute(db).await?;
    Ok(DomainCommandResult::new(
        (),
        vec![connection_mutation(
            context,
            MutationOperation::Upsert,
            &record.workspace_id,
            &record.id,
            revision,
        )],
    ))
}
