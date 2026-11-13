use base64::{engine::general_purpose::STANDARD, Engine};
use std::{future::Future, pin::Pin, sync::Arc};
use unfour_core::network::{BoxedTunnelStream, TunnelConnector};
use unfour_core::{domain::CommandContext, AppError, AppResult};
use unfour_redis_engine::connections;
use unfour_redis_engine::RedisEndpoint;
use zeroize::Zeroizing;

mod scan_cursor;
use crate::transaction::CommandActivity;
use crate::{CommandBus, RedisConnection, RedisConnectionInput, RedisKeyInfo, RedisScanPage};
use crate::{RedisCollectionKind, RedisCollectionPage, RedisMutation};

/// Redis calls this connector inside its single network deadline. SSH setup,
/// the forwarded channel and every subsequent command share that same budget.
struct RedisJump {
    bus: CommandBus,
    workspace: String,
    jump: String,
    ssh_revision: Option<i64>,
    saved: Option<(String, i64)>,
}

impl TunnelConnector for RedisJump {
    fn connect<'a>(
        &'a self,
        host: &'a str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = AppResult<BoxedTunnelStream>> + Send + 'a>> {
        Box::pin(async move {
            let tunnel = self
                .bus
                .ssh
                .open_tunnel_at_revision(&self.workspace, &self.jump, self.ssh_revision)
                .await
                .map_err(|error| match error {
                    AppError::NotFound(_) => {
                        AppError::Validation("SSH_TUNNEL_ROUTE_MISSING".into())
                    }
                    other => other,
                })?;
            let stream = tunnel.connect(host, port).await?;
            if let Some((id, revision)) = &self.saved {
                self.bus
                    .check_redis_revision(&self.workspace, id, *revision)
                    .await?;
            }
            Ok(stream)
        })
    }
}

impl CommandBus {
    pub async fn list_redis_connections(&self, workspace: &str) -> AppResult<Vec<RedisConnection>> {
        let mut db = self.db.pool().acquire().await?;
        connections::list_on(&mut db, workspace).await
    }

    pub async fn save_redis_connection(
        &self,
        input: RedisConnectionInput,
        secret: Option<String>,
    ) -> AppResult<RedisConnection> {
        input.endpoint()?;
        let secret = secret.map(Zeroizing::new);
        let _guard = self.credential_stage_guard().await?;
        let (_, existing_ref) = self.redis_draft_reference(&input, secret.is_some()).await?;
        self.track_connection_credential_edit(&input.workspace_id, input.id.as_deref(), "redis")
            .await?;
        let credential = match secret.as_deref() {
            None => None,
            Some(s) if s.is_empty() => Some(None),
            Some(s) => Some(Some(
                self.stage_connection_credential(
                    &input.workspace_id,
                    existing_ref.as_deref(),
                    "redis-password",
                    s,
                )
                .await?,
            )),
        };
        let staged: Vec<_> = credential.iter().flatten().cloned().collect();
        let context = CommandContext::local("redis.connection.save");
        let executor_context = context.clone();
        let result = self
            .execute_domain_command_with_activity_even_without_mutation(
                context,
                |saved: &RedisConnection| CommandActivity {
                    workspace_id: Some(saved.workspace_id.clone()),
                    target: Some(saved.id.clone()),
                    action: "redis.connection.save",
                    details: serde_json::json!({}),
                },
                move |db| {
                    Box::pin(async move {
                        connections::save_on(db, &executor_context, input, credential).await
                    })
                },
            )
            .await;
        self.finish_bundle_credential_edits(&staged, result).await
    }

