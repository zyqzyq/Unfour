//! Internal typed ingress. No argv/URI parser and no credential-bearing IPC DTO.
use super::*;
use serde::Serialize;
use std::time::Duration;
use tokio::time::Instant;
use zeroize::Zeroizing;

pub(in crate::ssh) mod probe;
pub use probe::{TemporaryProbeCheck, TemporaryProbeKind, TemporaryProbeReport};
mod terminal_operation;
pub use terminal_operation::{TemporaryTerminalOperation, TerminalOperationResult};

#[cfg(feature = "ssh-native")]
mod keepalive;
#[cfg(feature = "ssh-native")]
mod native;
#[cfg(feature = "ssh-native")]
pub(super) use keepalive::record_transport_disconnect;
#[cfg(feature = "ssh-native")]
mod task_runs;
#[cfg(feature = "ssh-native")]
pub(in crate::ssh) mod terminal;
#[cfg(feature = "ssh-native")]
pub(super) use native::TrustPrompt;
#[cfg(test)]
mod tests;

const RETENTION: Duration = Duration::from_secs(120);
const MAX_REQUESTS: usize = 16;
const MAX_RECEIPTS: usize = 256;
#[cfg(feature = "ssh-native")]
const MAX_TRUSTED_HOSTS: usize = 1024;

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
    display_name: String,
}
impl TemporaryRequest {
    pub fn password(
        host: String,
        port: u16,
        username: String,
        password: String,
    ) -> AppResult<Self> {
        let request = Self {
            display_name: String::new(),
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

    /// Display-only launcher metadata. Never use it for dialing or authentication.
    pub fn with_display_name(mut self, label: &str) -> Self {
        // Launch labels can be arbitrary. Never repeat credential-bearing URLs
        // or login material into a frontend summary, even if mislabeled upstream.
        if label.contains("://")
            || label.contains(self.username.as_str())
            || (!self.password.is_empty() && label.contains(self.password.as_str()))
        {
            return self;
        }
        let clean: String = label.chars().filter(|c| {
            !c.is_control() && !matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
        }).collect();
        if clean.contains("://")
            || clean.contains(self.username.as_str())
            || (!self.password.is_empty() && clean.contains(self.password.as_str()))
        {
            return self;
        }
        let mut chars = clean.trim().chars();
        self.display_name = chars.by_ref().take(160).collect();
        if chars.next().is_some() {
            self.display_name.push('…');
        }
        self
    }

    fn display_endpoint(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host.as_str(), self.port)
        } else {
            format!("{}:{}", self.host.as_str(), self.port)
        }
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
    pub display_name: String,
    pub endpoint: String,
    pub terminal_task_busy: bool,
    pub terminal_recovery_required: bool,
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
    native: Option<NativeSshHandle>,
    #[cfg(feature = "ssh-native")]
    pending_forwards: Arc<tokio::sync::Semaphore>,
    // Only child session IDs are retained, never the downstream credentials.
    children: std::collections::HashSet<String>,
    output: String,
    probe: Option<TemporaryProbeReport>,
    #[cfg(feature = "ssh-native")]
    probe_cancel: Option<tokio::sync::watch::Sender<bool>>,
    #[cfg(feature = "ssh-native")]
    task_runs: std::collections::VecDeque<super::task::ephemeral::MemoryTaskRecord>,
    #[cfg(feature = "ssh-native")]
    terminal: Option<terminal::TerminalOwner>,
    #[cfg(feature = "ssh-native")]
    login_prompt: super::login_prompt::LoginPrompt,
    #[cfg(feature = "ssh-native")]
    input_gate: Arc<tokio::sync::Mutex<()>>,
    #[cfg(feature = "ssh-native")]
    input_paused: tokio::sync::watch::Sender<bool>,
}

#[derive(Default)]
struct Queue {
    entries: HashMap<String, Entry>,
    // Bounded tombstones prevent a cancelled/consumed receipt being delivered
    // again while valid. Expired receipts are always rejected, even after GC.
    receipts: HashMap<String, (Instant, String)>,
    workspace_epochs: HashMap<String, u64>,
    // First-use pins survive individual launches, but never leave memory.
    // Closing the workspace releases its endpoint associations.
    #[cfg(feature = "ssh-native")]
    trusted_hosts: HashMap<(String, String, u16), String>,
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
            #[cfg(feature = "ssh-native")]
            if !e.task_runs.is_empty() {
                return true;
            }
            e.deadline > now
                // A disconnected probe may contain the sole record of a file
                // needing manual cleanup. Keep its bounded, memory-only report
                // until the tab/workspace closes, not just the 120s ingress TTL.
                // MAX_REQUESTS still caps all retained entries.
                || e.probe.is_some()
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
    pub(super) fn attach_temporary_child(
        &self,
        workspace: &str,
        parent: &str,
        child: &str,
    ) -> AppResult<()> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"))?;
        let entry = queue
            .owned(workspace, parent)
            .map_err(|_| failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"))?;
        if entry.summary.status != "connected" || entry.summary.error_code.is_some() {
            return Err(failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"));
        }
        entry.children.insert(child.to_owned());
        Ok(())
    }

    pub(super) fn temporary_parent_for_child(
        &self,
        workspace: &str,
        child: &str,
    ) -> AppResult<String> {
        let queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"))?;
        queue
            .entries
            .values()
            .find(|e| {
                e.summary.workspace_id.as_deref() == Some(workspace) && e.children.contains(child)
            })
            .map(|e| e.summary.handle.clone())
            .ok_or_else(|| failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"))
    }

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
                    display_name: if request.display_name.is_empty() {
                        request.display_endpoint()
                    } else {
                        request.display_name.clone()
                    },
                    endpoint: request.display_endpoint(),
                    terminal_task_busy: false,
                    terminal_recovery_required: false,
                },
                deadline: receipt.deadline,
                request: Some(request),
                children: Default::default(),
                output: String::new(),
                probe: None,
                #[cfg(feature = "ssh-native")]
                probe_cancel: None,
                #[cfg(feature = "ssh-native")]
                task_runs: Default::default(),
                #[cfg(feature = "ssh-native")]
                terminal: None,
                #[cfg(feature = "ssh-native")]
                login_prompt: Default::default(),
                #[cfg(feature = "ssh-native")]
                input_gate: Arc::new(tokio::sync::Mutex::new(())),
                #[cfg(feature = "ssh-native")]
                input_paused: tokio::sync::watch::channel(false).0,
                trusted_fingerprint: None,
                #[cfg(feature = "ssh-native")]
                cancel: None,
                #[cfg(feature = "ssh-native")]
                native: None,
                #[cfg(feature = "ssh-native")]
                pending_forwards: Arc::new(tokio::sync::Semaphore::new(32)),
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
        #[cfg(feature = "ssh-native")]
        queue
            .trusted_hosts
            .retain(|(owner, _, _), _| owner != workspace);
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
