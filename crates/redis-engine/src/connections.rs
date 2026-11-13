//! Workspace-scoped Redis connection definitions. Credential material belongs
//! to the OS store; only the CommandBus can attach a newly staged reference.
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection};
use unfour_core::domain::{
    connection_mutation, CommandContext, DomainCommandResult, MutationOperation,
};
use unfour_core::{AppError, AppResult};

use crate::{RedisEndpoint, RedisTransport};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RedisConnection {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub db_index: u32,
    pub username: Option<String>,
    pub tls: bool,
    pub ssh_connection_id: Option<String>,
    pub ssh_connection_name: Option<String>,
    pub route_needs_review: bool,
    pub allow_writes: bool,
    pub has_password: bool,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RedisConnectionInput {
    pub id: Option<String>,
    pub workspace_id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub db_index: u32,
    pub username: Option<String>,
    pub tls: bool,
    /// Omitted by older callers; explicit null requests a direct connection.
    #[serde(
        default,
        deserialize_with = "deserialize_route",
        skip_serializing_if = "Option::is_none"
    )]
    pub ssh_connection_id: Option<Option<String>>,
    /// Revision displayed by the editor for the selected jump. Never persisted.
    pub expected_ssh_revision: Option<i64>,
    /// Missing on older clients. Preserve local access only for the same target.
    pub allow_writes: Option<bool>,
    pub expected_revision: Option<i64>,
}

fn deserialize_route<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

pub fn validate_connection(
    name: &str,
    host: &str,
    port: u16,
    username: Option<&str>,
) -> AppResult<()> {
    if name.trim().is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(AppError::Validation("REDIS_INVALID_NAME".into()));
    }
    if host.is_empty()
        || host.len() > 253
        || port == 0
        || host.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\' | '@' | '?' | '#')
        })
        || (host.contains(':') && host.parse::<std::net::Ipv6Addr>().is_err())
    {
        return Err(AppError::Validation("REDIS_INVALID_ENDPOINT".into()));
    }
    if username.is_some_and(|u| u.is_empty() || u.len() > 256 || u.chars().any(char::is_control)) {
        return Err(AppError::Validation("REDIS_INVALID_USERNAME".into()));
    }
    Ok(())
}

impl RedisConnectionInput {
    pub fn effective_ssh_connection_id<'a>(
        &'a self,
        saved: Option<&'a RedisConnection>,
    ) -> Option<&'a str> {
        match &self.ssh_connection_id {
            Some(route) => route.as_deref(),
            None => saved.and_then(|record| record.ssh_connection_id.as_deref()),
        }
    }

    pub fn endpoint(&self) -> AppResult<RedisEndpoint> {
        validate_connection(&self.name, &self.host, self.port, self.username.as_deref())?;
        Ok(RedisEndpoint {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            db_index: self.db_index,
            tls: self.tls,
            transport: transport_for(self.effective_ssh_connection_id(None)),
        })
    }

    pub fn check_existing(
        &self,
        current: &RedisConnection,
        secret_supplied: bool,
    ) -> AppResult<()> {
        if self.expected_revision != Some(current.revision) {
            return Err(AppError::RowConflict("REDIS_CONNECTION_CHANGED".into()));
        }
        if current.route_needs_review && self.ssh_connection_id.is_none() {
            return Err(AppError::Validation("SSH_TUNNEL_ROUTE_CHANGED".into()));
        }
        if current.has_password
            && !secret_supplied
            && (current.host != self.host
                || current.port != self.port
                || current.tls != self.tls
                || current.username != self.username
                || current.ssh_connection_id.as_deref()
                    != self.effective_ssh_connection_id(Some(current))
                || current.route_needs_review)
        {
            return Err(AppError::Validation(
                "REDIS_ENDPOINT_REQUIRES_PASSWORD".into(),
            ));
        }
        Ok(())
    }
}

