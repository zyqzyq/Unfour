use crate::{
    encoded, error, inspect::key_type, protocol::Reply, valid_cursor, RedisEndpoint, Session,
    MAX_KEY_BYTES, MAX_SCAN_KEYS,
};
use serde::{Deserialize, Serialize};
use unfour_core::AppResult;

const PAGE_SIZE: usize = 50;
// A Redis rank returned to the JS client must be an exact integer.
const MAX_INDEX: u64 = (1 << 53) - 1;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RedisCollectionKind {
    Hash,
    List,
    Set,
    Zset,
}

impl RedisCollectionKind {
    fn redis_type(self) -> &'static str {
        match self {
            Self::Hash => "hash",
            Self::List => "list",
            Self::Set => "set",
            Self::Zset => "zset",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisCollectionPage {
    pub cursor: String,
    pub rows: Vec<RedisCollectionRow>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisCollectionRow {
    /// Hash field, List element, Set or Sorted Set member in original bytes.
    pub member_base64: String,
    pub value_base64: Option<String>,
    /// Zero-based list index or sorted set rank in the requested ordering.
    pub index: Option<u64>,
    /// Keep Redis' exact ASCII score, including infinities.
    pub score: Option<String>,
}

pub async fn browse_collection(
    endpoint: &RedisEndpoint,
    password: Option<&str>,
    key: &[u8],
    kind: RedisCollectionKind,
    cursor: &str,
    pattern: &str,
    descending: bool,
) -> AppResult<RedisCollectionPage> {
    use RedisCollectionKind::*;
    let scan = matches!(kind, Hash | Set);
    if key.len() > MAX_KEY_BYTES
        || !valid_cursor(cursor.as_bytes())
        || pattern.len() > 4096
        || (!scan && pattern != "*")
        || (descending && kind != Zset)
    {
        return Err(error("REDIS_INVALID_INPUT"));
    }
    let offset: u64 = cursor.parse().map_err(|_| error("REDIS_INVALID_INPUT"))?;
    if !scan && offset > MAX_INDEX {
        return Err(error("REDIS_INVALID_INPUT"));
    }

    let mut session = Session::connect(endpoint, password).await?;
    if key_type(&mut session, key).await? != kind.redis_type() {
        return Err(error("REDIS_KEY_CHANGED"));
    }
    let width = if matches!(kind, Hash | Zset) { 2 } else { 1 };
    let (mut next, items) = if scan {
        let command: &[u8] = if kind == Hash { b"HSCAN" } else { b"SSCAN" };
        session
            .collection_values(
                &[
                    command,
                    key,
                    cursor.as_bytes(),
                    b"MATCH",
                    pattern.as_bytes(),
                    b"COUNT",
                    b"50",
                ],
                true,
                MAX_SCAN_KEYS * width,
            )
            .await?
    } else {
        let last_index = (offset + PAGE_SIZE as u64 - 1).min(MAX_INDEX);
        let end = last_index.to_string();
        // Fetch only the visible rows. A large member on the next page must not
        // exceed this page's budget or be serialized just to check for more.
        let args = range_arguments(kind, key, cursor.as_bytes(), end.as_bytes(), descending);
        session
            .collection_values(&args, false, (last_index - offset + 1) as usize * width)
            .await?
    };
    if !items.len().is_multiple_of(width) {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    if next != "0" && next == cursor {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    if items.is_empty() && key_type(&mut session, key).await? != kind.redis_type() {
        return Err(error("REDIS_KEY_CHANGED"));
    }
    let mut rows = Vec::with_capacity(items.len() / width);
    for (i, chunk) in items.chunks_exact(width).enumerate() {
        let score = (kind == Zset).then(|| score_text(&chunk[1])).transpose()?;
        rows.push(RedisCollectionRow {
            member_base64: encoded(&chunk[0]),
            value_base64: (kind == Hash).then(|| encoded(&chunk[1])),
            index: (!scan).then(|| offset + i as u64),
            score,
        });
    }
    if !scan {
        next = "0".into();
        let following = offset + rows.len() as u64;
        if rows.len() == PAGE_SIZE && following <= MAX_INDEX {
            let cardinality: &[u8] = if kind == List { b"LLEN" } else { b"ZCARD" };
            match session.command(&[cardinality, key]).await? {
                Reply::Number(n) if n > following as i64 => next = following.to_string(),
                Reply::Number(0..) => {}
                _ => return Err(error("REDIS_PROTOCOL_ERROR")),
            }
        }
    }
    Ok(RedisCollectionPage { cursor: next, rows })
}

fn range_arguments<'a>(
    kind: RedisCollectionKind,
    key: &'a [u8],
    start: &'a [u8],
    end: &'a [u8],
    descending: bool,
) -> Vec<&'a [u8]> {
    if kind == RedisCollectionKind::List {
        return vec![b"LRANGE", key, start, end];
    }
    let command: &[u8] = if descending { b"ZREVRANGE" } else { b"ZRANGE" };
    vec![command, key, start, end, b"WITHSCORES"]
}

fn score_text(bytes: &[u8]) -> AppResult<String> {
    let text = std::str::from_utf8(bytes).map_err(|_| error("REDIS_PROTOCOL_ERROR"))?;
    if text.len() > 64 || !text.parse::<f64>().is_ok_and(|v| !v.is_nan()) {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    Ok(text.to_owned())
}
