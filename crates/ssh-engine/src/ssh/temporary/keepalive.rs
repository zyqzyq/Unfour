//! Protocol-level liveness, independent of PTY input and authorization replay.
use super::*;
use russh::client::DisconnectReason;

const INTERVAL: Duration = Duration::from_secs(60);
const MAX_MISSES: usize = 3;

pub(super) fn client_config() -> russh::client::Config {
    let mut config = native_client_config();
    // russh sends only after authentication and resets this idle timer whenever
    // it receives SSH data. The 15s PTY setup deadline remains earlier than the
    // first 60s probe, including when authentication took its full 30s budget.
    // Use its reply/transport bookkeeping instead of a separate ping task.
    config.keepalive_interval = Some(INTERVAL);
    config.keepalive_max = MAX_MISSES;
    config.inactivity_timeout = None;
    config
}

pub(in crate::ssh) fn record_transport_disconnect(
    trust: &TrustPrompt,
    workspace: &str,
    reason: &DisconnectReason<russh::Error>,
) {
    let code = match reason {
        DisconnectReason::ReceivedDisconnect(_) => "TEMPORARY_REMOTE_DISCONNECTED",
        DisconnectReason::Error(russh::Error::KeepaliveTimeout) => "TEMPORARY_KEEPALIVE_TIMEOUT",
        DisconnectReason::Error(_) => "TEMPORARY_TRANSPORT_CLOSED",
    };
    if let Some(queue) = trust.queue.upgrade() {
        record(&queue, workspace, &trust.handle, code);
    }
}

pub(super) fn record_channel_close(
    service: &SshService,
    workspace: &str,
    handle: &str,
    code: &'static str,
) {
    record(&service.temporary.0, workspace, handle, code);
}

fn record(queue: &Mutex<Queue>, workspace: &str, handle: &str, code: &'static str) {
    let Ok(mut queue) = queue.lock() else { return };
    let Ok(entry) = queue.owned(workspace, handle) else {
        return;
    };
    // User close removes the entry. Internal teardown signals cancellation
    // before sending its own disconnect; neither should become a remote error.
    // A host-key/rekey rejection already has the more useful fixed diagnosis.
    let connected_failure = entry.summary.status == "connected"
        && entry.summary.error_code.is_none()
        && !entry.cancel.as_ref().is_some_and(|cancel| *cancel.borrow());
    // The reader may observe EOF before the transport callback runs. Permit a
    // late specific reason to refine only our generic transport fallback, even
    // after internal teardown signals cancellation. Explicit shell exit and
    // security failures remain authoritative; user close already removed entry.
    let specific_after_fallback = matches!(
        code,
        "TEMPORARY_KEEPALIVE_TIMEOUT" | "TEMPORARY_REMOTE_DISCONNECTED"
    ) && entry.summary.error_code.as_deref()
        == Some("TEMPORARY_TRANSPORT_CLOSED")
        && matches!(entry.summary.status.as_str(), "connected" | "disconnected");
    if connected_failure || specific_after_fallback {
        entry.summary.error_code = Some(code.into());
    }
}

#[cfg(test)]
mod tests;
