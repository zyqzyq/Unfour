use super::*;
use sqlx::Row;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[cfg(feature = "ssh-native")]
const TASK_RETENTION_DAYS: i64 = 30;
#[cfg(feature = "ssh-native")]
const MAX_RUNS_PER_TASK: usize = 100;

#[derive(sqlx::FromRow)]
struct StoredTask {
    id: String,
    workspace_id: String,
    name: String,
    description: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(sqlx::FromRow)]
struct StoredStep {
    id: String,
    workspace_id: String,
    task_id: String,
    name: String,
    step_type: String,
    position: i64,
    enabled: i64,
    config_version: i64,
    config_json: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(sqlx::FromRow)]
struct StoredBinding {
    task_id: String,
    workspace_id: String,
    default_connection_id: Option<String>,
    last_used_connection_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
struct StoredRun {
    id: String,
    workspace_id: String,
    task_id: String,
    connection_id: Option<String>,
    status: String,
    started_at: String,
    finished_at: Option<String>,
    error_message: Option<String>,
    log_path: String,
}

impl SshService {
    pub async fn list_tasks(&self, workspace_id: String) -> AppResult<Vec<SshTask>> {
        validate_workspace_id(&workspace_id)?;
        let rows = sqlx::query_as::<_, StoredTask>(
            r#"
            SELECT id, workspace_id, name, description, created_at, updated_at, deleted_at
            FROM ssh_task
            WHERE workspace_id = ?1 AND deleted_at IS NULL
            ORDER BY updated_at DESC, name COLLATE NOCASE, id
            "#,
        )
        .bind(&workspace_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(task_from_row).collect())
    }

