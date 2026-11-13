//! Typed edits of one key. All validation and queued work use one request-scoped
//! connection. Only EXEC can publish a mutation; it is never retried here.
use super::{
    error, inspect::key_type, protocol::Reply, session::Session, RedisEndpoint, MAX_KEY_BYTES,
    PREVIEW_BYTES,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use unfour_core::{AppError, AppResult};

mod transaction;
use transaction::{queue, Version, WriteResult};

#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RedisMutation {
    CreateString {
        value_base64: String,
        ttl_ms: Option<u64>,
    },
    UpdateString {
        expected_base64: String,
        value_base64: String,
    },
    CreateHash {
        field_base64: String,
        value_base64: String,
    },
    AddHashField {
        field_base64: String,
        value_base64: String,
    },
    UpdateHashField {
        field_base64: String,
        expected_base64: String,
        value_base64: String,
    },
    DeleteHashField {
        field_base64: String,
        expected_base64: String,
    },
    SetTtl {
        expected_type: String,
        ttl_ms: u64,
    },
    Persist {
        expected_type: String,
    },
    DeleteKey {
        expected_type: String,
    },
}

impl RedisMutation {
    pub fn operation_name(&self) -> &'static str {
        match self {
            Self::CreateString { .. } => "createString",
            Self::UpdateString { .. } => "updateString",
            Self::CreateHash { .. } => "createHash",
            Self::AddHashField { .. } => "addHashField",
            Self::UpdateHashField { .. } => "updateHashField",
            Self::DeleteHashField { .. } => "deleteHashField",
            Self::SetTtl { .. } => "setTtl",
            Self::Persist { .. } => "persist",
            Self::DeleteKey { .. } => "deleteKey",
        }
    }

    fn expected_type(&self) -> &str {
        match self {
            Self::CreateString { .. } | Self::CreateHash { .. } => "none",
            Self::UpdateString { .. } => "string",
            Self::AddHashField { .. }
            | Self::UpdateHashField { .. }
            | Self::DeleteHashField { .. } => "hash",
            Self::SetTtl { expected_type, .. }
            | Self::Persist { expected_type }
            | Self::DeleteKey { expected_type } => expected_type,
        }
    }

    fn validate(&self) -> AppResult<()> {
        let kind = self.expected_type();
        if kind.is_empty()
            || kind.len() > 64
            || !kind.bytes().all(|b| b.is_ascii_graphic())
            || (kind == "none"
                && !matches!(self, Self::CreateString { .. } | Self::CreateHash { .. }))
        {
            return Err(error("REDIS_INVALID_INPUT"));
        }
        if let Self::SetTtl { ttl_ms, .. }
        | Self::CreateString {
            ttl_ms: Some(ttl_ms),
            ..
        } = self
        {
            // The caller uses JavaScript numbers; reject fractional or lossy inputs at IPC.
            if !(1..=9_007_199_254_740_991).contains(ttl_ms) {
                return Err(error("REDIS_INVALID_INPUT"));
            }
        }
        Ok(())
    }
}

fn decode(input: &str, max: usize) -> AppResult<Vec<u8>> {
    if input.len() > max.div_ceil(3) * 4 {
        return Err(error("REDIS_INVALID_INPUT"));
    }
    let bytes = STANDARD
        .decode(input)
        .map_err(|_| error("REDIS_INVALID_INPUT"))?;
    if bytes.len() > max {
        return Err(error("REDIS_INVALID_INPUT"));
    }
    Ok(bytes)
}

fn conflict() -> AppError {
    error("REDIS_WRITE_CONFLICT")
}

