use super::*;

impl SshService {
    pub async fn test_connection(&self, input: SshConnectionInput) -> AppResult<SshTestResult> {
        validate_workspace_id(&input.workspace_id)?;
        let storage = input_to_storage(&input)?;
        let now = Utc::now().to_rfc3339();
        let temp_id = Uuid::new_v4().to_string();
        let connection = SshConnection {
            id: temp_id.clone(),
            workspace_id: input.workspace_id.clone(),
            name: input.name.trim().to_string(),
            host: storage.host,
            port: storage.port,
            username: storage.username,
            auth_kind: storage.auth_method,
            key_path: storage.config.key_path,
            credential_ref: input.credential_ref.clone(),
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
            revision: 0,
            sync_status: "new".to_string(),
            remote_id: None,
        };
        let connect_input = SshConnectInput {
            workspace_id: input.workspace_id.clone(),
            connection_id: temp_id,
            cols: Some(80),
            rows: Some(24),
            secret: input.secret.clone(),
        };
        let started = Instant::now();
        let fields = serde_json::json!({
            "auth_method": &connection.auth_kind,
            "host": &connection.host,
            "port": connection.port,
        });
        unfour_diag::log_operation_event(
            "ssh_test_started",
            "ssh",
            "test_connection",
            "started",
            None,
            None,
            fields.clone(),
        );

        #[cfg(feature = "ssh-native")]
        let result = self.connect_native(&connection, &connect_input).await;
        #[cfg(not(feature = "ssh-native"))]
        let result = self.connect_simulated(&connection, &connect_input).await;
        match result {
            Ok(summary) => {
                unfour_diag::log_operation_event(
                    "ssh_test_succeeded",
                    "ssh",
                    "test_connection",
                    "ok",
                    Some(started.elapsed().as_millis()),
                    None,
                    fields,
                );
                let _ = self
                    .close_session(SshCloseInput {
                        workspace_id: summary.workspace_id.clone(),
                        session_id: summary.session_id.clone(),
                    })
                    .await;
                Ok(SshTestResult {
                    ok: true,
                    message: format!(
                        "Connected to {}@{} successfully",
                        summary.username, summary.host
                    ),
                })
            }
            Err(error) => {
                unfour_diag::log_operation_event(
                    "ssh_test_failed",
                    "ssh",
                    "test_connection",
                    "error",
                    Some(started.elapsed().as_millis()),
                    Some(unfour_diag::app_error_kind(&error)),
                    fields,
                );
                Ok(SshTestResult {
                    ok: false,
                    message: error.to_string(),
                })
            }
        }
    }

    pub async fn get_host_fingerprint(
        &self,
        input: SshHostKeyInput,
    ) -> AppResult<Option<SshHostFingerprintInfo>> {
        validate_workspace_id(&input.workspace_id)?;
        let host = input.host.trim().to_string();
        if host.is_empty() {
            return Err(AppError::Validation("host cannot be empty".to_string()));
        }
        if input.port == 0 {
            return Err(AppError::Validation("port cannot be 0".to_string()));
        }

        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        match host_key_store
            .get_fingerprint_info(&input.workspace_id, &host, input.port)
            .await?
        {
            Some((fingerprint, created_at)) => Ok(Some(SshHostFingerprintInfo {
                workspace_id: input.workspace_id,
                host,
                port: input.port,
                fingerprint,
                created_at,
            })),
            None => Ok(None),
        }
    }

    /// Remove the stored fingerprint for a host:port pair, allowing the next
    /// connection to establish a new trust (TOFU reset).
    pub async fn reset_host_fingerprint(&self, input: SshHostKeyInput) -> AppResult<bool> {
        validate_workspace_id(&input.workspace_id)?;
        let host = input.host.trim().to_string();
        if host.is_empty() {
            return Err(AppError::Validation("host cannot be empty".to_string()));
        }
        if input.port == 0 {
            return Err(AppError::Validation("port cannot be 0".to_string()));
        }

        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        host_key_store
            .delete_fingerprint(&input.workspace_id, &host, input.port)
            .await
    }

    /// List all stored host-key fingerprints in a workspace.
    pub async fn list_all_host_fingerprints(
        &self,
        workspace_id: String,
    ) -> AppResult<Vec<SshHostFingerprintInfo>> {
        validate_workspace_id(&workspace_id)?;
        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        let entries = host_key_store.list_all(&workspace_id).await?;
        Ok(entries
            .into_iter()
            .map(|entry| SshHostFingerprintInfo {
                workspace_id: entry.workspace_id,
                host: entry.host,
                port: entry.port.clamp(0, u16::MAX as i64) as u16,
                fingerprint: entry.fingerprint,
                created_at: entry.created_at,
            })
            .collect())
    }