    pub async fn get_task(&self, workspace_id: &str, task_id: &str) -> AppResult<SshTaskDetail> {
        validate_workspace_id(workspace_id)?;
        validate_task_id(task_id)?;
        let task = sqlx::query_as::<_, StoredTask>(
            r#"
            SELECT id, workspace_id, name, description, created_at, updated_at, deleted_at
            FROM ssh_task
            WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(task_id)
        .fetch_optional(self.db.pool())
        .await?
        .map(task_from_row)
        .ok_or_else(|| AppError::NotFound("SSH task".to_string()))?;
        let rows = sqlx::query_as::<_, StoredStep>(
            r#"
            SELECT id, workspace_id, task_id, name, step_type, position, enabled,
                   config_version, config_json, created_at, updated_at, deleted_at
            FROM ssh_task_step
            WHERE workspace_id = ?1 AND task_id = ?2 AND deleted_at IS NULL
            ORDER BY position, id
            "#,
        )
        .bind(workspace_id)
        .bind(task_id)
        .fetch_all(self.db.pool())
        .await?;
        let steps = rows
            .into_iter()
            .map(step_from_row)
            .collect::<AppResult<Vec<_>>>()?;
        let local_binding = sqlx::query_as::<_, StoredBinding>(
            r#"
            SELECT task_id, workspace_id, default_connection_id, last_used_connection_id,
                   created_at, updated_at
            FROM ssh_task_local_binding
            WHERE workspace_id = ?1 AND task_id = ?2
            "#,
        )
        .bind(workspace_id)
        .bind(task_id)
        .fetch_optional(self.db.pool())
        .await?
        .map(binding_from_row);
        Ok(SshTaskDetail {
            task,
            steps,
            local_binding,
        })
    }

    pub async fn save_task(&self, mut input: SshTaskSaveInput) -> AppResult<SshTaskDetail> {
        validate_workspace_id(&input.workspace_id)?;
        let name = normalize_task_name(&input.name)?;
        if input.description.chars().count() > 2_000 {
            return Err(AppError::Validation(
                "SSH task description must be 2000 characters or fewer".to_string(),
            ));
        }
        if input.steps.len() > 100 {
            return Err(AppError::Validation(
                "SSH task cannot contain more than 100 steps".to_string(),
            ));
        }
        if let Some(connection_id) = input.default_connection_id.as_deref() {
            self.get_connection(&input.workspace_id, connection_id)
                .await?;
        }
        input.steps.sort_by_key(|step| step.position);
        for (position, step) in input.steps.iter_mut().enumerate() {
            step.position = position as i64;
            step.name = normalize_step_name(&step.name, &step.step_type, position)?;
        }

        let now = Utc::now().to_rfc3339();
        let id = input.id.clone().unwrap_or_else(unfour_core::id::new_id);
        validate_task_id(&id)?;
        let existing = sqlx::query(
            "SELECT created_at FROM ssh_task WHERE workspace_id = ?1 AND id = ?2 AND deleted_at IS NULL",
        )
                .bind(&input.workspace_id)
                .bind(&id)
                .fetch_optional(self.db.pool())
                .await?;
        if input.id.is_some() && existing.is_none() {
            return Err(AppError::NotFound("SSH task".to_string()));
        }
        let created_at = existing
            .as_ref()
            .map(|row| row.get::<String, _>("created_at"))
            .unwrap_or_else(|| now.clone());

        let existing_steps = sqlx::query(
            "SELECT id, created_at, config_version FROM ssh_task_step WHERE workspace_id = ?1 AND task_id = ?2",
        )
        .bind(&input.workspace_id)
        .bind(&id)
        .fetch_all(self.db.pool())
        .await?
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("id"),
                (
                    row.get::<String, _>("created_at"),
                    row.get::<i64, _>("config_version"),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
        let had_binding: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ssh_task_local_binding WHERE workspace_id = ?1 AND task_id = ?2)",
        )
        .bind(&input.workspace_id)
        .bind(&id)
        .fetch_one(self.db.pool())
        .await?;

        for step in &mut input.steps {
            let step_id = step.id.get_or_insert_with(unfour_core::id::new_id).clone();
            validate_task_id(&step_id)?;
            let config_version = match existing_steps.get(&step_id) {
                Some((_, stored_version)) => {
                    if step
                        .config_version
                        .is_some_and(|requested| requested != *stored_version)
                    {
                        return Err(AppError::Validation(format!(
                            "SSH task step config version cannot be changed by a normal update (stored {stored_version})"
                        )));
                    }
                    *stored_version
                }
                None => step.config_version.unwrap_or(1),
            };
            validate_step_config(&step.step_type, config_version, &step.config_json)?;
            step.config_version = Some(config_version);
        }

        let mut transaction = self.db.pool().begin().await?;
        if existing.is_some() {
            sqlx::query(
                r#"
                UPDATE ssh_task
                SET name = ?1, description = ?2, updated_at = ?3
                WHERE workspace_id = ?4 AND id = ?5 AND deleted_at IS NULL
                "#,
            )
            .bind(&name)
            .bind(input.description.trim())
            .bind(&now)
            .bind(&input.workspace_id)
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO ssh_task (
                  id, workspace_id, name, description, created_at, updated_at, deleted_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)
                "#,
            )
            .bind(&id)
            .bind(&input.workspace_id)
            .bind(&name)
            .bind(input.description.trim())
            .bind(&created_at)
            .bind(&now)
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query(
            r#"
            UPDATE ssh_task_step
            SET deleted_at = ?1, updated_at = ?1
            WHERE workspace_id = ?2 AND task_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&input.workspace_id)
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
        for step in input.steps {
            let step_id = step.id.expect("step ids are assigned before persistence");
            let config_version = step
                .config_version
                .expect("config versions are assigned before persistence");
            if existing_steps.contains_key(&step_id) {
                sqlx::query(
                    r#"
                    UPDATE ssh_task_step
                    SET name = ?1, step_type = ?2, position = ?3, enabled = ?4,
                        config_json = ?5, updated_at = ?6, deleted_at = NULL
                    WHERE workspace_id = ?7 AND task_id = ?8 AND id = ?9
                    "#,
                )
                .bind(step.name)
                .bind(step.step_type)
                .bind(step.position)
                .bind(if step.enabled { 1_i64 } else { 0_i64 })
                .bind(serde_json::to_string(&step.config_json)?)
                .bind(&now)
                .bind(&input.workspace_id)
                .bind(&id)
                .bind(step_id)
                .execute(&mut *transaction)
                .await?;
            } else {
                sqlx::query(
                    r#"
                    INSERT INTO ssh_task_step (
                      id, workspace_id, task_id, name, step_type, position, enabled,
                      config_version, config_json, created_at, updated_at, deleted_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)
                    "#,
                )
                .bind(step_id)
                .bind(&input.workspace_id)
                .bind(&id)
                .bind(step.name)
                .bind(step.step_type)
                .bind(step.position)
                .bind(if step.enabled { 1_i64 } else { 0_i64 })
                .bind(config_version)
                .bind(serde_json::to_string(&step.config_json)?)
                .bind(&now)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
            }
        }
        if input.default_connection_id.is_some() || had_binding {
            sqlx::query(
                r#"
                INSERT INTO ssh_task_local_binding (
                  task_id, workspace_id, default_connection_id, last_used_connection_id,
                  created_at, updated_at
                ) VALUES (?1, ?2, ?3, NULL, ?4, ?4)
                ON CONFLICT(task_id) DO UPDATE SET
                  default_connection_id = excluded.default_connection_id,
                  updated_at = excluded.updated_at
                "#,
            )
            .bind(&id)
            .bind(&input.workspace_id)
            .bind(input.default_connection_id.as_deref())
            .bind(&now)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        self.get_task(&input.workspace_id, &id).await
    }

    pub async fn duplicate_task(
        &self,
        workspace_id: String,
        task_id: String,
    ) -> AppResult<SshTaskDetail> {
        let detail = self.get_task(&workspace_id, &task_id).await?;
        self.save_task(SshTaskSaveInput {
            id: None,
            workspace_id,
            name: format!("{} Copy", detail.task.name),
            description: detail.task.description,
            default_connection_id: detail
                .local_binding
                .and_then(|binding| binding.default_connection_id),
            steps: detail
                .steps
                .into_iter()
                .map(|step| SshTaskStepInput {
                    id: None,
                    name: step.name,
                    step_type: step.step_type,
                    position: step.position,
                    enabled: step.enabled,
                    config_version: Some(step.config_version),
                    config_json: step.config_json,
                })
                .collect(),
        })
        .await
    }

    pub async fn delete_task(&self, workspace_id: String, task_id: String) -> AppResult<()> {
        self.get_task(&workspace_id, &task_id).await?;
        let running: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2 AND status = 'running'",
        )
        .bind(&workspace_id)
        .bind(&task_id)
        .fetch_one(self.db.pool())
        .await?;
        if running > 0 {
            return Err(AppError::Validation(
                "cannot delete an SSH task while it is running".to_string(),
            ));
        }
        let log_paths = self.task_log_paths(&workspace_id, Some(&task_id)).await?;
        let now = Utc::now().to_rfc3339();
        let mut transaction = self.db.pool().begin().await?;
        sqlx::query(
            r#"
            UPDATE ssh_task_step
            SET deleted_at = ?1, updated_at = ?1
            WHERE workspace_id = ?2 AND task_id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&task_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM ssh_task_local_binding WHERE workspace_id = ?1 AND task_id = ?2")
            .bind(&workspace_id)
            .bind(&task_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2")
            .bind(&workspace_id)
            .bind(&task_id)
            .execute(&mut *transaction)
            .await?;
        let result = sqlx::query(
            r#"
            UPDATE ssh_task
            SET deleted_at = ?1, updated_at = ?1
            WHERE workspace_id = ?2 AND id = ?3 AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&workspace_id)
        .bind(&task_id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("SSH task".to_string()));
        }
        transaction.commit().await?;
        remove_task_logs(log_paths, &self.task_log_dir);
        Ok(())
    }

    pub async fn list_task_runs(
        &self,
        workspace_id: String,
        task_id: String,
    ) -> AppResult<Vec<SshTaskRun>> {
        self.get_task(&workspace_id, &task_id).await?;
        let rows = sqlx::query_as::<_, StoredRun>(
            r#"
            SELECT id, workspace_id, task_id, connection_id, status, started_at,
                   finished_at, error_message, log_path
            FROM ssh_task_run
            WHERE workspace_id = ?1 AND task_id = ?2
            ORDER BY started_at DESC, id DESC
            "#,
        )
        .bind(workspace_id)
        .bind(task_id)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(run_from_row).collect())
    }

