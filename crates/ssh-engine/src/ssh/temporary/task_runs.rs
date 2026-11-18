//! A temporary parent's ownership also owns all task observations.
use super::*;
use crate::ssh::task::ephemeral::{MemoryTaskRecord, TEMPORARY_TASK_MARKER};
use crate::ssh::task::shared::SharedTaskSource;
use unfour_core::models::{SshTaskRun, SshTaskRunEvent};

impl SshService {
    pub(in crate::ssh) fn register_temporary_task(
        &self,
        workspace: &str,
        handle: &str,
        record: MemoryTaskRecord,
        terminal_mode: bool,
    ) -> AppResult<(SharedTaskSource, Option<super::terminal::TerminalLease>)> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        let entry = queue.owned(workspace, handle)?;
        let cancel = entry
            .cancel
            .as_ref()
            .ok_or_else(|| failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"))?;
        if entry.summary.status != "connected"
            || entry.summary.error_code.is_some()
            || *cancel.borrow()
        {
            return Err(failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"));
        }
        if entry
            .task_runs
            .iter()
            .any(|record| record.run.status == "running")
        {
            return Err(failure("SSH_TASK_TEMPORARY_BUSY"));
        }
        let source = SharedTaskSource {
            handle: entry
                .native
                .as_ref()
                .ok_or_else(|| failure("SSH_TUNNEL_TEMPORARY_UNAVAILABLE"))?
                .handle
                .clone(),
            parent: cancel.subscribe(),
            cancel: record.cancel.subscribe(),
            pending: entry.pending_forwards.clone(),
        };
        let terminal = if terminal_mode {
            let mut lease = super::terminal::claim(self, entry, workspace, handle)?;
            lease.curl_context = None;
            Some(lease)
        } else {
            None
        };
        // Four bounded observations per parent; all disappear with the parent.
        while entry.task_runs.len() >= 4 {
            entry.task_runs.pop_front();
        }
        entry.task_runs.push_back(record);
        Ok((source, terminal))
    }

    pub(in crate::ssh) fn temporary_task_runs(
        &self,
        workspace: &str,
        task: &str,
    ) -> AppResult<Vec<SshTaskRun>> {
        let queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        Ok(queue
            .entries
            .values()
            .filter(|entry| entry.summary.workspace_id.as_deref() == Some(workspace))
            .flat_map(|entry| entry.task_runs.iter())
            .filter(|record| record.run.task_id == task)
            .map(|record| record.run.clone())
            .collect())
    }

    pub(in crate::ssh) fn temporary_task_run(
        &self,
        workspace: &str,
        id: &str,
    ) -> AppResult<Option<SshTaskRun>> {
        self.with_temporary_task(workspace, id, |record| record.run.clone())
    }

    pub(in crate::ssh) fn temporary_task_log(
        &self,
        workspace: &str,
        id: &str,
    ) -> AppResult<Option<String>> {
        self.with_temporary_task(workspace, id, |record| record.log.clone())
    }

    pub(in crate::ssh) fn cancel_temporary_task(
        &self,
        workspace: &str,
        id: &str,
    ) -> AppResult<Option<SshTaskRun>> {
        self.with_temporary_task(workspace, id, |record| {
            record.cancel.send_replace(true);
            record.run.clone()
        })
    }

    pub(in crate::ssh) fn observe_temporary_task(&self, workspace: &str, event: &SshTaskRunEvent) {
        let retained = self
            .with_temporary_task(workspace, &event.run_id, |record| {
                record.append(event);
                if event.kind == "run" && event.status.as_deref() != Some("running") {
                    record.run.status = event.status.clone().unwrap_or_else(|| "failed".into());
                    record.run.finished_at = Some(event.created_at.clone());
                    record.run.error_message = event.error.clone();
                }
            })
            .ok()
            .flatten()
            .is_some();
        if retained {
            self.emit_task_run_event(event);
        }
    }

    pub(in crate::ssh) fn clear_temporary_task_runs(
        &self,
        workspace: &str,
        task: Option<&str>,
    ) -> AppResult<usize> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        let mut removed = 0;
        for entry in queue
            .entries
            .values_mut()
            .filter(|entry| entry.summary.workspace_id.as_deref() == Some(workspace))
        {
            entry.task_runs.retain(|record| {
                let keep = record.run.status == "running"
                    || task.is_some_and(|task| task != record.run.task_id);
                if !keep {
                    removed += 1;
                }
                keep
            });
        }
        Ok(removed)
    }

    fn with_temporary_task<T>(
        &self,
        workspace: &str,
        id: &str,
        callback: impl FnOnce(&mut MemoryTaskRecord) -> T,
    ) -> AppResult<Option<T>> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        let record = queue
            .entries
            .values_mut()
            .filter(|entry| entry.summary.workspace_id.as_deref() == Some(workspace))
            .flat_map(|entry| entry.task_runs.iter_mut())
            .find(|record| record.run.id == id);
        Ok(record.map(callback))
    }

    pub fn is_temporary_task_run(&self, workspace: &str, id: &str) -> bool {
        self.temporary_task_run(workspace, id)
            .ok()
            .flatten()
            .is_some_and(|run| {
                run.warnings
                    .iter()
                    .any(|warning| warning == TEMPORARY_TASK_MARKER)
            })
    }
}
