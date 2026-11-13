mod browse;
pub mod connections;
pub mod exchange;
mod inspect;
mod mutation;
mod protocol;
mod session;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use session::Session;
use unfour_core::{AppError, AppResult};

pub use browse::{browse_collection, RedisCollectionKind, RedisCollectionPage, RedisCollectionRow};
pub use inspect::{check_key, inspect_key};
pub use mutation::{mutate_key, RedisMutation};

const PREVIEW_BYTES: usize = 16 * 1024;
const MAX_KEY_BYTES: usize = 64 * 1024;
const MAX_SCAN_BYTES: usize = 256 * 1024;
const MAX_SCAN_KEYS: usize = 1024;

#[derive(Clone)]
pub struct RedisEndpoint {
    pub host: String,
    pub port: u16,
    pub db_index: u32,
    pub username: Option<String>,
    pub tls: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisScanPage {
    pub cursor: String,
    /// Exact bytes, including non-UTF-8 and empty keys, in standard padded base64.
    pub keys: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyInfo {
    pub exists: bool,
    pub key_type: String,
    pub pttl_ms: i64,
    /// Byte length for strings, cardinality for native collection types.
    pub length: Option<u64>,
    pub preview: Option<RedisPreview>,
    pub field_found: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisPreview {
    pub base64: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisCheckResult {
    pub exists: bool,
    pub key_type: String,
    pub pttl_ms: i64,
    pub matched: bool,
}

pub async fn test_connection(endpoint: &RedisEndpoint, password: Option<&str>) -> AppResult<()> {
    let mut session = Session::connect(endpoint, password).await?;
    match session.command(&[b"PING"]).await {
        Ok(protocol::Reply::Simple(value)) if value == b"PONG" => Ok(()),
        // An authenticated least-privilege account may read keys but lack PING.
        Err(AppError::Validation(reason)) if reason == "REDIS_PERMISSION_DENIED" => Ok(()),
        Err(error) => Err(error),
        _ => Err(error("REDIS_PROTOCOL_ERROR")),
    }
}

pub async fn scan_keys(
    endpoint: &RedisEndpoint,
    password: Option<&str>,
    cursor: &str,
    pattern: &str,
) -> AppResult<RedisScanPage> {
    if !valid_cursor(cursor.as_bytes()) || pattern.len() > 4096 {
        return Err(error("REDIS_INVALID_INPUT"));
    }
    let mut session = Session::connect(endpoint, password).await?;
    session.scan(cursor.as_bytes(), pattern.as_bytes()).await
}

fn valid_cursor(cursor: &[u8]) -> bool {
    cursor.len() <= 20
        && !cursor.is_empty()
        && cursor.iter().all(u8::is_ascii_digit)
        && std::str::from_utf8(cursor).is_ok_and(|v| v.parse::<u64>().is_ok())
}

fn encoded(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

fn error(code: &'static str) -> AppError {
    AppError::Validation(code.to_owned())
}

#[cfg(test)]
mod tests;