pub async fn mutate_key(
    endpoint: &RedisEndpoint,
    password: Option<&str>,
    key: &[u8],
    mutation: &RedisMutation,
) -> AppResult<()> {
    if key.len() > MAX_KEY_BYTES {
        return Err(error("REDIS_INVALID_INPUT"));
    }
    mutation.validate()?;
    let mut session = Session::connect(endpoint, password).await?;
    let version = Version::check(&mut session).await?;
    transaction::watch(&mut session, key).await?;
    if key_type(&mut session, key).await? != mutation.expected_type() {
        return Err(conflict());
    }
    match mutation {
        RedisMutation::CreateString {
            value_base64,
            ttl_ms,
        } => {
            let value = decode(value_base64, PREVIEW_BYTES)?;
            let ttl = ttl_ms.map(|ttl| ttl.to_string());
            let mut args: Vec<&[u8]> = vec![b"SET", key, &value, b"NX"];
            if let Some(ttl) = &ttl {
                args.extend([b"PX".as_slice(), ttl.as_bytes()]);
            }
            queue(&mut session, &args, WriteResult::Ok).await
        }
        RedisMutation::UpdateString {
            expected_base64,
            value_base64,
        } => {
            let expected = decode(expected_base64, PREVIEW_BYTES)?;
            let value = decode(value_base64, PREVIEW_BYTES)?;
            match session.string_prefix(key).await? {
                Reply::Bulk(Some(current)) if current == expected => {}
                Reply::Bulk(_) => return Err(conflict()),
                _ => return Err(error("REDIS_PROTOCOL_ERROR")),
            }
            queue(
                &mut session,
                &[b"SET", key, &value, b"XX", b"KEEPTTL"],
                WriteResult::Ok,
            )
            .await
        }
        RedisMutation::CreateHash {
            field_base64,
            value_base64,
        }
        | RedisMutation::AddHashField {
            field_base64,
            value_base64,
        } => {
            let field = decode(field_base64, MAX_KEY_BYTES)?;
            let value = decode(value_base64, PREVIEW_BYTES)?;
            queue(
                &mut session,
                &[b"HSETNX", key, &field, &value],
                WriteResult::One,
            )
            .await
        }
        RedisMutation::UpdateHashField {
            field_base64,
            expected_base64,
            value_base64,
        } => {
            let field = decode(field_base64, MAX_KEY_BYTES)?;
            let expected = decode(expected_base64, PREVIEW_BYTES)?;
            let value = decode(value_base64, PREVIEW_BYTES)?;
            expect_field(&mut session, key, &field, &expected).await?;
            if version.major >= 8 {
                queue(
                    &mut session,
                    &[
                        b"HSETEX", key, b"FXX", b"KEEPTTL", b"FIELDS", b"1", &field, &value,
                    ],
                    WriteResult::One,
                )
                .await
            } else {
                if version >= Version::new(7, 4, 0) {
                    match session
                        .single_result(&[b"HPTTL", key, b"FIELDS", b"1", &field])
                        .await?
                    {
                        Some(Reply::Number(-1)) => (),
                        Some(Reply::Number(-2)) => return Err(conflict()),
                        Some(Reply::Number(0..)) => {
                            return Err(error("REDIS_FIELD_TTL_UNSUPPORTED"))
                        }
                        _ => return Err(error("REDIS_PROTOCOL_ERROR")),
                    }
                }
                queue(
                    &mut session,
                    &[b"HSET", key, &field, &value],
                    WriteResult::Zero,
                )
                .await
            }
        }
        RedisMutation::DeleteHashField {
            field_base64,
            expected_base64,
        } => {
            let field = decode(field_base64, MAX_KEY_BYTES)?;
            expect_field(
                &mut session,
                key,
                &field,
                &decode(expected_base64, PREVIEW_BYTES)?,
            )
            .await?;
            queue(&mut session, &[b"HDEL", key, &field], WriteResult::One).await
        }
        RedisMutation::SetTtl { ttl_ms, .. } => {
            queue(
                &mut session,
                &[b"PEXPIRE", key, ttl_ms.to_string().as_bytes()],
                WriteResult::One,
            )
            .await
        }
        RedisMutation::Persist { .. } => {
            queue(&mut session, &[b"PERSIST", key], WriteResult::One).await
        }
        RedisMutation::DeleteKey { .. } => {
            queue(&mut session, &[b"UNLINK", key], WriteResult::One).await
        }
    }
}

async fn expect_field(
    session: &mut Session,
    key: &[u8],
    field: &[u8],
    expected: &[u8],
) -> AppResult<()> {
    // HSTRLEN rejects a large field before Redis is asked to send it. read_field
    // also checks the HGET length header in case a concurrent writer enlarged it.
    match super::inspect::read_field(session, key, field).await? {
        Some(value) if value == expected => Ok(()),
        _ => Err(conflict()),
    }
}