    pub async fn delete_redis_connection(
        &self,
        workspace: String,
        id: String,
        expected_revision: i64,
    ) -> AppResult<Vec<RedisConnection>> {
        let _guard = self.credential_stage_guard().await?;
        self.track_connection_credential_edit(&workspace, Some(&id), "redis")
            .await?;
        let context = CommandContext::local("redis.connection.delete");
        let executor_context = context.clone();
        let activity = CommandActivity {
            workspace_id: Some(workspace.clone()),
            target: Some(id.clone()),
            action: "redis.connection.delete",
            details: serde_json::json!({}),
        };
        self.execute_domain_command(context, Some(activity), move |db| {
            Box::pin(async move {
                let deleted = connections::delete_on(
                    db,
                    &executor_context,
                    &workspace,
                    &id,
                    expected_revision,
                )
                .await?;
                let remaining = connections::list_on(db, &workspace).await?;
                Ok(unfour_core::domain::DomainCommandResult::new(
                    remaining,
                    deleted.mutations,
                ))
            })
        })
        .await
    }

    async fn redis_draft_reference(
        &self,
        input: &RedisConnectionInput,
        supplied: bool,
    ) -> AppResult<(Option<RedisConnection>, Option<String>)> {
        let mut db = self.db.pool().acquire().await?;
        match input.id.as_deref() {
            Some(id) => {
                let (record, reference) =
                    connections::get_on(&mut db, &input.workspace_id, id).await?;
                input.check_existing(&record, supplied)?;
                Ok((Some(record), reference))
            }
            None => {
                connections::ensure_workspace_on(&mut db, &input.workspace_id).await?;
                Ok((None, None))
            }
        }
    }

    async fn read_redis_password(
        &self,
        workspace: &str,
        reference: Option<String>,
    ) -> AppResult<Option<Zeroizing<String>>> {
        match reference {
            Some(reference) => Ok(Some(Zeroizing::new(
                self.secret_store
                    .read_secret(workspace.into(), reference)
                    .await?,
            ))),
            None => Ok(None),
        }
    }

    pub async fn test_redis_connection_input(
        &self,
        input: RedisConnectionInput,
        secret: Option<String>,
    ) -> AppResult<()> {
        let endpoint = input.endpoint()?;
        let (saved, reference) = self.redis_draft_reference(&input, secret.is_some()).await?;
        let route_id = input.effective_ssh_connection_id(saved.as_ref());
        {
            let mut db = self.db.pool().acquire().await?;
            connections::validate_jump(
                &mut db,
                &input.workspace_id,
                route_id,
                input.expected_ssh_revision,
            )
            .await?;
        }
        let password = match secret {
            Some(value) => (!value.is_empty()).then(|| Zeroizing::new(value)),
            None => {
                self.read_redis_password(&input.workspace_id, reference)
                    .await?
            }
        };
        let endpoint = self.redis_endpoint_via(
            &input.workspace_id,
            endpoint,
            route_id,
            input.expected_ssh_revision,
            saved.as_ref(),
        );
        if route_id.is_none() {
            if let Some(record) = &saved {
                self.check_redis_revision(&record.workspace_id, &record.id, record.revision)
                    .await?;
            }
        }
        unfour_redis_engine::test_connection(&endpoint, password.as_deref().map(String::as_str))
            .await
    }

    fn redis_endpoint_via(
        &self,
        workspace: &str,
        endpoint: RedisEndpoint,
        jump: Option<&str>,
        expected_ssh_revision: Option<i64>,
        saved: Option<&RedisConnection>,
    ) -> RedisEndpoint {
        let Some(jump) = jump else {
            return endpoint;
        };
        endpoint.with_tunnel(Arc::new(RedisJump {
            bus: self.clone(),
            workspace: workspace.to_owned(),
            jump: jump.to_owned(),
            ssh_revision: expected_ssh_revision,
            saved: saved.map(|record| (record.id.clone(), record.revision)),
        }))
    }

    async fn check_redis_revision(
        &self,
        workspace: &str,
        id: &str,
        revision: i64,
    ) -> AppResult<()> {
        let mut db = self.db.pool().acquire().await?;
        let (latest, _) = connections::get_on(&mut db, workspace, id).await?;
        if latest.revision != revision {
            return Err(AppError::RowConflict("REDIS_CONNECTION_CHANGED".into()));
        }
        Ok(())
    }