    /// Import entries from OpenSSH known_hosts content.
    pub async fn import_known_hosts(
        &self,
        input: SshKnownHostsImportInput,
    ) -> AppResult<SshKnownHostsImportResult> {
        validate_workspace_id(&input.workspace_id)?;
        if input.content.trim().is_empty() {
            return Err(AppError::Validation(
                "known_hosts content cannot be empty".to_string(),
            ));
        }
        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        host_key_store
            .import_known_hosts(&input.workspace_id, &input.content)
            .await
    }

    /// Export stored fingerprints to OpenSSH known_hosts format.
    pub async fn export_known_hosts(
        &self,
        input: SshKnownHostsExportInput,
    ) -> AppResult<SshKnownHostsExportResult> {
        validate_workspace_id(&input.workspace_id)?;
        let host_key_store = HostKeyStore::new(self.db.pool().clone());
        let (content, entry_count) = host_key_store
            .export_known_hosts(&input.workspace_id)
            .await?;
        Ok(SshKnownHostsExportResult {
            content,
            entry_count,
        })
    }

    pub(super) async fn get_connection(
        &self,
        workspace_id: &str,
        id: &str,
    ) -> AppResult<SshConnection> {
        validate_workspace_id(workspace_id)?;
        validate_connection_id(id)?;

        let row = sqlx::query_as::<_, StoredSshConnection>(
            r#"
            SELECT
              c.id, c.workspace_id, c.name, c.host, c.port,
              sub.username, sub.auth_method, sub.config_json, c.credential_ref,
              c.created_at, c.updated_at, c.deleted_at, c.revision, c.sync_status, c.remote_id
            FROM connections c
            INNER JOIN ssh_connections sub ON sub.connection_id = c.id
            WHERE c.id = ?1 AND c.workspace_id = ?2
              AND c.connection_type = 'ssh' AND c.deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(self.db.pool())
        .await?;

        row.map(stored_to_ssh_connection)
            .transpose()?
            .ok_or_else(|| AppError::NotFound("ssh connection".to_string()))
    }

    pub(super) async fn close_sessions_for_connection(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();

        // Collect native handles under the lock, then disconnect outside it.
        #[cfg(feature = "ssh-native")]
        let native_handles: Vec<NativeSshHandle> = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            sessions
                .values()
                .filter(|state| {
                    state.summary.workspace_id == workspace_id
                        && state.summary.connection_id == connection_id
                        && is_live_status(&state.summary.status)
                })
                .filter_map(|state| state.native_handle.clone())
                .collect()
        };

        // Mark live sessions disconnected, capture their ids, and append the
        // close notice to the persisted terminal output.
        let session_ids: Vec<String> = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            let mut ids = Vec::new();
            for state in sessions.values_mut().filter(|state| {
                state.summary.workspace_id == workspace_id
                    && state.summary.connection_id == connection_id
                    && is_live_status(&state.summary.status)
            }) {
                #[cfg(feature = "ssh-native")]
                {
                    state.intentional_close = true;
                    if let Some(cancel_tx) = state.cancel_tx.take() {
                        let _ = cancel_tx.send(true);
                    }
                    state.native_handle.take();
                }

                state.summary.status = "disconnected".to_string();
                state.summary.reconnect_attempt = 0;
                state.summary.updated_at = now.clone();
                append_pending_output(
                    &mut state.pending_output,
                    "SSH session closed because the connection was deleted.\r\n",
                );
                ids.push(state.summary.session_id.clone());
            }
            ids
        };

        // Flush buffered terminal output to the database before dropping entries.
        for id in &session_ids {
            let _ = self.flush_session_history(id).await;
        }

        // Drop the in-memory entries so the session map cannot grow without
        // bound across the process lifetime (#4).
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            sessions.retain(|id, _| !session_ids.contains(id));
        }

        // Disconnect native handles outside the mutex lock.
        #[cfg(feature = "ssh-native")]
        for native in native_handles {
            // Close the channel first.
            let _ = native.writer.close().await;
            let handle = native.handle.lock().await;
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "connection deleted", "en")
                .await;
        }

        Ok(())
    }
}
