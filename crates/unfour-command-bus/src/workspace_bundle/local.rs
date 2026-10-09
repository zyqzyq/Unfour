use super::*;
use std::collections::HashSet;

#[derive(Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceBundleOptions {
    pub password: Option<String>,
    #[serde(default)]
    pub include_secrets: bool,
    #[serde(default)]
    pub keep_local_paths: bool,
    #[serde(default)]
    pub path_mappings: Vec<PathMapping>,
    #[serde(default)]
    pub path_overrides: Vec<LocalPath>,
    #[serde(default)]
    pub check_paths: bool,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathMapping {
    pub from: String,
    pub to: String,
}
impl zeroize::Zeroize for WorkspaceBundleOptions {
    fn zeroize(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.password);
    }
}

pub(super) fn check_paths(bundle: &WorkspaceBundle, preview: &mut WorkspaceBundlePreview) {
    for path in &bundle.local_paths {
        let destination = path.field == "localPath"
            && bundle
                .ssh_steps
                .iter()
                .any(|s| s.id == path.entity_id && s.step_type == "download");
        let result = unfour_core::local_path::check_runtime(&path.path, destination);
        if let Some(issue) = preview.reconfigure.iter_mut().find(|i| {
            i.entity_id == path.entity_id && i.field == path.field && i.code == "pathCheck"
        }) {
            issue.code = result.err().unwrap_or("pathReady").into();
            issue.status = if result.is_ok() {
                "ready"
            } else if path.path.contains("{{") {
                "unchecked"
            } else {
                "missing"
            }
            .into();
        }
    }
}

pub(super) fn validate_paths(bundle: &WorkspaceBundle) -> AppResult<()> {
    let mut seen = HashSet::new();
    let mut required = HashSet::new();
    for id in &bundle.credential_requirements {
        if !required.insert(id) || !bundle.connections.iter().any(|c| c.id == *id) {
            return Err(invalid());
        }
    }
    for path in &bundle.local_paths {
        if path.path.is_empty()
            || path.path.len() > 32768
            || path.path.contains(['\0', '\r', '\n'])
            || !seen.insert((&path.entity_id, &path.field))
        {
            return Err(invalid());
        }
        let valid = match path.field.as_str() {
            "localPath" => bundle.ssh_steps.iter().any(|r| r.id == path.entity_id && matches!(r.step_type.as_str(), "upload" | "download")),
            "keyPath" => bundle.connections.iter().any(|r| r.id == path.entity_id && matches!(&r.config, BundleConnectionConfig::Ssh { auth_method, .. } if auth_method == "private-key")),
            "sqlitePath" => bundle.connections.iter().any(|r| r.id == path.entity_id && matches!(&r.config, BundleConnectionConfig::Database { driver, .. } if driver == "sqlite")),
            _ => false,
        };
        if !valid {
            return Err(invalid());
        }
    }
    Ok(())
}
pub(super) fn map_paths(bundle: &mut WorkspaceBundle, mappings: &[PathMapping]) -> AppResult<()> {
    if mappings.len() > 100 {
        return Err(invalid());
    }
    for mapping in mappings {
        let normalized = mapping.from.replace('\\', "/");
        let from = if normalized == "/" {
            "/"
        } else {
            normalized.trim_end_matches('/')
        };
        if from.is_empty() || mapping.to.is_empty() {
            return Err(invalid());
        }
        for path in &mut bundle.local_paths {
            let value = path.path.replace('\\', "/");
            let windows = from.as_bytes().get(1) == Some(&b':') || from.starts_with("//");
            let matches = value.get(..from.len()).is_some_and(|prefix| {
                if windows {
                    prefix.eq_ignore_ascii_case(from)
                } else {
                    prefix == from
                }
            });
            if !matches {
                continue;
            }
            let suffix = &value[from.len()..];
            if !suffix.is_empty() && !suffix.starts_with('/') && from != "/" {
                continue;
            }
            let separator = if mapping.to.contains('\\') { "\\" } else { "/" };
            let base = mapping.to.trim_end_matches(['/', '\\']);
            if suffix.is_empty() {
                path.path = mapping.to.clone();
            } else {
                path.path = format!(
                    "{base}{separator}{}",
                    suffix.trim_start_matches('/').replace('/', separator)
                );
            }
        }
    }
    validate_paths(bundle)
}
pub(super) fn override_paths(
    bundle: &mut WorkspaceBundle,
    overrides: &[LocalPath],
) -> AppResult<()> {
    let mut seen = HashSet::new();
    for replacement in overrides {
        if !seen.insert((&replacement.entity_id, &replacement.field)) {
            return Err(invalid());
        }
        if let Some(path) = bundle
            .local_paths
            .iter_mut()
            .find(|p| p.entity_id == replacement.entity_id && p.field == replacement.field)
        {
            path.path = replacement.path.clone();
        } else {
            bundle.local_paths.push(replacement.clone());
        }
    }
    validate_paths(bundle)
}

