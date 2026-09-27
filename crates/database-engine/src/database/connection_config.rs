use super::*;

#[derive(Debug)]
pub(super) struct DatabaseConnectionStorageInput {
    pub(super) driver: String,
    pub(super) host: Option<String>,
    pub(super) port: Option<u16>,
    pub(super) database_name: Option<String>,
    pub(super) username: Option<String>,
    pub(super) ssl_mode: Option<String>,
    pub(super) read_only: bool,
    pub(super) config: DatabaseConnectionConfig,
}

pub(super) fn input_to_storage(
    input: &DatabaseConnectionInput,
) -> AppResult<DatabaseConnectionStorageInput> {
    let driver = input.driver.trim().to_ascii_lowercase();
    if !matches!(driver.as_str(), "sqlite" | "postgres" | "mysql") {
        return Err(AppError::Validation(format!(
            "unsupported database driver: {}",
            input.driver
        )));
    }

    if driver == "sqlite" {
        let sqlite_path = input
            .sqlite_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::Validation("SQLite path is required".to_string()))?;

        return Ok(DatabaseConnectionStorageInput {
            driver,
            host: None,
            port: None,
            database_name: None,
            username: None,
            read_only: input.read_only,
            ssl_mode: None,
            config: DatabaseConnectionConfig {
                sqlite_path: Some(sqlite_path.to_string()),
                ..empty_database_config()
            },
        });
    }

    Ok(DatabaseConnectionStorageInput {
        driver,
        host: empty_to_none(input.host.clone()),
        port: input.port,
        database_name: empty_to_none(input.database.clone()),
        username: empty_to_none(input.username.clone()),
        read_only: input.read_only,
        ssl_mode: normalize_ssl_mode(input.ssl_mode.clone())?,
        config: empty_database_config(),
    })
}

pub(super) fn database_config_to_json(config: &DatabaseConnectionConfig) -> AppResult<String> {
    serde_json::to_string(config).map_err(AppError::from)
}

pub(super) fn parse_database_config(
    connection_id: &str,
    config_json: &str,
) -> AppResult<DatabaseConnectionConfig> {
    serde_json::from_str::<DatabaseConnectionConfig>(config_json).map_err(|error| {
        AppError::Config(format!(
            "invalid database_connections.config_json for connection {connection_id}: {error}"
        ))
    })
}

pub(super) fn normalize_ssl_mode(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = empty_to_none(value) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "disable" | "prefer" | "require" | "verify-ca" | "verify-full"
    ) {
        Ok(Some(normalized))
    } else {
        Err(AppError::Validation(format!(
            "unsupported database ssl mode: {value}"
        )))
    }
}

pub(super) fn decode_port(value: Option<i64>, label: &str) -> AppResult<Option<u16>> {
    match value {
        None => Ok(None),
        Some(port) if (1..=u16::MAX as i64).contains(&port) => Ok(Some(port as u16)),
        Some(port) => Err(AppError::Config(format!("{label} out of range: {port}"))),
    }
}

pub(super) fn normalize_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "database connection name cannot be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 80 {
        return Err(AppError::Validation(
            "database connection name must be 80 characters or fewer".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

pub(super) fn empty_to_none(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

/// Format: `<service>:<workspace_id>:<kind>:<record_id>`.
pub(super) fn validate_credential_ref_for_workspace(
    credential_ref: Option<&str>,
    workspace_id: &str,
) -> AppResult<()> {
    let Some(credential_ref) = credential_ref else {
        return Ok(());
    };
    let parsed_workspace = parse_credential_ref_workspace(credential_ref)?;
    if parsed_workspace != workspace_id {
        return Err(AppError::Validation(
            "credential reference does not belong to the workspace".to_string(),
        ));
    }
    Ok(())
}

fn parse_credential_ref_workspace(credential_ref: &str) -> AppResult<&str> {
    let mut parts = credential_ref.splitn(4, ':');
    let service_name = parts.next().unwrap_or_default();
    let workspace_id = parts.next().unwrap_or_default();
    let kind = parts.next().unwrap_or_default();
    let record_id = parts.next().unwrap_or_default();
    if service_name.is_empty() || workspace_id.is_empty() || kind.is_empty() || record_id.is_empty()
    {
        return Err(AppError::Validation(
            "credential reference is invalid".to_string(),
        ));
    }
    Ok(workspace_id)
}

pub(super) fn validate_workspace_id(workspace_id: &str) -> AppResult<()> {
    if workspace_id.trim().is_empty() {
        return Err(AppError::Validation(
            "workspace id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_connection_id(connection_id: &str) -> AppResult<()> {
    if connection_id.trim().is_empty() {
        return Err(AppError::Validation(
            "database connection id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn empty_database_config() -> DatabaseConnectionConfig {
    DatabaseConnectionConfig {
        sqlite_path: None,
        connect_timeout_ms: None,
        statement_timeout_ms: None,
        default_schema: None,
    }
}
