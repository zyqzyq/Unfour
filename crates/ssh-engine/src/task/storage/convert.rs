use super::super::*;
use std::path::{Path, PathBuf};

#[derive(sqlx::FromRow)]
pub(super) struct StoredTask {
    id: String,
    workspace_id: String,
    name: String,
    description: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(sqlx::FromRow)]
pub(super) struct StoredStep {
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
pub(super) struct StoredBinding {
    task_id: String,
    workspace_id: String,
    default_connection_id: Option<String>,
    last_used_connection_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
pub(super) struct StoredRun {
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

pub(super) fn remove_task_logs(paths: Vec<PathBuf>, allowed_root: &Path) -> usize {
    paths
        .into_iter()
        .filter(|path| safe_task_log_path(path, Some(allowed_root)))
        .filter(|path| std::fs::remove_file(path).is_ok())
        .count()
}

pub(super) fn safe_task_log_path(path: &Path, allowed_root: Option<&Path>) -> bool {
    let Some(root) = allowed_root else {
        return false;
    };
    path.starts_with(root) && path.extension().is_some_and(|value| value == "log")
}

pub(super) fn task_from_row(row: StoredTask) -> SshTask {
    SshTask {
        id: row.id,
        workspace_id: row.workspace_id,
        name: row.name,
        description: row.description,
        sort_order: row.sort_order,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
    }
}

pub(super) fn step_from_row(row: StoredStep) -> AppResult<SshTaskStep> {
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

pub(super) fn binding_from_row(row: StoredBinding) -> SshTaskLocalBinding {
    SshTaskLocalBinding {
        task_id: row.task_id,
        workspace_id: row.workspace_id,
        default_connection_id: row.default_connection_id,
        last_used_connection_id: row.last_used_connection_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub(super) fn run_from_row(row: StoredRun) -> SshTaskRun {
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

pub(super) fn validate_task_id(value: &str) -> AppResult<()> {
    if value.trim().is_empty() || value.chars().count() > 128 {
        return Err(AppError::Validation("invalid SSH task id".to_string()));
    }
    Ok(())
}

pub(super) fn normalize_task_name(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 128 {
        return Err(AppError::Validation(
            "SSH task name must be between 1 and 128 characters".to_string(),
        ));
    }
    Ok(value.to_string())
}

pub(super) fn normalize_step_name(
    value: &str,
    step_type: &str,
    position: usize,
) -> AppResult<String> {
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
