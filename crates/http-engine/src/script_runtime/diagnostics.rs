use unfour_core::redaction::{
    is_sensitive_flow_name, is_sensitive_workspace_variable, redact_json_body,
    redact_sensitive_lines, REDACTED_VALUE,
};

use super::*;

/// Execution-local provenance only. Never serialized or persisted. A host
/// collector retains setter values even after overwrite/unset or failed export.
pub(super) struct DiagnosticRedactions {
    values: Vec<String>,
    bytes: usize,
    complete: bool,
}

impl DiagnosticRedactions {
    pub(super) fn new(input: &ScriptRunInput) -> Self {
        let mut redactions = Self {
            values: Vec::new(),
            bytes: 0,
            complete: true,
        };
        for value in &input.redactions {
            redactions.record(value);
        }
        // Temporary values already use conservative provenance in HTTP history.
        for variable in &input.variables {
            redactions.record(&variable.value);
        }
        for variable in &input.environment {
            if is_sensitive_workspace_variable(&variable.key, &variable.value, false) {
                redactions.record(&variable.value);
            }
        }
        for header in input
            .request
            .headers
            .iter()
            .chain(input.response.iter().flat_map(|response| &response.headers))
        {
            if is_sensitive_workspace_variable(&header.key, &header.value, false)
                || header.key.eq_ignore_ascii_case("set-cookie")
            {
                redactions.record(&header.value);
            }
        }
        redactions
    }

    pub(super) fn record(&mut self, value: &str) {
        self.insert(value);
        if !self.complete {
            return;
        }
        // A generated value may be a wrapped credential (e.g. "Bearer ...")
        // or a structured payload. Treat its components as secret too, without
        // guessing from the destination's field name.
        for component in value.split_whitespace() {
            self.insert(component);
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(value) {
            self.collect_json(&json);
        }
    }

    fn insert(&mut self, value: &str) {
        if !self.complete || value.is_empty() || value == REDACTED_VALUE {
            return;
        }
        if self.bytes.saturating_add(value.len()) > SCRIPT_MAX_OUTPUT_BYTES
            || self.values.len() >= 2048
        {
            // Bound memory and replacement work independently of JS output.
            // Losing provenance must never turn into permission to show text.
            self.complete = false;
        } else if !self.values.iter().any(|existing| existing == value) {
            self.bytes += value.len();
            self.values.push(value.to_owned());
        }
    }

    fn collect_json(&mut self, value: &serde_json::Value) {
        match value {
            serde_json::Value::String(value) => self.insert(value),
            serde_json::Value::Object(fields) => {
                for value in fields.values() {
                    self.collect_json(value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    self.collect_json(value);
                }
            }
            serde_json::Value::Number(_) | serde_json::Value::Bool(_) => {
                self.insert(&value.to_string());
            }
            _ => {}
        }
    }

    pub(super) fn collect_output(&mut self, input: &ScriptRunInput, output: &RawScriptOutput) {
        for (before, after) in [
            (&input.variables, &output.variables),
            (&input.environment, &output.environment),
        ] {
            for variable in after {
                if is_sensitive_workspace_variable(&variable.key, &variable.value, false)
                    || !before
                        .iter()
                        .any(|old| old.key == variable.key && old.value == variable.value)
                {
                    self.record(&variable.value);
                }
            }
        }
        let before = &input.request;
        let after = &output.request;
        if before.method != after.method {
            self.record(&after.method);
        }
        if before.url != after.url {
            self.record(&after.url);
            if let Ok(url) = reqwest::Url::parse(&after.url) {
                for (_, value) in url.query_pairs() {
                    self.record(&value);
                }
                for segment in url.path_segments().into_iter().flatten() {
                    self.record(segment);
                }
            } else {
                self.complete = false;
            }
        }
        if before.body_raw != after.body_raw {
            if let Some(body) = &after.body_raw {
                self.record(body);
                if !body.is_empty() && serde_json::from_str::<serde_json::Value>(body).is_err() {
                    // Like opaque request snapshots, embedded unnamed values
                    // in text/XML/form payloads cannot be separated reliably.
                    self.complete = false;
                }
            }
        }
        for header in &after.headers {
            if !before.headers.contains(header)
                || is_sensitive_workspace_variable(&header.key, &header.value, false)
            {
                self.record(&header.value);
            }
        }
    }

    pub(super) fn scrub(&self, message: &str) -> String {
        if !self.complete {
            return REDACTED_VALUE.to_string();
        }
        // Use the exact raw/JSON-escaped/URL-encoded replacement and structural
        // redaction used by execution snapshots/history, including short values.
        let safe = crate::api_client::scrub_values(message, &self.values);
        let safe = redact_sensitive_lines(&redact_json_body(&safe).0).0;
        // Embedded/invalid JSON in an exception cannot be structurally walked.
        // Reuse shared credential naming and hide ambiguous diagnostic text.
        if is_sensitive_flow_name(&safe) {
            REDACTED_VALUE.to_string()
        } else {
            safe
        }
    }

    pub(super) fn sanitize_diagnostics(
        &self,
        console: &mut [ScriptConsoleEntry],
        tests: &mut [ScriptTestResult],
    ) {
        for entry in console {
            entry.message = self.scrub(&entry.message);
        }
        for test in tests {
            test.name = self.scrub(&test.name);
            test.error_message = test
                .error_message
                .as_deref()
                .map(|message| self.scrub(message));
        }
    }
}
