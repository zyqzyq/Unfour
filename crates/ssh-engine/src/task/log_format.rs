pub(super) const HEADER: &str = "Unfour SSH task log v2\n";
const STEP: &str = "@step ";
const PARTIAL: &str = "@partial ";
const DIAGNOSTIC: &str = "@diagnostic ";

/// Navigation records are generated from trusted step events. Escape reserved
/// lines in every readable event, including multiline stdout and errors.
#[cfg(any(feature = "ssh-native", test))]
pub(super) fn encode_event(event: &super::SshTaskRunEvent) -> String {
    let mut encoded = String::new();
    if event.kind == "step" {
        let marker = serde_json::json!({
            "id": event.step_id,
            "name": event.step_name,
            "position": event.position,
            "status": if event.error.as_deref() == Some(super::SSH_TASK_OUTCOME_UNCERTAIN) {
                Some("outcomeUncertain")
            } else {
                event.status.as_deref()
            },
        });
        encoded.push_str(STEP);
        encoded.push_str(&marker.to_string());
        encoded.push('\n');
    }
    for (index, line) in super::events::format_task_log_event(event)
        .split_inclusive('\n')
        .enumerate()
    {
        if !line.ends_with('\n') {
            // Frame output chunks without a final newline while preserving the
            // original text that Flow and MCP receive.
            encoded.push_str(PARTIAL);
            encoded.push_str(line);
            encoded.push('\n');
        } else {
            if index == 0 && event.kind == "output" && event.stream.as_deref() == Some("diagnostic")
            {
                encoded.push_str(DIAGNOSTIC);
            } else if line.starts_with('@') {
                encoded.push('@');
            }
            encoded.push_str(line);
        }
    }
    encoded
}

pub(super) fn plain_text(log: String) -> String {
    let Some(body) = log.strip_prefix(HEADER) else {
        return log;
    };
    let mut plain = String::with_capacity(body.len());
    for line in body.split_inclusive('\n') {
        if line.starts_with(STEP) {
            continue;
        }
        if let Some(partial) = line.strip_prefix(PARTIAL) {
            plain.push_str(partial.strip_suffix('\n').unwrap_or(partial));
        } else if let Some(diagnostic) = line.strip_prefix(DIAGNOSTIC) {
            plain.push_str(diagnostic);
        } else {
            plain.push_str(if line.starts_with("@@") {
                &line[1..]
            } else {
                line
            });
        }
    }
    plain
}