    fn redis_saved_endpoint(&self, record: &RedisConnection) -> RedisEndpoint {
        self.redis_endpoint_via(
            &record.workspace_id,
            record.endpoint(),
            record.ssh_connection_id.as_deref(),
            None,
            Some(record),
        )
    }

    async fn redis_target(
        &self,
        workspace: &str,
        id: &str,
        expected_revision: Option<i64>,
    ) -> AppResult<(RedisConnection, Option<Zeroizing<String>>)> {
        let (record, reference) = {
            let mut db = self.db.pool().acquire().await?;
            connections::get_on(&mut db, workspace, id).await?
        };
        if expected_revision.is_some_and(|revision| revision != record.revision) {
            return Err(AppError::RowConflict("REDIS_CONNECTION_CHANGED".into()));
        }
        Ok((
            record,
            self.read_redis_password(workspace, reference).await?,
        ))
    }

    pub async fn inspect_redis_key(
        &self,
        workspace: &str,
        id: &str,
        key_base64: &str,
        field_base64: Option<&str>,
        expected_revision: Option<i64>,
    ) -> AppResult<RedisKeyInfo> {
        let key = decode_argument(key_base64)?;
        let field = field_base64.map(decode_argument).transpose()?;
        let (record, password) = self.redis_target(workspace, id, expected_revision).await?;
        unfour_redis_engine::inspect_key(
            &self.redis_saved_endpoint(&record),
            password.as_deref().map(String::as_str),
            &key,
            field.as_deref(),
        )
        .await
    }

    pub async fn scan_redis_keys(
        &self,
        workspace: &str,
        id: &str,
        cursor: &str,
        pattern: &str,
        expected_revision: Option<i64>,
    ) -> AppResult<RedisScanPage> {
        if pattern.len() > 4096 || cursor.len() > 40 {
            return Err(AppError::Validation("REDIS_INVALID_INPUT".into()));
        }
        let (record, password) = self.redis_target(workspace, id, expected_revision).await?;
        let scope = scan_cursor::scope(&record, pattern);
        let offset = scan_cursor::unpack(&scope, cursor)?;
        let mut page = unfour_redis_engine::scan_keys(
            &self.redis_saved_endpoint(&record),
            password.as_deref().map(String::as_str),
            offset,
            pattern,
        )
        .await?;
        page.cursor = scan_cursor::pack(&scope, &page.cursor);
        Ok(page)
    }
    pub async fn check_redis_key(
        &self,
        workspace: &str,
        id: &str,
        key: &[u8],
        field: Option<&[u8]>,
        equals: Option<&[u8]>,
    ) -> AppResult<unfour_redis_engine::RedisCheckResult> {
        self.check_redis_key_at_revision(workspace, id, key, field, equals, None)
            .await
    }

    pub(crate) async fn check_redis_key_at_revision(
        &self,
        workspace: &str,
        id: &str,
        key: &[u8],
        field: Option<&[u8]>,
        equals: Option<&[u8]>,
        revision: Option<i64>,
    ) -> AppResult<unfour_redis_engine::RedisCheckResult> {
        let (record, password) = self.redis_target(workspace, id, revision).await?;
        unfour_redis_engine::check_key(
            &self.redis_saved_endpoint(&record),
            password.as_deref().map(String::as_str),
            key,
            field,
            equals,
        )
        .await
    }
}

