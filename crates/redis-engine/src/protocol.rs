//! RESP2 shapes used by the read-only browser. Lengths are checked before allocating.
//! Connections are request-scoped, so a rejected frame can safely be abandoned.
use super::{
    encoded, error, valid_cursor, RedisScanPage, MAX_KEY_BYTES, MAX_SCAN_BYTES, MAX_SCAN_KEYS,
};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};
use unfour_core::{AppError, AppResult};

pub(super) enum Reply {
    Simple(Vec<u8>),
    Number(i64),
    Bulk(Option<Vec<u8>>),
}

pub(super) fn io_error(_: std::io::Error) -> AppError {
    error("REDIS_CONNECTION_FAILED")
}

fn server_error(line: &[u8]) -> AppError {
    // Server error text can echo key names, arguments or connection secrets.
    let kind = line.split(|byte| *byte == b' ').next().unwrap_or_default();
    error(match kind {
        b"NOAUTH" | b"WRONGPASS" => "REDIS_AUTH_FAILED",
        b"NOPERM" => "REDIS_PERMISSION_DENIED",
        b"WRONGTYPE" => "REDIS_KEY_CHANGED",
        b"MOVED" | b"ASK" | b"CLUSTERDOWN" => "REDIS_CLUSTER_UNSUPPORTED",
        _ => "REDIS_COMMAND_FAILED",
    })
}

async fn header(reader: &mut (impl AsyncBufRead + Unpin)) -> AppResult<(u8, Vec<u8>)> {
    let marker = reader.read_u8().await.map_err(io_error)?;
    let mut field = Vec::new();
    loop {
        let buffer = reader.fill_buf().await.map_err(io_error)?;
        if buffer.is_empty() {
            return Err(error("REDIS_CONNECTION_FAILED"));
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(buffer.len(), |position| position + 1);
        if field.len() + take > 1024 {
            return Err(error("REDIS_PROTOCOL_ERROR"));
        }
        field.extend_from_slice(&buffer[..take]);
        reader.consume(take);
        if newline.is_some() {
            break;
        }
    }
    if !field.ends_with(b"\r\n") {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    field.truncate(field.len() - 2);
    if marker == b'-' {
        return Err(server_error(&field));
    }
    Ok((marker, field))
}

fn number(bytes: &[u8]) -> AppResult<i64> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|field| field.parse().ok())
        .ok_or_else(|| error("REDIS_PROTOCOL_ERROR"))
}

async fn bulk(
    reader: &mut (impl AsyncBufRead + Unpin),
    field: &[u8],
    budget: usize,
) -> AppResult<Option<Vec<u8>>> {
    let length = number(field)?;
    if length == -1 {
        return Ok(None);
    }
    if length < 0 {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    let length = usize::try_from(length).map_err(|_| error("REDIS_RESPONSE_TOO_LARGE"))?;
    if length > budget {
        return Err(error("REDIS_RESPONSE_TOO_LARGE"));
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).await.map_err(io_error)?;
    let mut end = [0; 2];
    reader.read_exact(&mut end).await.map_err(io_error)?;
    if &end != b"\r\n" {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    Ok(Some(bytes))
}

pub(super) async fn read_reply(
    reader: &mut (impl AsyncBufRead + Unpin),
    budget: usize,
) -> AppResult<Reply> {
    let (marker, field) = header(reader).await?;
    match marker {
        b'+' => Ok(Reply::Simple(field)),
        b':' => Ok(Reply::Number(number(&field)?)),
        b'$' => Ok(Reply::Bulk(bulk(reader, &field, budget).await?)),
        _ => Err(error("REDIS_PROTOCOL_ERROR")),
    }
}

async fn array_count(reader: &mut (impl AsyncBufRead + Unpin)) -> AppResult<usize> {
    let (marker, field) = header(reader).await?;
    if marker != b'*' {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    usize::try_from(number(&field)?).map_err(|_| error("REDIS_PROTOCOL_ERROR"))
}

pub(super) async fn read_scan(
    reader: &mut (impl AsyncBufRead + Unpin),
) -> AppResult<RedisScanPage> {
    if array_count(reader).await? != 2 {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    let Reply::Bulk(Some(cursor)) = read_reply(reader, 20).await? else {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    };
    if !valid_cursor(&cursor) {
        return Err(error("REDIS_PROTOCOL_ERROR"));
    }
    let count = array_count(reader).await?;
    if count > MAX_SCAN_KEYS {
        return Err(error("REDIS_RESPONSE_TOO_LARGE"));
    }
    let mut keys = Vec::with_capacity(count);
    let mut remaining = MAX_SCAN_BYTES;
    for _ in 0..count {
        let Reply::Bulk(Some(key)) = read_reply(reader, remaining.min(MAX_KEY_BYTES)).await? else {
            return Err(error("REDIS_PROTOCOL_ERROR"));
        };
        remaining -= key.len();
        keys.push(encoded(&key));
    }
    Ok(RedisScanPage {
        cursor: String::from_utf8(cursor).map_err(|_| error("REDIS_PROTOCOL_ERROR"))?,
        keys,
    })
}