    pub async fn clear_task_runs(
        &self,
        input: SshTaskCleanupInput,
    ) -> AppResult<SshTaskCleanupResult> {
        validate_workspace_id(&input.workspace_id)?;
        if let Some(task_id) = input.task_id.as_deref() {
            self.get_task(&input.workspace_id, task_id).await?;
        }
        let log_paths = self
            .task_log_paths(&input.workspace_id, input.task_id.as_deref())
            .await?;
        let result = if let Some(task_id) = input.task_id.as_deref() {
            sqlx::query(
                "DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2 AND status <> 'running'",
            )
                .bind(&input.workspace_id)
                .bind(task_id)
                .execute(self.db.pool())
                .await?
        } else {
            sqlx::query("DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND status <> 'running'")
                .bind(&input.workspace_id)
                .execute(self.db.pool())
                .await?
        };
        Ok(SshTaskCleanupResult {
            deleted_runs: result.rows_affected() as usize,
            deleted_logs: remove_task_logs(log_paths, &self.task_log_dir),
        })
    }

    #[cfg(any(feature = "ssh-native", test))]
    pub(super) async fn record_task_connection_use(
        &self,
        workspace_id: &str,
        task_id: &str,
        connection_id: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO ssh_task_local_binding (
              task_id, workspace_id, default_connection_id, last_used_connection_id,
              created_at, updated_at
            ) VALUES (?1, ?2, NULL, ?3, ?4, ?4)
            ON CONFLICT(task_id) DO UPDATE SET
              last_used_connection_id = excluded.last_used_connection_id,
              updated_at = excluded.updated_at
            "#,
        )
        .bind(task_id)
        .bind(workspace_id)
        .bind(connection_id)
        .bind(&now)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    #[cfg(feature = "ssh-native")]
    pub(super) async fn insert_task_run(&self, run: &SshTaskRun) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO ssh_task_run (
              id, workspace_id, task_id, connection_id, status, started_at,
              finished_at, error_message, log_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&run.id)
        .bind(&run.workspace_id)
        .bind(&run.task_id)
        .bind(run.connection_id.as_deref())
        .bind(&run.status)
        .bind(&run.started_at)
        .bind(run.finished_at.as_deref())
        .bind(run.error_message.as_deref())
        .bind(&run.log_path)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    #[cfg(feature = "ssh-native")]
    pub(super) async fn finish_task_run(
        &self,
        workspace_id: &str,
        run_id: &str,
        status: &str,
        error_message: Option<&str>,
    ) -> AppResult<SshTaskRun> {
        let finished_at = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE ssh_task_run
            SET status = ?1, finished_at = ?2, error_message = ?3
            WHERE workspace_id = ?4 AND id = ?5
            "#,
        )
        .bind(status)
        .bind(&finished_at)
        .bind(error_message)
        .bind(workspace_id)
        .bind(run_id)
        .execute(self.db.pool())
        .await?;
        self.get_task_run(workspace_id, run_id).await
    }

