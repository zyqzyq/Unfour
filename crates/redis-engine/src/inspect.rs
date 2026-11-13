use super::{
    encoded, error, protocol::Reply, session::Session, RedisEndpoint, RedisKeyInfo, RedisPreview,
    MAX_KEY_BYTES, PREVIEW_BYTES,
};
use unfour_core::AppResult;

pub async fn check_key(
    endpoint: &RedisEndpoint,
    password: Option<&str>,
    key: &[u8],
    field: Option<&[u8]>,
    equals: Option<&[u8]>,
) -> AppResult<super::RedisCheckResult> {
    if key.len() > MAX_KEY_BYTES
        || field.is_some_and(|f| f.len() > MAX_KEY_BYTES)
        || equals.is_some_and(|s| s.len() > PREVIEW_BYTES)
    {
        return Err(error("REDIS_INVALID_INPUT"));
    }
    let mut session = Session::connect(endpoint, password).await?;
    let kind = key_type(&mut session, key).await?;
    let matched = match (kind.as_str(), field, equals) {
        ("none", _, _) => false,
        (_, None, None) => true,
        ("hash", Some(field), None) => {
            integer(session.command(&[b"HEXISTS", key, field]).await?)? == 1
        }
        ("string", None, Some(expected)) => {
            let Reply::Bulk(Some(bytes)) = session.string_prefix(key).await? else {
                return Err(error("REDIS_PROTOCOL_ERROR"));
            };
            if bytes.len() > PREVIEW_BYTES {
                return Err(error("REDIS_RESPONSE_TOO_LARGE"));
            }
            bytes == expected
        }
        ("hash", Some(field), Some(expected)) => {
            // Reject large static values before HGET; the decoder also protects against growth.
            read_field(&mut session, key, field)
                .await?
                .is_some_and(|bytes| bytes == expected)
        }
        _ => return Err(error("REDIS_CHECK_TYPE_UNSUPPORTED")),
    };
    let ttl = if kind == "none" {
        -2
    } else {
        pttl(&mut session, key).await?
    };
    let exists = ttl != -2;
    Ok(super::RedisCheckResult {
        exists,
        key_type: if exists { kind } else { "none".into() },
        pttl_ms: ttl,
        matched: exists && matched,
    })
}

async fn pttl(session: &mut Session, key: &[u8]) -> AppResult<i64> {
    let ttl = integer(session.command(&[b"PTTL", key]).await?)?;
    if ttl < -2 {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    Ok(ttl)
}

pub(super) async fn key_type(session: &mut Session, key: &[u8]) -> AppResult<String> {
    let Reply::Simple(kind) = session.command(&[b"TYPE", key]).await? else {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    };
    if kind.is_empty() || kind.len() > 64 || !kind.iter().all(u8::is_ascii_graphic) {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    String::from_utf8(kind).map_err(|_| error("REDIS_PROTOCOL_ERROR"))
}

pub(super) async fn read_field(
    session: &mut Session,
    key: &[u8],
    field: &[u8],
) -> AppResult<Option<Vec<u8>>> {
    let length = integer(session.command(&[b"HSTRLEN", key, field]).await?)?;
    if length > PREVIEW_BYTES as i64 {
        return Err(error("REDIS_RESPONSE_TOO_LARGE"));
    }
    if length < 0 {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    let Reply::Bulk(value) = session.command(&[b"HGET", key, field]).await? else {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    };
    Ok(value)
}

pub async fn inspect_key(
    endpoint: &RedisEndpoint,
    password: Option<&str>,
    key: &[u8],
    field: Option<&[u8]>,
) -> AppResult<RedisKeyInfo> {
    if key.len() > MAX_KEY_BYTES || field.is_some_and(|f| f.len() > MAX_KEY_BYTES) {
        return Err(error("REDIS_INVALID_INPUT"));
    }
    let mut session = Session::connect(endpoint, password).await?;
    let kind = key_type(&mut session, key).await?;
    if kind == "none" {
        return Ok(missing());
    }
    if field.is_some() && kind != "hash" {
        return Err(error("REDIS_KEY_CHANGED"));
    }
    let mut info = RedisKeyInfo {
        exists: true,
        key_type: kind,
        pttl_ms: -1,
        length: None,
        preview: None,
        field_found: None,
    };
    let size_command: Option<&[u8]> = match info.key_type.as_bytes() {
        b"string" => Some(b"STRLEN"),
        b"hash" => Some(b"HLEN"),
        b"list" => Some(b"LLEN"),
        b"set" => Some(b"SCARD"),
        b"zset" => Some(b"ZCARD"),
        b"stream" => Some(b"XLEN"),
        _ => None,
    };
    if let Some(command) = size_command {
        let n = integer(session.command(&[command, key]).await?)?;
        info.length = Some(u64::try_from(n).map_err(|_| error("REDIS_PROTOCOL_ERROR"))?);
    }
    match info.key_type.as_bytes() {
        b"string" => {
            let Reply::Bulk(Some(bytes)) = session.string_prefix(key).await? else {
                return Err(error("REDIS_PROTOCOL_ERROR"));
            };
            info.preview = Some(RedisPreview {
                base64: encoded(&bytes[..bytes.len().min(PREVIEW_BYTES)]),
                truncated: bytes.len() > PREVIEW_BYTES,
            });
        }
        b"hash" => {
            if let Some(field) = field {
                let value = read_field(&mut session, key, field).await?;
                info.field_found = Some(value.is_some());
                info.preview = value.map(|bytes| RedisPreview {
                    base64: encoded(&bytes),
                    truncated: false,
                });
            }
        }
        _ => {}
    }
    info.pttl_ms = pttl(&mut session, key).await?;
    Ok(if info.pttl_ms == -2 { missing() } else { info })
}

fn integer(reply: Reply) -> AppResult<i64> {
    match reply {
        Reply::Number(number) => Ok(number),
        _ => Err(error("REDIS_PROTOCOL_ERROR")),
    }
}

fn missing() -> RedisKeyInfo {
    RedisKeyInfo {
        exists: false,
        key_type: "none".to_owned(),
        pttl_ms: -2,
        length: None,
        preview: None,
        field_found: None,
    }
}
