use super::*;
use unfour_core::models::{SshTaskInputSnapshot, SshTaskInputSource, SshTaskRunContext};
use unfour_core::redaction::{
    is_sensitive_flow_name, redact_connection_string, redact_shell_command,
};

pub(super) fn sensitive_input(name: &str, value: &str) -> bool {
    is_sensitive_flow_name(name)
        || redact_shell_command(value).1
        || redact_connection_string(value) != value
        || value.contains("-----BEGIN ")
        || value.to_ascii_lowercase().starts_with("bearer ")
        || value.to_ascii_lowercase().starts_with("basic ")
}

pub(super) fn run_context(
    input: &SshTaskRunInput,
    detail: &SshTaskDetail,
    secrets: &[String],
) -> AppResult<Option<SshTaskRunContext>> {
    let Some(context) = &input.context else {
        return Ok(None);
    };
    if context.task_version != task_version(detail) {
        return Err(AppError::Validation("SSH_TASK_DEFINITION_CHANGED".into()));
    }
    let names = SshService::detected_task_inputs(&detail.steps)?;
    let inputs = names
        .into_iter()
        .map(|name| {
            let source = context
                .input_sources
                .get(&name)
                .cloned()
                .unwrap_or(SshTaskInputSource::Manual);
            let secret = input.secret_input_names.contains(&name)
                || input.inputs.get(&name).is_some_and(|value| {
                    sensitive_input(&name, value)
                        || secrets
                            .iter()
                            .any(|secret| !secret.is_empty() && value.contains(secret))
                });
            let value = input
                .inputs
                .get(&name)
                .filter(|_| {
                    source == SshTaskInputSource::Manual
                        && context.input_sources.contains_key(&name)
                        && !secret
                })
                .cloned();
            (
                name,
                SshTaskInputSnapshot {
                    source,
                    secret,
                    value,
                },
            )
        })
        .collect();
    Ok(Some(SshTaskRunContext {
        version: 1,
        environment_id: context.environment_id.clone(),
        task_version: task_version(detail),
        environment_updated_at: context.environment_updated_at.clone(),
        inputs,
    }))
}

pub(super) fn task_version(detail: &SshTaskDetail) -> String {
    std::iter::once(detail.task.updated_at.clone())
        .chain(
            detail
                .steps
                .iter()
                .map(|step| format!("{}:{}", step.id, step.updated_at)),
        )
        .collect::<Vec<_>>()
        .join("|")
}