impl CommandBus {
    #[allow(clippy::too_many_arguments)]
    pub async fn browse_redis_collection(
        &self,
        workspace: &str,
        id: &str,
        key_base64: &str,
        kind: RedisCollectionKind,
        cursor: &str,
        pattern: &str,
        descending: bool,
        expected_revision: i64,
        start_index: Option<u64>,
    ) -> AppResult<RedisCollectionPage> {
        let key = decode_argument(key_base64)?;
        if cursor.len() > 40 || pattern.len() > 4096 {
            return Err(AppError::Validation("REDIS_INVALID_INPUT".into()));
        }
        let (record, password) = self
            .redis_target(workspace, id, Some(expected_revision))
            .await?;
        let scope = scan_cursor::scope(&record, ("collection", &key, kind, pattern, descending));
        let start;
        let offset = match start_index {
            Some(index)
                if cursor == "0"
                    && index <= (1_u64 << 53) - 1
                    && matches!(kind, RedisCollectionKind::List | RedisCollectionKind::Zset) =>
            {
                start = index.to_string();
                start.as_str()
            }
            Some(_) => return Err(AppError::Validation("REDIS_INVALID_INPUT".into())),
            None => scan_cursor::unpack(&scope, cursor)?,
        };
        let mut page = unfour_redis_engine::browse_collection(
            &self.redis_saved_endpoint(&record),
            password.as_deref().map(String::as_str),
            &key,
            kind,
            offset,
            pattern,
            descending,
        )
        .await?;
        page.cursor = scan_cursor::pack(&scope, &page.cursor);
        Ok(page)
    }

    /// An explicit desktop write always pins the local grant and connection revision.
    pub async fn mutate_redis_key(
        &self,
        workspace: &str,
        id: &str,
        key_base64: &str,
        mutation: RedisMutation,
        expected_revision: i64,
        confirmed: bool,
    ) -> AppResult<()> {
        if !confirmed {
            return Err(AppError::Validation(
                "REDIS_WRITE_CONFIRMATION_REQUIRED".into(),
            ));
        }
        let key = decode_argument(key_base64)?;
        let (record, reference) = {
            let mut db = self.db.pool().acquire().await?;
            connections::get_on(&mut db, workspace, id).await?
        };
        if record.revision != expected_revision {
            return Err(AppError::RowConflict("REDIS_CONNECTION_CHANGED".into()));
        }
        if !record.allow_writes {
            return Err(AppError::Validation("REDIS_WRITE_READ_ONLY".into()));
        }
        let has_secret = reference.is_some();
        let password = self.read_redis_password(workspace, reference).await?;
        // OS credential access can block while another window revokes the local grant.
        if has_secret {
            let mut db = self.db.pool().acquire().await?;
            let (latest, _) = connections::get_on(&mut db, workspace, id).await?;
            if latest.revision != expected_revision || !latest.allow_writes {
                return Err(AppError::RowConflict("REDIS_CONNECTION_CHANGED".into()));
            }
        }
        let result = unfour_redis_engine::mutate_key(
            &self.redis_saved_endpoint(&record),
            password.as_deref().map(String::as_str),
            &key,
            &mutation,
        )
        .await;
        // A local logging failure must never turn a remote commit into a retryable failure.
        let outcome = match &result {
            Ok(_) => "succeeded",
            Err(error) if error.code() == "REDIS_WRITE_OUTCOME_UNKNOWN" => "unknown",
            Err(_) => "failed",
        };
        self.record_completion_activity(CommandActivity {
            workspace_id: Some(record.workspace_id), action: "redis.key.mutate", target: Some(record.id),
            details: serde_json::json!({"operation": mutation.operation_name(), "outcome": outcome}),
        }, "redis", "mutate_key").await;
        result
    }
}

fn decode_argument(encoded: &str) -> AppResult<Vec<u8>> {
    // A bounded key/field buffer also bounds the IPC decode allocation.
    const MAX_BYTES: usize = 64 * 1024;
    if encoded.len() > MAX_BYTES.div_ceil(3) * 4 {
        return Err(AppError::Validation("REDIS_KEY_TOO_LARGE".into()));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| AppError::Validation("REDIS_INVALID_KEY_ENCODING".into()))?;
    if bytes.len() > MAX_BYTES {
        return Err(AppError::Validation("REDIS_KEY_TOO_LARGE".into()));
    }
    Ok(bytes)
}
