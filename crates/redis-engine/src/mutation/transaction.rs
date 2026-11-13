use super::{conflict, error, AppError, AppResult, Reply, Session};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Version {
    pub(super) major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    pub(super) const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub(super) async fn check(session: &mut Session) -> AppResult<Self> {
        let unsupported = || error("REDIS_WRITE_VERSION_UNSUPPORTED");
        let info = match session.command(&[b"INFO", b"SERVER"]).await {
            Ok(Reply::Bulk(Some(info))) => info,
            Ok(_) => return Err(error("REDIS_PROTOCOL_ERROR")),
            Err(AppError::Validation(code)) if code == "REDIS_PERMISSION_DENIED" => {
                return Err(error("REDIS_WRITE_INFO_DENIED"));
            }
            Err(other) => return Err(other),
        };
        let info = std::str::from_utf8(&info).map_err(|_| unsupported())?;
        // A cluster node can execute writes for its local slots without a MOVED
        // reply. Require the topology we actually route before issuing WATCH.
        if !info.lines().any(|line| line == "redis_mode:standalone") {
            return Err(error("REDIS_WRITE_TOPOLOGY_UNSUPPORTED"));
        }
        let mut version = info
            .lines()
            .find_map(|s| s.strip_prefix("redis_version:"))
            .ok_or_else(unsupported)?
            .split('.');
        let mut next = || {
            version
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(unsupported)
        };
        let version = Self::new(next()?, next()?, next()?);
        // Since 6.0.9 expiry invalidates a WATCH. Earlier versions could let
        // e.g. HSETNX recreate an expired key during an edit of an existing hash.
        if version < Self::new(6, 0, 9) {
            return Err(unsupported());
        }
        Ok(version)
    }
}

pub(super) async fn watch(session: &mut Session, key: &[u8]) -> AppResult<()> {
    expect_simple(session, &[b"WATCH", key], b"OK").await
}

async fn expect_simple(session: &mut Session, args: &[&[u8]], expected: &[u8]) -> AppResult<()> {
    match session.command(args).await? {
        Reply::Simple(text) if text == expected => Ok(()),
        _ => Err(error("REDIS_PROTOCOL_ERROR")),
    }
}

pub(super) enum WriteResult {
    Ok,
    Zero,
    One,
}

pub(super) async fn queue(
    session: &mut Session,
    args: &[&[u8]],
    expected: WriteResult,
) -> AppResult<()> {
    expect_simple(session, &[b"MULTI"], b"OK").await?;
    expect_simple(session, args, b"QUEUED").await?;
    // Exactly one queued command: an explicit Redis error is definitive.
    // Once EXEC is attempted, transport/decode failures cannot establish
    // whether the server committed the write. Never retry on a new socket.
    let reply = match session.single_result(&[b"EXEC"]).await {
        Ok(Some(value)) => value,
        Ok(None) => return Err(conflict()),
        Err(AppError::Validation(code))
            if matches!(
                code.as_str(),
                "REDIS_PERMISSION_DENIED"
                    | "REDIS_AUTH_FAILED"
                    | "REDIS_COMMAND_FAILED"
                    | "REDIS_CLUSTER_UNSUPPORTED"
                    | "REDIS_KEY_CHANGED"
            ) =>
        {
            return Err(AppError::Validation(code));
        }
        Err(_) => return Err(error("REDIS_WRITE_OUTCOME_UNKNOWN")),
    };
    match (expected, reply) {
        (WriteResult::Ok, Reply::Simple(ok)) if ok == b"OK" => Ok(()),
        (WriteResult::One, Reply::Number(1)) | (WriteResult::Zero, Reply::Number(0)) => Ok(()),
        (WriteResult::One, Reply::Number(0)) | (WriteResult::Ok, Reply::Bulk(None)) => {
            Err(conflict())
        }
        _ => Err(error("REDIS_WRITE_OUTCOME_UNKNOWN")),
    }
}