const SELECT_CONNECTION: &str = "SELECT c.id,c.workspace_id,c.name,c.host,c.port,
    r.db_index,r.username,r.tls,r.ssh_connection_id,jump.name AS ssh_connection_name,r.route_review_generation>0 AS route_needs_review,
    r.allow_writes,c.credential_ref IS NOT NULL AS has_password,c.revision,
    c.credential_ref FROM connections c
    JOIN redis_connections r ON r.connection_id=c.id AND r.workspace_id=c.workspace_id
    JOIN workspaces w ON w.id=c.workspace_id AND w.deleted_at IS NULL
    LEFT JOIN connections jump ON jump.workspace_id=c.workspace_id AND jump.id=r.ssh_connection_id
      AND jump.connection_type='ssh' AND jump.deleted_at IS NULL
    WHERE c.workspace_id=? AND c.connection_type='redis' AND c.deleted_at IS NULL";

pub async fn ensure_workspace_on(db: &mut SqliteConnection, workspace: &str) -> AppResult<()> {
    if !sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=? AND deleted_at IS NULL)",
    )
    .bind(workspace)
    .fetch_one(db)
    .await?
    {
        return Err(AppError::NotFound("workspace".into()));
    }
    Ok(())
}

pub async fn list_on(
    db: &mut SqliteConnection,
    workspace: &str,
) -> AppResult<Vec<RedisConnection>> {
    let query = format!("{SELECT_CONNECTION} ORDER BY c.name COLLATE NOCASE,c.id");
    Ok(sqlx::query_as(&query).bind(workspace).fetch_all(db).await?)
}

pub async fn get_on(
    db: &mut SqliteConnection,
    workspace: &str,
    id: &str,
) -> AppResult<(RedisConnection, Option<String>)> {
    use sqlx::Row;
    let query = format!("{SELECT_CONNECTION} AND c.id=?");
    let row = sqlx::query(&query)
        .bind(workspace)
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Redis connection".into()))?;
    Ok((
        RedisConnection::from_row(&row)?,
        row.try_get("credential_ref")?,
    ))
}

impl RedisConnection {
    pub fn endpoint(&self) -> RedisEndpoint {
        RedisEndpoint {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            tls: self.tls,
            transport: if self.route_needs_review {
                RedisTransport::NeedsReview
            } else {
                transport_for(self.ssh_connection_id.as_deref())
            },
            db_index: self.db_index,
        }
    }
}

fn transport_for(route: Option<&str>) -> RedisTransport {
    if route.is_some() {
        RedisTransport::RequiresTunnel
    } else {
        RedisTransport::Direct
    }
}

pub async fn validate_jump(
    db: &mut SqliteConnection,
    workspace: &str,
    id: Option<&str>,
    expected_revision: Option<i64>,
) -> AppResult<()> {
    if let Some(id) = id {
        let revision: i64 = sqlx::query_scalar(
            "SELECT revision FROM connections
            WHERE workspace_id=? AND id=? AND connection_type='ssh' AND deleted_at IS NULL",
        )
        .bind(workspace)
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::Validation("SSH_TUNNEL_ROUTE_MISSING".into()))?;
        if expected_revision.is_some_and(|expected| expected != revision) {
            return Err(AppError::Validation("SSH_TUNNEL_ROUTE_CHANGED".into()));
        }
    } else if expected_revision.is_some() {
        return Err(AppError::Validation("SSH_TUNNEL_ROUTE_CHANGED".into()));
    }
    Ok(())
}

