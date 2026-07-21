use super::*;

mod support;
use support::*;

#[cfg(feature = "ssh-native")]
use russh_sftp::client::SftpSession;
#[cfg(feature = "ssh-native")]
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(feature = "ssh-native")]
const TRANSFER_BUFFER_SIZE: usize = 64 * 1024;
#[cfg(feature = "ssh-native")]
const MAX_FINISHED_SFTP_TRANSFERS: usize = 32;

impl SshService {
    pub async fn sftp_open(&self, input: SftpSessionInput) -> AppResult<SftpOpenResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;

        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = input;
            return Err(AppError::Config(
                "SFTP requires the native SSH transport".to_string(),
            ));
        }

        #[cfg(feature = "ssh-native")]
        {
            let (native, connection_id, generation) = {
                let mut sessions = self
                    .sessions
                    .lock()
                    .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
                let state = session_for_workspace_mut(
                    &mut sessions,
                    &input.workspace_id,
                    &input.session_id,
                )?;
                ensure_session_active(state)?;
                if let Some(sftp) = &state.sftp {
                    return Ok(SftpOpenResult {
                        workspace_id: input.workspace_id,
                        session_id: input.session_id,
                        connection_id: state.summary.connection_id.clone(),
                        home_path: sftp.home_path.clone(),
                    });
                }
                let native = state.native_handle.clone().ok_or_else(|| {
                    AppError::Config("SSH transport is unavailable for SFTP".to_string())
                })?;
                state.sftp_generation = state.sftp_generation.saturating_add(1);
                (
                    native,
                    state.summary.connection_id.clone(),
                    state.sftp_generation,
                )
            };

            let channel = native
                .handle
                .lock()
                .await
                .channel_open_session()
                .await
                .map_err(|error| sftp_error("open SFTP channel", error))?;
            channel
                .request_subsystem(true, "sftp")
                .await
                .map_err(|error| sftp_error("request SFTP subsystem", error))?;
            let session = Arc::new(
                SftpSession::new(channel.into_stream())
                    .await
                    .map_err(|error| sftp_error("initialize SFTP", error))?,
            );
            session.set_timeout(20);
            let home_path = session
                .canonicalize(".")
                .await
                .ok()
                .and_then(|path| normalize_remote_path(&path).ok())
                .unwrap_or_else(|| "/".to_string());

            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
            let state =
                session_for_workspace_mut(&mut sessions, &input.workspace_id, &input.session_id)?;
            ensure_session_active(state)?;
            if state.sftp_generation != generation {
                // A concurrent open/reconnect won the generation race. If that
                // winner already installed a live channel, return it instead of
                // failing the caller (e.g. React Query / Strict Mode double open).
                if let Some(sftp) = &state.sftp {
                    return Ok(SftpOpenResult {
                        workspace_id: input.workspace_id,
                        session_id: input.session_id,
                        connection_id: state.summary.connection_id.clone(),
                        home_path: sftp.home_path.clone(),
                    });
                }
                return Err(AppError::Config(
                    "SFTP initialization was superseded by a newer SSH channel".to_string(),
                ));
            }
            state.sftp = Some(SftpChannelState {
                session,
                home_path: home_path.clone(),
                generation,
            });
            Ok(SftpOpenResult {
                workspace_id: input.workspace_id,
                session_id: input.session_id,
                connection_id,
                home_path,
            })
        }
    }

    pub async fn sftp_list_directory(
        &self,
        input: SftpPathInput,
    ) -> AppResult<SftpDirectoryListing> {
        let path = normalize_remote_path(&input.path)?;

        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (&input, &path);
            return Err(AppError::Config(
                "SFTP requires the native SSH transport".to_string(),
            ));
        }

        #[cfg(feature = "ssh-native")]
        {
            let (sftp, connection_id, generation) = self.sftp_context(&input).await?;
            let canonical_path = sftp
                .canonicalize(path)
                .await
                .map_err(|error| sftp_error("resolve remote directory", error))?;
            let canonical_path = normalize_remote_path(&canonical_path)?;
            let read_dir = sftp
                .read_dir(canonical_path.clone())
                .await
                .map_err(|error| sftp_error("list remote directory", error))?;
            let mut entries = Vec::new();
            for entry in read_dir {
                let metadata = entry.metadata();
                let kind = file_kind(metadata.file_type());
                let entry_path = normalize_remote_path(&entry.path())?;
                let link_target = if kind == "symlink" {
                    sftp.read_link(entry_path.clone()).await.ok()
                } else {
                    None
                };
                entries.push(SftpFileEntry {
                    name: entry.file_name(),
                    path: entry_path,
                    kind: kind.to_string(),
                    size: metadata.len(),
                    modified_at: metadata
                        .modified()
                        .ok()
                        .map(|time| chrono::DateTime::<Utc>::from(time).to_rfc3339()),
                    permissions: metadata
                        .permissions
                        .map(|_| metadata.permissions().to_string()),
                    link_target,
                });
            }
            entries.sort_by(|left, right| {
                file_kind_rank(&left.kind)
                    .cmp(&file_kind_rank(&right.kind))
                    .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                    .then_with(|| left.name.cmp(&right.name))
            });
            self.ensure_sftp_generation(&input.session_id, generation)?;
            Ok(SftpDirectoryListing {
                workspace_id: input.workspace_id,
                session_id: input.session_id,
                connection_id,
                path: canonical_path,
                entries,
            })
        }
    }

    pub async fn sftp_stat(&self, input: SftpPathInput) -> AppResult<SftpFileEntry> {
        let path = normalize_remote_path(&input.path)?;
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (&input, &path);
            return Err(AppError::Config(
                "SFTP requires the native SSH transport".to_string(),
            ));
        }
        #[cfg(feature = "ssh-native")]
        {
            let (sftp, _, generation) = self.sftp_context(&input).await?;
            let metadata = sftp
                .symlink_metadata(path.clone())
                .await
                .map_err(|error| sftp_error("stat remote path", error))?;
            let kind = file_kind(metadata.file_type()).to_string();
            let link_target = if kind == "symlink" {
                sftp.read_link(path.clone()).await.ok()
            } else {
                None
            };
            self.ensure_sftp_generation(&input.session_id, generation)?;
            Ok(SftpFileEntry {
                name: remote_file_name(&path),
                path,
                kind,
                size: metadata.len(),
                modified_at: metadata
                    .modified()
                    .ok()
                    .map(|time| chrono::DateTime::<Utc>::from(time).to_rfc3339()),
                permissions: metadata
                    .permissions
                    .map(|_| metadata.permissions().to_string()),
                link_target,
            })
        }
    }

    pub async fn sftp_create_directory(&self, input: SftpPathInput) -> AppResult<()> {
        let path = normalize_mutation_path(&input.path)?;
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (input, path);
            return Err(AppError::Config(
                "SFTP requires the native SSH transport".to_string(),
            ));
        }
        #[cfg(feature = "ssh-native")]
        self.with_sftp_path(input, "create remote directory", |sftp| async move {
            sftp.create_dir(path).await
        })
        .await
    }

    pub async fn sftp_rename(&self, input: SftpRenameInput) -> AppResult<()> {
        let old_path = normalize_mutation_path(&input.old_path)?;
        let new_path = normalize_mutation_path(&input.new_path)?;
        let path_input = SftpPathInput {
            workspace_id: input.workspace_id,
            session_id: input.session_id,
            path: old_path.clone(),
        };
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (path_input, old_path, new_path);
            return Err(AppError::Config(
                "SFTP requires the native SSH transport".to_string(),
            ));
        }
        #[cfg(feature = "ssh-native")]
        self.with_sftp_path(path_input, "rename remote path", |sftp| async move {
            sftp.rename(old_path, new_path).await
        })
        .await
    }

    pub async fn sftp_delete(&self, input: SftpDeleteInput) -> AppResult<()> {
        let path = normalize_mutation_path(&input.path)?;
        let is_directory = input.is_directory;
        let path_input = SftpPathInput {
            workspace_id: input.workspace_id,
            session_id: input.session_id,
            path: path.clone(),
        };
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (path_input, path, is_directory);
            return Err(AppError::Config(
                "SFTP requires the native SSH transport".to_string(),
            ));
        }
        #[cfg(feature = "ssh-native")]
        self.with_sftp_path(path_input, "delete remote path", |sftp| async move {
            if is_directory {
                sftp.remove_dir(path).await
            } else {
                sftp.remove_file(path).await
            }
        })
        .await
    }

    pub async fn sftp_download(&self, input: SftpTransferInput) -> AppResult<SftpTransferState> {
        self.start_transfer(input, "download").await
    }

    pub async fn sftp_upload(&self, input: SftpTransferInput) -> AppResult<SftpTransferState> {
        self.start_transfer(input, "upload").await
    }

    pub async fn sftp_cancel_transfer(
        &self,
        input: SftpCancelTransferInput,
    ) -> AppResult<SftpTransferState> {
        validate_workspace_id(&input.workspace_id)?;
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = input;
            return Err(AppError::Config(
                "SFTP requires the native SSH transport".to_string(),
            ));
        }
        #[cfg(feature = "ssh-native")]
        {
            let transfers = self
                .transfers
                .lock()
                .map_err(|_| AppError::Config("SFTP transfer lock poisoned".to_string()))?;
            let transfer = transfers
                .get(&input.transfer_id)
                .filter(|transfer| transfer.state.workspace_id == input.workspace_id)
                .ok_or_else(|| AppError::NotFound("SFTP transfer".to_string()))?;
            if matches!(transfer.state.status.as_str(), "pending" | "running") {
                let _ = transfer.cancel_tx.send(true);
            }
            Ok(transfer.state.clone())
        }
    }

    pub async fn sftp_list_transfers(
        &self,
        input: SftpSessionInput,
    ) -> AppResult<Vec<SftpTransferState>> {
        validate_workspace_id(&input.workspace_id)?;
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = input;
            return Ok(Vec::new());
        }
        #[cfg(feature = "ssh-native")]
        {
            let mut states = self
                .transfers
                .lock()
                .map_err(|_| AppError::Config("SFTP transfer lock poisoned".to_string()))?
                .values()
                .filter(|runtime| {
                    runtime.state.workspace_id == input.workspace_id
                        && runtime.state.session_id == input.session_id
                })
                .map(|runtime| runtime.state.clone())
                .collect::<Vec<_>>();
            states.sort_by(|left, right| right.started_at.cmp(&left.started_at));
            Ok(states)
        }
    }

    #[cfg(feature = "ssh-native")]
    async fn sftp_context(
        &self,
        input: &SftpPathInput,
    ) -> AppResult<(Arc<SftpSession>, String, u64)> {
        validate_workspace_id(&input.workspace_id)?;
        validate_session_id(&input.session_id)?;
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| AppError::Config("ssh session lock poisoned".to_string()))?;
        let state = session_for_workspace(&sessions, &input.workspace_id, &input.session_id)?;
        ensure_session_active(state)?;
        let sftp = state.sftp.as_ref().ok_or_else(|| {
            AppError::Validation("SFTP channel has not been initialized".to_string())
        })?;
        Ok((
            sftp.session.clone(),
            state.summary.connection_id.clone(),
            sftp.generation,
        ))
    }

    #[cfg(feature = "ssh-native")]
    fn ensure_sftp_generation(&self, session_id: &str, generation: u64) -> AppResult<()> {
        let current = self
            .sessions
            .lock()
            .ok()
            .map(|sessions| {
                sessions
                    .get(session_id)
                    .and_then(|state| state.sftp.as_ref())
                    .map(|sftp| sftp.generation == generation)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if current {
            Ok(())
        } else {
            Err(AppError::Config(
                "SFTP request belongs to a stale SSH channel".to_string(),
            ))
        }
    }

    #[cfg(feature = "ssh-native")]
    async fn with_sftp_path<F, Fut>(
        &self,
        input: SftpPathInput,
        operation: &'static str,
        action: F,
    ) -> AppResult<()>
    where
        F: FnOnce(Arc<SftpSession>) -> Fut,
        Fut: std::future::Future<Output = Result<(), russh_sftp::client::error::Error>>,
    {
        let (sftp, _, generation) = self.sftp_context(&input).await?;
        action(sftp)
            .await
            .map_err(|error| sftp_error(operation, error))?;
        self.ensure_sftp_generation(&input.session_id, generation)
    }

    async fn start_transfer(
        &self,
        input: SftpTransferInput,
        direction: &'static str,
    ) -> AppResult<SftpTransferState> {
        let remote_path = normalize_mutation_path(&input.remote_path)?;
        if input.local_path.trim().is_empty() || input.local_path.contains('\0') {
            return Err(AppError::Validation(
                "local transfer path is invalid".to_string(),
            ));
        }
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = (input, direction, remote_path);
            return Err(AppError::Config(
                "SFTP requires the native SSH transport".to_string(),
            ));
        }
        #[cfg(feature = "ssh-native")]
        {
            let path_input = SftpPathInput {
                workspace_id: input.workspace_id.clone(),
                session_id: input.session_id.clone(),
                path: remote_path.clone(),
            };
            let (sftp, connection_id, generation) = self.sftp_context(&path_input).await?;
            let transfer_id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            let state = SftpTransferState {
                transfer_id: transfer_id.clone(),
                workspace_id: input.workspace_id.clone(),
                session_id: input.session_id.clone(),
                connection_id: connection_id.clone(),
                direction: direction.to_string(),
                local_path: input.local_path.clone(),
                remote_path: remote_path.clone(),
                transferred_bytes: 0,
                total_bytes: 0,
                bytes_per_second: 0,
                status: "pending".to_string(),
                error: None,
                started_at: now,
                finished_at: None,
            };
            let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
            {
                let mut transfers = self
                    .transfers
                    .lock()
                    .map_err(|_| AppError::Config("SFTP transfer lock poisoned".to_string()))?;
                if transfers.values().any(|runtime| {
                    runtime.state.connection_id == connection_id
                        && matches!(runtime.state.status.as_str(), "pending" | "running")
                }) {
                    return Err(AppError::Validation(
                        "only one SFTP transfer can run per SSH connection".to_string(),
                    ));
                }
                transfers.insert(
                    transfer_id.clone(),
                    SftpTransferRuntime {
                        state: state.clone(),
                        cancel_tx,
                    },
                );
            }
            self.emit_sftp_transfer(&state);

            let service = self.clone();
            let local_path = input.local_path;
            let overwrite = input.overwrite;
            let session_id = input.session_id;
            tokio::spawn(async move {
                service.update_transfer(&transfer_id, |state| {
                    state.status = "running".to_string();
                });
                let result = if direction == "download" {
                    service
                        .run_download(
                            &transfer_id,
                            &session_id,
                            generation,
                            sftp,
                            &remote_path,
                            &local_path,
                            overwrite,
                            cancel_rx,
                        )
                        .await
                } else {
                    service
                        .run_upload(
                            &transfer_id,
                            &session_id,
                            generation,
                            sftp,
                            &local_path,
                            &remote_path,
                            overwrite,
                            cancel_rx,
                        )
                        .await
                };
                service.finish_transfer(&transfer_id, result);
            });
            Ok(state)
        }
    }

    #[cfg(feature = "ssh-native")]
    #[allow(clippy::too_many_arguments)]
    async fn run_download(
        &self,
        transfer_id: &str,
        session_id: &str,
        generation: u64,
        sftp: Arc<SftpSession>,
        remote_path: &str,
        local_path: &str,
        overwrite: bool,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), TransferRunError> {
        let target = std::path::PathBuf::from(local_path);
        if target.exists() && !overwrite {
            return Err(TransferRunError::Failed(
                "local target already exists".to_string(),
            ));
        }
        let part = download_part_path(&target);
        let result = async {
            let mut remote = sftp
                .open(remote_path.to_string())
                .await
                .map_err(transfer_sftp_error)?;
            let total = remote.metadata().await.map_err(transfer_sftp_error)?.len();
            self.update_transfer(transfer_id, |state| state.total_bytes = total);
            let mut local = tokio::fs::File::create(&part)
                .await
                .map_err(|error| TransferRunError::Failed(error.to_string()))?;
            let mut buffer = vec![0_u8; TRANSFER_BUFFER_SIZE];
            let started = std::time::Instant::now();
            let mut transferred = 0_u64;
            loop {
                ensure_not_cancelled(&cancel_rx)?;
                let read = tokio::select! {
                    changed = cancel_rx.changed() => {
                        let _ = changed;
                        return Err(TransferRunError::Cancelled);
                    }
                    result = remote.read(&mut buffer) => result,
                }
                .map_err(|error| TransferRunError::Failed(error.to_string()))?;
                if read == 0 {
                    break;
                }
                tokio::select! {
                    changed = cancel_rx.changed() => {
                        let _ = changed;
                        return Err(TransferRunError::Cancelled);
                    }
                    result = local.write_all(&buffer[..read]) => {
                        result.map_err(|error| TransferRunError::Failed(error.to_string()))?;
                    }
                }
                transferred = transferred.saturating_add(read as u64);
                self.report_transfer_progress(transfer_id, transferred, total, started);
                self.ensure_sftp_generation(session_id, generation)
                    .map_err(|error| TransferRunError::Failed(error.to_string()))?;
            }
            local
                .flush()
                .await
                .map_err(|error| TransferRunError::Failed(error.to_string()))?;
            local
                .sync_all()
                .await
                .map_err(|error| TransferRunError::Failed(error.to_string()))?;
            drop(local);
            let _ = remote.shutdown().await;
            replace_local_file(&part, &target, transfer_id, overwrite).await?;
            Ok(())
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&part).await;
        }
        result
    }

    #[cfg(feature = "ssh-native")]
    #[allow(clippy::too_many_arguments)]
    async fn run_upload(
        &self,
        transfer_id: &str,
        session_id: &str,
        generation: u64,
        sftp: Arc<SftpSession>,
        local_path: &str,
        remote_path: &str,
        overwrite: bool,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), TransferRunError> {
        let mut local = tokio::fs::File::open(local_path)
            .await
            .map_err(|error| TransferRunError::Failed(error.to_string()))?;
        let total = local
            .metadata()
            .await
            .map_err(|error| TransferRunError::Failed(error.to_string()))?
            .len();
        let target_exists = sftp
            .try_exists(remote_path.to_string())
            .await
            .map_err(transfer_sftp_error)?;
        if target_exists && !overwrite {
            return Err(TransferRunError::Failed(
                "remote target already exists".to_string(),
            ));
        }
        let temp_path = upload_temp_path(remote_path);
        if sftp.try_exists(temp_path.clone()).await.unwrap_or(false) {
            let _ = sftp.remove_file(temp_path.clone()).await;
        }
        let mut remote = sftp
            .create(temp_path.clone())
            .await
            .map_err(transfer_sftp_error)?;
        self.update_transfer(transfer_id, |state| state.total_bytes = total);
        let started = std::time::Instant::now();
        let mut transferred = 0_u64;
        let mut buffer = vec![0_u8; TRANSFER_BUFFER_SIZE];
        let copy_result = async {
            loop {
                ensure_not_cancelled(&cancel_rx)?;
                let read = tokio::select! {
                    changed = cancel_rx.changed() => {
                        let _ = changed;
                        return Err(TransferRunError::Cancelled);
                    }
                    result = local.read(&mut buffer) => result,
                }
                .map_err(|error| TransferRunError::Failed(error.to_string()))?;
                if read == 0 {
                    break;
                }
                tokio::select! {
                    changed = cancel_rx.changed() => {
                        let _ = changed;
                        return Err(TransferRunError::Cancelled);
                    }
                    result = remote.write_all(&buffer[..read]) => {
                        result.map_err(|error| TransferRunError::Failed(error.to_string()))?;
                    }
                }
                transferred = transferred.saturating_add(read as u64);
                self.report_transfer_progress(transfer_id, transferred, total, started);
                self.ensure_sftp_generation(session_id, generation)
                    .map_err(|error| TransferRunError::Failed(error.to_string()))?;
            }
            remote
                .flush()
                .await
                .map_err(|error| TransferRunError::Failed(error.to_string()))?;
            remote
                .shutdown()
                .await
                .map_err(|error| TransferRunError::Failed(error.to_string()))?;
            finalize_remote_upload(&sftp, &temp_path, remote_path, transfer_id, target_exists).await
        }
        .await;
        if copy_result.is_err() {
            let _ = sftp.remove_file(temp_path).await;
        }
        copy_result
    }

    #[cfg(feature = "ssh-native")]
    fn report_transfer_progress(
        &self,
        transfer_id: &str,
        transferred: u64,
        total: u64,
        started: std::time::Instant,
    ) {
        let elapsed = started.elapsed().as_secs_f64();
        self.update_transfer(transfer_id, |state| {
            state.transferred_bytes = transferred;
            state.total_bytes = total;
            state.bytes_per_second = transfer_speed(transferred, elapsed);
        });
    }

    #[cfg(feature = "ssh-native")]
    fn update_transfer(&self, transfer_id: &str, update: impl FnOnce(&mut SftpTransferState)) {
        let state = self.transfers.lock().ok().and_then(|mut transfers| {
            transfers.get_mut(transfer_id).map(|runtime| {
                update(&mut runtime.state);
                runtime.state.clone()
            })
        });
        if let Some(state) = state {
            self.emit_sftp_transfer(&state);
        }
    }

    #[cfg(feature = "ssh-native")]
    fn finish_transfer(&self, transfer_id: &str, result: Result<(), TransferRunError>) {
        self.update_transfer(transfer_id, |state| {
            state.finished_at = Some(Utc::now().to_rfc3339());
            match result {
                Ok(()) => {
                    state.status = "success".to_string();
                    state.transferred_bytes = state.total_bytes;
                    state.error = None;
                }
                Err(TransferRunError::Cancelled) => {
                    state.status = "cancelled".to_string();
                    state.error = None;
                }
                Err(TransferRunError::Failed(error)) => {
                    state.status = "failed".to_string();
                    state.error = Some(error);
                }
            }
        });
        self.prune_finished_transfers();
    }

    #[cfg(feature = "ssh-native")]
    fn prune_finished_transfers(&self) {
        let Ok(mut transfers) = self.transfers.lock() else {
            return;
        };
        let finished = transfers
            .iter()
            .filter(|(_, runtime)| {
                !matches!(runtime.state.status.as_str(), "pending" | "running")
            })
            .map(|(id, runtime)| (id.clone(), runtime.state.started_at.clone()))
            .collect::<Vec<_>>();
        for id in finished_transfer_ids_to_prune(finished, MAX_FINISHED_SFTP_TRANSFERS) {
            transfers.remove(&id);
        }
    }

    #[cfg(feature = "ssh-native")]
    fn emit_sftp_transfer(&self, state: &SftpTransferState) {
        let callback = self
            .on_sftp_transfer
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        if let Some(callback) = callback {
            if let Ok(payload) = serde_json::to_string(state) {
                callback(payload);
            }
        }
    }

    #[cfg(feature = "ssh-native")]
    pub(super) fn cancel_sftp_transfers_for_session(&self, session_id: &str) {
        if let Ok(transfers) = self.transfers.lock() {
            for runtime in transfers.values() {
                if runtime.state.session_id == session_id
                    && matches!(runtime.state.status.as_str(), "pending" | "running")
                {
                    let _ = runtime.cancel_tx.send(true);
                }
            }
        }
    }
}
