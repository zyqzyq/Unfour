//! Opt-in, memory-only observations of the already authenticated transport.
//! No command, file path or credential can be supplied by a caller. TCP accepts
//! one explicit destination solely for a no-payload channel-open check.
use super::*;
use serde::Deserialize;

#[cfg(feature = "ssh-native")]
pub(in crate::ssh) mod bounded_stream;
#[cfg(feature = "ssh-native")]
mod native;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TemporaryProbeKind {
    Exec,
    Channels,
    Sftp,
    Tcp,
    Terminal,
    Transport,
    Curl,
}

#[cfg(test)]
mod kind_tests {
    use super::*;

    #[test]
    fn transport_kind_has_an_explicit_wire_contract() {
        let kind: TemporaryProbeKind = serde_json::from_str("\"transport\"").unwrap();
        assert_eq!(kind, TemporaryProbeKind::Transport);
        assert_eq!(serde_json::to_string(&kind).unwrap(), "\"transport\"");
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporaryProbeCheck {
    pub id: String,
    pub status: String,
    pub code: String,
    pub expected_exit_code: Option<u32>,
    pub actual_exit_code: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporaryProbeReport {
    pub run_id: String,
    pub kind: TemporaryProbeKind,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub checks: Vec<TemporaryProbeCheck>,
    pub probe_file: Option<String>,
    pub cleanup: Option<String>,
}

impl SshService {
    pub fn temporary_probe(
        &self,
        workspace: &str,
        handle: &str,
    ) -> AppResult<Option<TemporaryProbeReport>> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        Ok(queue.owned(workspace, handle)?.probe.clone())
    }

    pub fn cancel_temporary_probe(
        &self,
        workspace: &str,
        handle: &str,
        run_id: &str,
    ) -> AppResult<()> {
        let mut queue = self
            .temporary
            .0
            .lock()
            .map_err(|_| failure("TEMPORARY_UNAVAILABLE"))?;
        let entry = queue.owned(workspace, handle)?;
        // An old UI response must never cancel a later run on this handle.
        if entry
            .probe
            .as_ref()
            .is_some_and(|report| report.run_id == run_id)
        {
            #[cfg(feature = "ssh-native")]
            if let Some(cancel) = &entry.probe_cancel {
                cancel.send_replace(true);
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "ssh-native"))]
    pub fn start_temporary_probe(
        &self,
        _workspace: &str,
        _handle: &str,
        _kind: TemporaryProbeKind,
        _confirmed: bool,
    ) -> AppResult<TemporaryProbeReport> {
        Err(failure("TEMPORARY_NATIVE_REQUIRED"))
    }

    #[cfg(not(feature = "ssh-native"))]
    pub fn start_temporary_tcp_probe(
        &self,
        _workspace: &str,
        _handle: &str,
        _host: &str,
        _port: u16,
        _confirmed: bool,
    ) -> AppResult<TemporaryProbeReport> {
        Err(failure("TEMPORARY_NATIVE_REQUIRED"))
    }
}