/// `credential` is None to retain, Some(None) to clear, or Some(Some(ref))
/// to replace with a newly staged reference. Re-read and compare at commit.
pub async fn save_on(
    db: &mut SqliteConnection,
    ctx: &CommandContext,
    input: RedisConnectionInput,
    credential: Option<Option<String>>,
) -> AppResult<DomainCommandResult<RedisConnection>> {
    input.endpoint()?;
    ensure_workspace_on(db, &input.workspace_id).await?;
    let now = Utc::now().to_rfc3339();
    let name = input.name.trim();
    let (id, shared_changed, allow_writes, route) = if let Some(id) = input.id.as_deref() {
        let (current, old_ref) = get_on(db, &input.workspace_id, id).await?;
        input.check_existing(&current, credential.is_some())?;
        let route = input
            .effective_ssh_connection_id(Some(&current))
            .map(str::to_owned);
        validate_jump(
            db,
            &input.workspace_id,
            route.as_deref(),
            input.expected_ssh_revision,
        )
        .await?;
        let target_changed = current.host != input.host
            || current.port != input.port
            || current.tls != input.tls
            || current.db_index != input.db_index
            || current.username != input.username
            || current.ssh_connection_id != route;
        let shared_changed = target_changed || current.name != name;
        let allow_writes = input
            .allow_writes
            .unwrap_or(current.allow_writes && !target_changed && !current.route_needs_review);
        let next_ref = credential.unwrap_or_else(|| old_ref.clone());
        if !shared_changed
            && old_ref == next_ref
            && current.allow_writes == allow_writes
            && !current.route_needs_review
        {
            return Ok(DomainCommandResult::unchanged(current));
        }
        sqlx::query("UPDATE connections SET name=?1,host=?2,port=?3,credential_ref=?4,revision=revision+1,
            updated_at=CASE WHEN ?5 THEN ?6 ELSE updated_at END,
            sync_status=CASE WHEN ?5 THEN 'pending' ELSE sync_status END WHERE workspace_id=?7 AND id=?8")
            .bind(name).bind(&input.host).bind(input.port).bind(next_ref).bind(shared_changed)
            .bind(&now).bind(&input.workspace_id).bind(id).execute(&mut *db).await?;
        (id.to_string(), shared_changed, allow_writes, route)
    } else {
        if input.expected_revision.is_some() {
            return Err(AppError::RowConflict("REDIS_CONNECTION_CHANGED".into()));
        }
        let id = unfour_core::id::new_id();
        let route = input.effective_ssh_connection_id(None).map(str::to_owned);
        validate_jump(
            db,
            &input.workspace_id,
            route.as_deref(),
            input.expected_ssh_revision,
        )
        .await?;
        sqlx::query("INSERT INTO connections (id,workspace_id,connection_type,name,host,port,credential_ref,created_at,updated_at)
            VALUES (?,?,'redis',?,?,?,?,?,?)")
            .bind(&id).bind(&input.workspace_id).bind(name).bind(&input.host).bind(input.port)
            .bind(credential.flatten()).bind(&now).bind(&now).execute(&mut *db).await?;
        (id, true, input.allow_writes.unwrap_or(false), route)
    };
    sqlx::query("INSERT INTO redis_connections (workspace_id,connection_id,username,db_index,tls,allow_writes,ssh_connection_id) VALUES (?,?,?,?,?,?,?)
        ON CONFLICT(connection_id) DO UPDATE SET username=excluded.username,db_index=excluded.db_index,tls=excluded.tls,allow_writes=excluded.allow_writes,ssh_connection_id=excluded.ssh_connection_id,route_review_generation=0")
        .bind(&input.workspace_id).bind(&id).bind(&input.username).bind(input.db_index).bind(input.tls).bind(allow_writes).bind(route)
        .execute(&mut *db).await?;
    let (saved, _) = get_on(db, &input.workspace_id, &id).await?;
    let mutations = if shared_changed {
        vec![connection_mutation(
            ctx,
            MutationOperation::Upsert,
            &input.workspace_id,
            &id,
            saved.revision,
        )]
    } else {
        Vec::new()
    };
    Ok(DomainCommandResult::new(saved, mutations))
}

pub async fn delete_on(
    db: &mut SqliteConnection,
    ctx: &CommandContext,
    workspace: &str,
    id: &str,
    revision: i64,
) -> AppResult<DomainCommandResult<()>> {
    let (current, _) = get_on(db, workspace, id).await?;
    if current.revision != revision {
        return Err(AppError::RowConflict("REDIS_CONNECTION_CHANGED".into()));
    }
    let now = Utc::now().to_rfc3339();
    let revision = sqlx::query_scalar(
        "UPDATE connections SET deleted_at=?1,updated_at=?1,credential_ref=NULL,
        revision=revision+1,sync_status='deleted' WHERE workspace_id=?2 AND id=?3 RETURNING revision",
    )
    .bind(now)
    .bind(workspace)
    .bind(id)
    .fetch_one(db)
    .await?;
    Ok(DomainCommandResult::new(
        (),
        vec![connection_mutation(
            ctx,
            MutationOperation::Delete,
            workspace,
            id,
            revision,
        )],
    ))
}
