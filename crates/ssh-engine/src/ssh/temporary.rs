//! Internal typed ingress. No argv/URI parser and no credential-bearing IPC DTO.
use super::*;
use serde::Serialize;
use std::time::Duration;
use tokio::time::Instant;
use zeroize::Zeroizing;

#[cfg(feature = "ssh-native")]
mod native;
#[cfg(feature = "ssh-native")]
pub(super) use native::TrustPrompt;
#[cfg(test)]
mod tests;

const RETENTION: Duration = Duration::from_secs(120);
const MAX_REQUESTS: usize = 16;
const MAX_RECEIPTS: usize = 256;

/// Generated once at a trusted ingress boundary, reused only for redelivery.
/// A new user launch gets a new receipt even for identical credentials.
#[derive(Clone)]
pub struct LaunchReceipt {
    id: String,
    deadline: Instant,
}
impl Default for LaunchReceipt {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            deadline: Instant::now() + RETENTION,
        }
    }
}

/// Deliberately neither Debug, Clone nor Serialize/Deserialize. Password is
/// an internal supported mechanism, not a claim about AccessClient's contract.
/// ```compile_fail
/// use unfour_ssh_engine::ssh::temporary::TemporaryRequest;
/// fn assert_debug<T: std::fmt::Debug>() {}
/// assert_debug::<TemporaryRequest>();
/// ```
/// ```compile_fail
/// use unfour_ssh_engine::ssh::temporary::TemporaryRequest;
/// let request = TemporaryRequest::password("fixture.invalid".into(), 22, "synthetic".into(), "synthetic".into()).unwrap();
/// serde_json::to_string(&request).unwrap();
/// ```
pub struct TemporaryRequest {
    host: Zeroizing<String>,
    port: u16,
    username: Zeroizing<String>,
    password: Zeroizing<String>,
}
impl TemporaryRequest {
    pub fn password(
        host: String,
        port: u16,
        username: String,
        password: String,
    ) -> AppResult<Self> {
        let request = Self {
            host: Zeroizing::new(host),
            port,
            username: Zeroizing::new(username),
            password: Zeroizing::new(password),
        };
        if request.host.is_empty()
            || request.port == 0
            || request.username.is_empty()
            || request.host.len() + request.username.len() + request.password.len() > 64 * 1024
            || request
                .host
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(failure("TEMPORARY_INVALID_REQUEST"));
        }
        Ok(request)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporarySummary {
    pub handle: String,
    pub workspace_id: Option<String>,
    pub status: String,
    pub expires_in_seconds: u64,
    pub fingerprint: Option<String>,
    pub error_code: Option<String>,
}

struct Entry {
    summary: TemporarySummary,
    deadline: Instant,
    request: Option<TemporaryRequest>,
    #[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
    trusted_fingerprint: Option<String>,
    #[cfg(feature = "ssh-native")]
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    #[cfg(feature = "ssh-native")]
    trust: Option<tokio::sync::oneshot::Sender<bool>>,
    #[cfg(feature = "ssh-native")]
    native: Option<NativeSshHandle>,
    output: String,
}

#[derive(Default)]
struct Queue {
    entries: HashMap<String, Entry>,
    // Bounded tombstones prevent a cancelled/consumed receipt being delivered
    // again while valid. Expired receipts are always rejected, even after GC.
    receipts: HashMap<String, (Instant, String)>,
    workspace_epochs: HashMap<String, u64>,
}
#[derive(Clone, Default)]
pub(super) struct TemporarySessions(Arc<Mutex<Queue>>);

impl TemporarySessions {
    fn schedule_sweep(&self, runtime: &tokio::runtime::Handle, deadline: Instant) {
        // A weak timer retains no credentials and works without UI readiness.
        let weak = Arc::downgrade(&self.0);
        runtime.spawn(async move {
            tokio::time::sleep_until(deadline).await;
            if let Some(queue) = weak.upgrade() {
                if let Ok(mut queue) = queue.lock() {
                    queue.sweep(Instant::now());
                }
            }
        });
    }
}

fn failure(code: &str) -> AppError {
    AppError::Validation(code.to_owned())
}

impl Queue {
    fn sweep(&mut self, now: Instant) {
        self.receipts.retain(|_, (deadline, _)| *deadline > now);
        for entry in self.entries.values_mut() {
            if entry.request.is_some() && entry.deadline <= now {
                entry.request = None;
                entry.summary.status = "expired".into();
            }
        }
        // Closed/expired entries do not retain authorization or grow forever.
        self.entries.retain(|_, e| {
            e.deadline > now
                || matches!(
                    e.summary.status.as_str(),
                    "connecting" | "awaiting-trust" | "connected"
                )
        });
    }
    fn owned(&mut self, workspace: &str, handle: &str) -> AppResult<&mut Entry> {
        self.entries
            .get_mut(handle)
            .filter(|e| e.summary.workspace_id.as_deref() == Some(workspace))
            .ok_or_else(|| failure("TEMPORARY_NOT_FOUND"))
    }
}

impl SshService {
    /// Only trusted Rust adapters can submit typed requests. Never exposed by
    /// Tauri/MCP; test injection uses this same boundary in automated tests.
    pub fn receive_temporary(
        &self,
        receipt: &LaunchReceipt,
        request: TemporaryRequest,
    ) -> AppResult<String> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| failure("TEMPORARY_RUNTIME_REQUIRED"))?;
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        let now = Instant::now();
        queue.sweep(now);
        if receipt.deadline <= now {
            return Err(failure("TEMPORARY_EXPIRED"));
        }
        if let Some((_, handle)) = queue.receipts.get(&receipt.id) {
            return Ok(handle.clone());
        }
        if queue.entries.len() >= MAX_REQUESTS || queue.receipts.len() >= MAX_RECEIPTS {
            return Err(failure("TEMPORARY_QUEUE_FULL"));
        }
        let handle = format!("temporary-{}", Uuid::new_v4());
        queue
            .receipts
            .insert(receipt.id.clone(), (receipt.deadline, handle.clone()));
        queue.entries.insert(
            handle.clone(),
            Entry {
                summary: TemporarySummary {
                    handle: handle.clone(),
                    workspace_id: None,
                    status: "pending".into(),
                    expires_in_seconds: 120,
                    fingerprint: None,
                    error_code: None,
                },
                deadline: receipt.deadline,
                request: Some(request),
                output: String::new(),
                trusted_fingerprint: None,
                #[cfg(feature = "ssh-native")]
                cancel: None,
                #[cfg(feature = "ssh-native")]
                trust: None,
                #[cfg(feature = "ssh-native")]
                native: None,
            },
        );
        self.temporary.schedule_sweep(&runtime, receipt.deadline);
        Ok(handle)
    }

    pub fn list_temporary(&self, workspace: &str) -> AppResult<Vec<TemporarySummary>> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        queue.sweep(Instant::now());
        Ok(queue
            .entries
            .values()
            .filter(|e| {
                e.summary.workspace_id.is_none()
                    || e.summary.workspace_id.as_deref() == Some(workspace)
            })
            .map(|e| {
                let mut summary = e.summary.clone();
                summary.expires_in_seconds = e
                    .deadline
                    .saturating_duration_since(Instant::now())
                    .as_secs();
                summary
            })
            .collect())
    }

    pub async fn claim_temporary(
        &self,
        workspace: &str,
        handle: &str,
    ) -> AppResult<TemporarySummary> {
        let epoch = *self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?
            .workspace_epochs
            .get(workspace)
            .unwrap_or(&0);
        // Verify an existing live workspace, never merely a syntactically valid ID.
        let exists: (i64,) =
            sqlx::query_as("SELECT count(*) FROM workspaces WHERE id = ? AND deleted_at IS NULL")
                .bind(workspace)
                .fetch_one(self.db.pool())
                .await?;
        if exists.0 != 1 {
            return Err(failure("TEMPORARY_WORKSPACE_UNAVAILABLE"));
        }
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        if *queue.workspace_epochs.get(workspace).unwrap_or(&0) != epoch {
            return Err(failure("TEMPORARY_WORKSPACE_CLOSED"));
        }
        queue.sweep(Instant::now());
        let entry = queue
            .entries
            .get_mut(handle)
            .ok_or_else(|| failure("TEMPORARY_NOT_FOUND"))?;
        if let Some(owner) = &entry.summary.workspace_id {
            if owner != workspace {
                return Err(failure("TEMPORARY_NOT_FOUND"));
            }
            return Ok(entry.summary.clone());
        }
        if entry.summary.status != "pending" {
            return Err(failure("TEMPORARY_CONSUMED"));
        }
        entry.summary.workspace_id = Some(workspace.into());
        entry.summary.status = "claimed".into();
        Ok(entry.summary.clone())
    }

    pub fn cancel_temporary(&self, workspace: &str, handle: &str) -> AppResult<()> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        if let Some(entry) = queue.entries.get(handle) {
            if entry
                .summary
                .workspace_id
                .as_deref()
                .is_some_and(|owner| owner != workspace)
            {
                return Err(failure("TEMPORARY_NOT_FOUND"));
            }
        }
        if let Some(mut entry) = queue.entries.remove(handle) {
            entry.request = None;
            #[cfg(feature = "ssh-native")]
            if let Some(cancel) = entry.cancel.take() {
                let _ = cancel.send(true);
            }
        }
        Ok(())
    }

    pub fn close_temporary_workspace(&self, workspace: &str) -> AppResult<()> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        let epoch = queue.workspace_epochs.entry(workspace.into()).or_default();
        *epoch = epoch.wrapping_add(1);
        queue.entries.retain(|_, entry| {
            if entry.summary.workspace_id.as_deref() != Some(workspace) {
                return true;
            }
            #[cfg(feature = "ssh-native")]
            if let Some(cancel) = entry.cancel.take() {
                let _ = cancel.send(true);
            }
            false
        });
        Ok(())
    }

    /// Destructive drain: terminal output is transient, bounded, never history.
    pub fn temporary_output(
        &self,
        workspace: &str,
        handle: &str,
    ) -> AppResult<Vec<SshSessionEvent>> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        let entry = queue.owned(workspace, handle)?;
        let data = std::mem::take(&mut entry.output);
        Ok(if data.is_empty() {
            vec![]
        } else {
            vec![SshSessionEvent {
                session_id: handle.into(),
                kind: "output".into(),
                data,
                created_at: Utc::now().to_rfc3339(),
            }]
        })
    }

    #[cfg(not(feature = "ssh-native"))]
    pub fn start_temporary(&self, _workspace: &str, _handle: &str) -> AppResult<()> {
        Err(failure("TEMPORARY_NATIVE_REQUIRED"))
    }
    #[cfg(not(feature = "ssh-native"))]
    pub fn trust_temporary(
        &self,
        _workspace: &str,
        _handle: &str,
        _fingerprint: &str,
    ) -> AppResult<()> {
        Err(failure("TEMPORARY_NATIVE_REQUIRED"))
    }
}