    #[cfg(feature = "ssh-native")]
    pub(super) async fn cleanup_task_retention(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> AppResult<SshTaskCleanupResult> {
        let runs = self
            .list_task_runs(workspace_id.to_string(), task_id.to_string())
            .await?;
        let cutoff = Utc::now() - chrono::Duration::days(TASK_RETENTION_DAYS);
        let mut completed_seen = 0_usize;
        let mut remove = Vec::new();
        for run in runs {
            if run.status == "running" {
                continue;
            }
            let older_than_cutoff = chrono::DateTime::parse_from_rfc3339(&run.started_at)
                .map(|started| started.with_timezone(&Utc) < cutoff)
                .unwrap_or(false);
            if older_than_cutoff || completed_seen >= MAX_RUNS_PER_TASK {
                remove.push(run);
            }
            completed_seen += 1;
        }
        let mut deleted_logs = 0;
        for run in &remove {
            sqlx::query("DELETE FROM ssh_task_run WHERE workspace_id = ?1 AND id = ?2")
                .bind(workspace_id)
                .bind(&run.id)
                .execute(self.db.pool())
                .await?;
            deleted_logs +=
                remove_task_logs(vec![PathBuf::from(&run.log_path)], &self.task_log_dir);
        }
        Ok(SshTaskCleanupResult {
            deleted_runs: remove.len(),
            deleted_logs,
        })
    }

    #[cfg(feature = "ssh-native")]
    pub(super) async fn get_task_run(
        &self,
        workspace_id: &str,
        run_id: &str,
    ) -> AppResult<SshTaskRun> {
        sqlx::query_as::<_, StoredRun>(
            r#"
            SELECT id, workspace_id, task_id, connection_id, status, started_at,
                   finished_at, error_message, log_path
            FROM ssh_task_run WHERE workspace_id = ?1 AND id = ?2
            "#,
        )
        .bind(workspace_id)
        .bind(run_id)
        .fetch_optional(self.db.pool())
        .await?
        .map(run_from_row)
        .ok_or_else(|| AppError::NotFound("SSH task run".to_string()))
    }

    async fn task_log_paths(
        &self,
        workspace_id: &str,
        task_id: Option<&str>,
    ) -> AppResult<Vec<PathBuf>> {
        let rows = if let Some(task_id) = task_id {
            sqlx::query(
                "SELECT log_path FROM ssh_task_run WHERE workspace_id = ?1 AND task_id = ?2 AND status <> 'running'",
            )
            .bind(workspace_id)
            .bind(task_id)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query(
                "SELECT log_path FROM ssh_task_run WHERE workspace_id = ?1 AND status <> 'running'",
            )
            .bind(workspace_id)
            .fetch_all(self.db.pool())
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|row| PathBuf::from(row.get::<String, _>("log_path")))
            .collect())
    }
}

