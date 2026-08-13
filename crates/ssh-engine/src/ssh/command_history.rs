use super::*;

impl SshService {
    pub async fn list_command_history(
        &self,
        query: SshCommandHistoryQuery,
    ) -> AppResult<Vec<SshCommandHistoryEntry>> {
        self.command_history.list(query).await
    }

    pub(super) async fn record_executed_commands(
        &self,
        workspace_id: &str,
        connection_id: &str,
        session_id: &str,
        commands: Vec<String>,
        executed_at: &str,
    ) {
        for command in commands {
            if let Err(error) = self
                .command_history
                .record(SshCommandHistoryRecordInput {
                    workspace_id: workspace_id.to_string(),
                    connection_id: connection_id.to_string(),
                    session_id: Some(session_id.to_string()),
                    command,
                    cwd: None,
                    exit_code: None,
                    duration_ms: None,
                    executed_at: executed_at.to_string(),
                })
                .await
            {
                // The PTY write already succeeded and cannot be rolled back.
                // Keep the interactive stream responsive, while surfacing a
                // structured diagnostic for persistence troubleshooting.
                unfour_diag::log_operation_event(
                    "ssh_command_history_record_failed",
                    "ssh",
                    "record_command_history",
                    "error",
                    None,
                    Some(unfour_diag::app_error_kind(&error)),
                    serde_json::json!({ "connection_id": connection_id }),
                );
            }
        }
    }

    #[cfg_attr(not(feature = "ssh-native"), allow(dead_code))]
    pub(super) fn spawn_record_executed_commands(
        &self,
        workspace_id: String,
        connection_id: String,
        session_id: String,
        commands: Vec<String>,
    ) {
        if commands.is_empty() {
            return;
        }
        let service = self.clone();
        let executed_at = Utc::now().to_rfc3339();
        tokio::spawn(async move {
            service
                .record_executed_commands(
                    &workspace_id,
                    &connection_id,
                    &session_id,
                    commands,
                    &executed_at,
                )
                .await;
        });
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) async fn ingest_terminal_output(
        &self,
        workspace_id: &str,
        session_id: &str,
        output: &str,
    ) {
        let (connection_id, commands) = {
            let Ok(mut sessions) = self.sessions.lock() else {
                return;
            };
            let Ok(state) = session_for_workspace_mut(&mut sessions, workspace_id, session_id)
            else {
                return;
            };
            let commands = state.command_line.observe_output(output);
            (state.summary.connection_id.clone(), commands)
        };
        self.record_executed_commands(
            workspace_id,
            &connection_id,
            session_id,
            commands,
            &Utc::now().to_rfc3339(),
        )
        .await;
    }
}
