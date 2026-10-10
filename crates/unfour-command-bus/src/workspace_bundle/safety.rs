use super::*;
use unfour_core::redaction::is_sensitive_flow_name;
pub(super) fn sanitize(bundle: &mut WorkspaceBundle) -> AppResult<()> {
    for r in &mut bundle.variables {
        if unfour_core::redaction::is_sensitive_workspace_variable(
            &r.key,
            match &r.value {
                SnapshotVariableValue::Plain(value) => value,
                _ => "",
            },
            r.is_secret || matches!(r.value, SnapshotVariableValue::SecretRedacted),
        ) {
            r.is_secret = true;
            r.value = SnapshotVariableValue::SecretRedacted;
        }
    }
    for r in &mut bundle.environment_variables {
        if unfour_core::redaction::is_sensitive_workspace_variable(
            &r.key,
            match &r.value {
                SnapshotVariableValue::Plain(value) => value,
                _ => "",
            },
            r.is_secret || matches!(r.value, SnapshotVariableValue::SecretRedacted),
        ) {
            r.is_secret = true;
            r.value = SnapshotVariableValue::SecretRedacted;
        }
    }
    for r in &mut bundle.requests {
        let mut value = serde_json::to_value(&*r)?;
        value["workspaceId"] = json!("portable");
        value["createdAt"] = json!("portable");
        value["updatedAt"] = json!("portable");
        value["revision"] = json!(1);
        let mut snapshot: ApiRequestSnapshot = serde_json::from_value(value)?;
        unfour_http_engine::sanitize_portable_api_request(&mut snapshot)?;
        *r = portable(snapshot)?;
        if r.url.contains("@unfour-secret:") {
            r.url = "<redacted>".into();
        }
        // V1 carries only the supported timeout setting, never unknown opaque config.
        let settings: ApiRequestSettings =
            serde_json::from_str(&r.settings_json).map_err(|_| invalid())?;
        r.settings_json = serde_json::to_string(&settings)?;
        for text in [&mut r.pre_request_script, &mut r.post_response_script] {
            if let Some(text) = text {
                *text = scrub_text(text);
            }
        }
        if let Some(body) = &mut r.body {
            let mut value = json!(body);
            scrub(&mut value);
            *body = value.as_str().ok_or_else(invalid)?.into();
        }
    }
    for c in &mut bundle.connections {
        if let BundleConnectionConfig::Database {
            driver,
            database_name,
            username,
            ssl_mode,
            ..
        } = &mut c.config
        {
            if driver == "sqlite" {
                c.host = None;
                c.port = None;
                *database_name = None;
                *username = None;
                *ssl_mode = None;
            }
        }
    }
    for step in &mut bundle.ssh_steps {
        if !step.config_json.is_object() {
            return Err(invalid());
        }
        scrub(&mut step.config_json);
        if matches!(step.step_type.as_str(), "upload" | "download") {
            step.config_json["localPath"] = json!("{{local_path}}");
            // Missing dependencies must fail preflight, never silently skip an enabled step.
        }
    }
    for r in &mut bundle.saved_sql {
        r.sql = scrub_text(&r.sql);
    }
    for flow in &mut bundle.flows {
        for input in &mut flow.inputs {
            if input.secret || is_sensitive_flow_name(&input.name) {
                input.default = None;
            } else if let Some(value) = &mut input.default {
                scrub(value);
            }
        }
        // Definitions already forbid inline secrets; preserve reference expressions and schema flags.
        for step in &mut flow.steps {
            let mut value = serde_json::to_value(&step.node)?;
            scrub(&mut value);
            step.node = serde_json::from_value(value)?;
        }
    }
    super::templates::apply(bundle)?;
    Ok(())
}
fn scrub_text(text: &str) -> String {
    let mut private_key = false;
    text.split_inclusive('\n')
        .map(|line| {
            let upper = line.to_ascii_uppercase();
            let key_start = upper.contains("-----BEGIN ") && upper.contains("PRIVATE KEY-----");
            let key_end = upper.contains("-----END ") && upper.contains("PRIVATE KEY-----");
            private_key |= key_start;
            let sensitive = private_key
                || line.contains("@unfour-secret:")
                || unfour_core::redaction::is_sensitive_log_line(line)
                || (is_sensitive_flow_name(line) && line.contains(['=', ':']));
            let result = if sensitive && !is_reference_text(line.trim_end()) {
                let ending = if line.ends_with("\r\n") {
                    "\r\n"
                } else if line.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                format!("<redacted>{ending}")
            } else {
                line.to_string()
            };
            if key_end {
                private_key = false;
            }
            result
        })
        .collect()
}
// Device paths/credential handles are not portable, including JSON encoded payloads.
fn scrub(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let pair = map
                .get("key")
                .and_then(Value::as_str)
                .is_some_and(is_sensitive_flow_name);
            for (key, value) in map {
                if key == "key" && pair {
                    continue;
                }
                let reference =
                    value.as_str().is_some_and(is_reference_text) || value.get("$ref").is_some();
                if (is_sensitive_flow_name(key)
                    || (pair && key == "value")
                    || matches!(key.as_str(), "localPath" | "filePath" | "sqlitePath"))
                    && !reference
                {
                    *value = json!("<redacted>");
                } else {
                    scrub(value);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                scrub(item);
            }
        }
        Value::String(text) => {
            if let Ok(mut parsed) = serde_json::from_str::<Value>(text) {
                if parsed.is_object() || parsed.is_array() {
                    scrub(&mut parsed);
                    *text = parsed.to_string();
                }
            } else {
                *text = scrub_text(text);
            }
        }
        _ => {}
    }
}
pub(super) fn preview(bundle: &WorkspaceBundle) -> WorkspaceBundlePreview {
    let counts = BTreeMap::from([
        ("variables".into(), bundle.variables.len()),
        ("environments".into(), bundle.environments.len()),
        (
            "environmentVariables".into(),
            bundle.environment_variables.len(),
        ),
        ("collections".into(), bundle.collections.len()),
        ("folders".into(), bundle.folders.len()),
        ("requests".into(), bundle.requests.len()),
        ("connections".into(), bundle.connections.len()),
        ("sshTasks".into(), bundle.ssh_tasks.len()),
        ("sshSteps".into(), bundle.ssh_steps.len()),
        ("savedSql".into(), bundle.saved_sql.len()),
        ("flows".into(), bundle.flows.len()),
    ]);
    let mut reconfigure = Vec::new();
    let mut issue = |id: &str, name: &str, code: &str, field: &str, status: &str| {
        reconfigure.push(WorkspaceBundleIssue {
            entity_id: id.into(),
            name: name.into(),
            code: code.into(),
            field: field.into(),
            status: status.into(),
        })
    };
    for c in &bundle.connections {
        if bundle.credential_requirements.contains(&c.id)
            || matches!(&c.config, BundleConnectionConfig::Ssh { auth_method, .. } if auth_method == "password")
        {
            issue(&c.id, &c.name, "connection", "credential", "missing");
        }
        let field = match &c.config {
            BundleConnectionConfig::Ssh { auth_method, .. } if auth_method == "private-key" => {
                Some("keyPath")
            }
            BundleConnectionConfig::Database { driver, .. } if driver == "sqlite" => {
                Some("sqlitePath")
            }
            _ => None,
        };
        if let Some(field) = field {
            if !bundle
                .local_paths
                .iter()
                .any(|p| p.entity_id == c.id && p.field == field)
            {
                issue(&c.id, &c.name, "localPath", field, "missing");
            }
        }
    }
    for v in &bundle.variables {
        if v.is_secret {
            issue(&v.id, &v.key, "secret", "value", "missing");
        }
    }
    for v in &bundle.environment_variables {
        if v.is_secret {
            issue(&v.id, &v.key, "secret", "value", "missing");
        }
    }
    for r in &bundle.requests {
        for (field, value) in [
            ("auth", r.auth_json.clone()),
            ("url", r.url.clone()),
            (
                "headers",
                serde_json::to_string(&r.headers).unwrap_or_default(),
            ),
            ("query", serde_json::to_string(&r.query).unwrap_or_default()),
            ("body", r.body.clone().unwrap_or_default()),
            (
                "preRequestScript",
                r.pre_request_script.clone().unwrap_or_default(),
            ),
            (
                "postResponseScript",
                r.post_response_script.clone().unwrap_or_default(),
            ),
        ] {
            if redacted(&value) {
                issue(&r.id, &r.name, "redacted", field, "missing");
            }
        }
        if r.body_kind == MULTIPART_BODY_KIND
            && parse_multipart_definition(r.body.as_deref())
                .unwrap_or_default()
                .iter()
                .any(|p| matches!(p, ApiMultipartPart::File { enabled: true, .. }))
        {
            issue(&r.id, &r.name, "files", "body", "missing");
        }
    }
    for r in &bundle.ssh_steps {
        if matches!(r.step_type.as_str(), "upload" | "download") {
            if !bundle
                .local_paths
                .iter()
                .any(|p| p.entity_id == r.id && p.field == "localPath")
            {
                issue(&r.id, &r.name, "localPath", "localPath", "missing");
            }
        }
        if redacted(&r.config_json.to_string()) {
            issue(&r.id, &r.name, "redacted", "config", "missing");
        }
    }
    for r in &bundle.saved_sql {
        if redacted(&r.sql) {
            issue(&r.id, &r.name, "redacted", "sql", "missing");
        }
    }
    for f in &bundle.flows {
        if redacted(&serde_json::to_string(f).unwrap_or_default()) {
            issue(&f.id, &f.name, "redacted", "steps", "missing");
        }
    }
    for path in &bundle.local_paths {
        let name = bundle
            .connections
            .iter()
            .find(|r| r.id == path.entity_id)
            .map(|r| r.name.as_str())
            .or_else(|| {
                bundle
                    .ssh_steps
                    .iter()
                    .find(|r| r.id == path.entity_id)
                    .map(|r| r.name.as_str())
            })
            .unwrap_or("");
        issue(&path.entity_id, name, "pathCheck", &path.field, "unchecked");
    }
    WorkspaceBundlePreview {
        name: bundle.workspace.name.clone(),
        // Display only; workspace materialization still validates the environment.
        environment_type: match bundle
            .workspace
            .environment_type
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "" => "dev".into(),
            value => value.into(),
        },
        counts,
        reconfigure,
        paths: bundle.local_paths.clone(),
    }
}

fn redacted(value: &str) -> bool {
    value.contains("<redacted>") || value.to_ascii_lowercase().contains("%3credacted%3e")
}

fn is_reference_text(value: &str) -> bool {
    let value = value.strip_prefix("Bearer ").unwrap_or(value);
    (value.starts_with("${/")
        && value.ends_with('}')
        && value.matches("${").count() == 1
        && !value[..value.len() - 1].contains('}'))
        || (value.starts_with("{{")
            && value.ends_with("}}")
            && value.matches("{{").count() == 1
            && !value[..value.len() - 2].contains("}}"))
}