#[cfg(feature = "ssh-native")]
impl SshService {
    #[cfg(feature = "ssh-native")]
    pub(super) fn task_log_path(&self, run_id: &str) -> AppResult<PathBuf> {
        std::fs::create_dir_all(&*self.task_log_dir).map_err(|error| {
            AppError::Config(format!("failed to create task log directory: {error}"))
        })?;
        Ok(self.task_log_dir.join(format!("{run_id}.log")))
    }
}

fn remove_task_logs(paths: Vec<PathBuf>, allowed_root: &Path) -> usize {
    paths
        .into_iter()
        .filter(|path| safe_task_log_path(path, Some(allowed_root)))
        .filter(|path| std::fs::remove_file(path).is_ok())
        .count()
}

fn safe_task_log_path(path: &Path, allowed_root: Option<&Path>) -> bool {
    let Some(root) = allowed_root else {
        return false;
    };
    path.starts_with(root) && path.extension().is_some_and(|value| value == "log")
}

fn task_from_row(row: StoredTask) -> SshTask {
    SshTask {
        id: row.id,
        workspace_id: row.workspace_id,
        name: row.name,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
    }
}

fn step_from_row(row: StoredStep) -> AppResult<SshTaskStep> {
    let config_json = serde_json::from_str(&row.config_json).map_err(|error| {
        AppError::Config(format!("stored SSH task step config is invalid: {error}"))
    })?;
    validate_step_config(&row.step_type, row.config_version, &config_json)?;
    Ok(SshTaskStep {
        id: row.id,
        workspace_id: row.workspace_id,
        task_id: row.task_id,
        name: row.name,
        step_type: row.step_type,
        position: row.position,
        enabled: row.enabled != 0,
        config_version: row.config_version,
        config_json,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
    })
}

fn binding_from_row(row: StoredBinding) -> SshTaskLocalBinding {
    SshTaskLocalBinding {
        task_id: row.task_id,
        workspace_id: row.workspace_id,
        default_connection_id: row.default_connection_id,
        last_used_connection_id: row.last_used_connection_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn run_from_row(row: StoredRun) -> SshTaskRun {
    SshTaskRun {
        id: row.id,
        workspace_id: row.workspace_id,
        task_id: row.task_id,
        connection_id: row.connection_id,
        status: row.status,
        started_at: row.started_at,
        finished_at: row.finished_at,
        error_message: row.error_message,
        log_path: row.log_path,
    }
}

fn validate_task_id(value: &str) -> AppResult<()> {
    if value.trim().is_empty() || value.chars().count() > 128 {
        return Err(AppError::Validation("invalid SSH task id".to_string()));
    }
    Ok(())
}

fn normalize_task_name(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 128 {
        return Err(AppError::Validation(
            "SSH task name must be between 1 and 128 characters".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn normalize_step_name(value: &str, step_type: &str, position: usize) -> AppResult<String> {
    let fallback = match step_type {
        "command" => "Command",
        "upload" => "Upload",
        "download" => "Download",
        _ => "Step",
    };
    let value = if value.trim().is_empty() {
        format!("{fallback} {}", position + 1)
    } else {
        value.trim().to_string()
    };
    if value.chars().count() > 128 {
        return Err(AppError::Validation(
            "SSH task step name must be 128 characters or fewer".to_string(),
        ));
    }
    Ok(value)
}