impl CommandBus {
    pub(super) async fn collect_local_fields(
        &self,
        bundle: &mut WorkspaceBundle,
        keep_paths: bool,
    ) -> AppResult<Vec<(String, String)>> {
        let mut db = self.db.pool().acquire().await?;
        let workspace = &bundle.workspace.id;
        let mut credentials = Vec::new();
        for (rows, field) in [
            (
                self.ssh
                    .bundle_connection_fields_on(&mut db, workspace)
                    .await?,
                "keyPath",
            ),
            (
                self.database
                    .bundle_connection_fields_on(&mut db, workspace)
                    .await?,
                "sqlitePath",
            ),
        ] {
            for (id, path, credential) in rows {
                let config = &bundle
                    .connections
                    .iter()
                    .find(|c| c.id == id)
                    .ok_or_else(invalid)?
                    .config;
                let uses_credential = match config {
                    BundleConnectionConfig::Ssh { auth_method, .. } => {
                        matches!(auth_method.as_str(), "password" | "private-key")
                    }
                    BundleConnectionConfig::Database { driver, .. } => driver != "sqlite",
                };
                let uses_path = field != "keyPath"
                    || matches!(config, BundleConnectionConfig::Ssh { auth_method, .. } if auth_method == "private-key");
                if let Some(reference) = credential.filter(|s| uses_credential && !s.is_empty()) {
                    bundle.credential_requirements.push(id.clone());
                    credentials.push((id.clone(), reference));
                }
                if keep_paths && uses_path {
                    if let Some(path) = path.filter(|s| !s.is_empty()) {
                        bundle.local_paths.push(LocalPath {
                            entity_id: id,
                            field: field.into(),
                            path,
                        });
                    }
                }
            }
        }
        for (id, path, enabled) in self
            .ssh
            .bundle_transfer_fields_on(&mut db, workspace)
            .await?
        {
            if let Some(step) = bundle.ssh_steps.iter_mut().find(|s| s.id == id) {
                step.enabled = enabled;
            }
            if keep_paths && !path.is_empty() {
                bundle.local_paths.push(LocalPath {
                    entity_id: id,
                    field: "localPath".into(),
                    path,
                });
            }
        }
        validate_paths(bundle)?;
        Ok(credentials)
    }
    pub(super) async fn restore_local_fields_on(
        &self,
        db: &mut sqlx::SqliteConnection,
        plan: &remap::ImportPlan,
    ) -> AppResult<()> {
        for template in &plan.api_templates {
            let (id, _) = plan.ids.get(&template.entity_id).ok_or_else(invalid)?;
            self.api_client
                .restore_bundle_template_on(
                    db,
                    &plan.workspace_id,
                    id,
                    &template.field,
                    &template.pointer,
                    &template.value,
                )
                .await?;
        }
        for path in &plan.local_paths {
            let (id, _) = plan.ids.get(&path.entity_id).ok_or_else(invalid)?;
            match path.field.as_str() {
                "sqlitePath" => {
                    self.database
                        .restore_bundle_path_on(db, &plan.workspace_id, id, &path.path)
                        .await?
                }
                _ => {
                    self.ssh
                        .restore_bundle_path_on(db, &plan.workspace_id, id, &path.field, &path.path)
                        .await?
                }
            }
        }
        Ok(())
    }
}
