use base64::{engine::general_purpose::STANDARD, Engine};
use unfour_core::{domain::CommandContext, AppError, AppResult};
use unfour_redis_engine::connections;
use zeroize::Zeroizing;

mod scan_cursor;
use crate::transaction::CommandActivity;
use crate::{CommandBus, RedisConnection, RedisConnectionInput, RedisKeyInfo, RedisScanPage};

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
        let existing_ref = self.redis_draft_reference(&input, secret.is_some()).await?;
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
    ) -> AppResult<Option<String>> {
        let mut db = self.db.pool().acquire().await?;
        match input.id.as_deref() {
            Some(id) => {
                let (record, reference) =
                    connections::get_on(&mut db, &input.workspace_id, id).await?;
                input.check_existing(&record, supplied)?;
                Ok(reference)
            }
            None => {
                connections::ensure_workspace_on(&mut db, &input.workspace_id).await?;
                Ok(None)
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
        let reference = self.redis_draft_reference(&input, secret.is_some()).await?;
        let password = match secret {
            Some(value) => (!value.is_empty()).then(|| Zeroizing::new(value)),
            None => {
                self.read_redis_password(&input.workspace_id, reference)
                    .await?
            }
        };
        unfour_redis_engine::test_connection(&endpoint, password.as_deref().map(String::as_str))
            .await
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
            &record.endpoint(),
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
            &record.endpoint(),
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
            &record.endpoint(),
            password.as_deref().map(String::as_str),
            key,
            field,
            equals,
        )
        .await
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
