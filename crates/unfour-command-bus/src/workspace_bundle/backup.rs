use super::*;
mod api_fields;
use api_fields::*;
use std::collections::{HashMap, HashSet};
use unfour_secret_store::backup as encryption;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupPayload {
    bundle: WorkspaceBundle,
    secrets: Vec<BackupSecret>,
}
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupSecret {
    #[zeroize(skip)]
    kind: String,
    #[zeroize(skip)]
    targets: Vec<String>,
    #[serde(default)]
    field: String,
    #[serde(default)]
    pointer: String,
    value: String,
}

fn decode(content: &str, options: &WorkspaceBundleOptions) -> AppResult<BackupPayload> {
    if content.len() > encryption::MAX_ARCHIVE_BYTES {
        return Err(invalid());
    }
    let mut payload = if encryption::is_encrypted(content) {
        let password = options
            .password
            .as_deref()
            .ok_or_else(|| AppError::Validation("WORKSPACE_BUNDLE_PASSWORD_REQUIRED".into()))?;
        let plaintext = encryption::decrypt(content, password)?;
        let mut payload: BackupPayload =
            serde_json::from_slice(&plaintext).map_err(|_| invalid())?;
        if payload.bundle.format != "unfour-workspace" || payload.bundle.version != 2 {
            return Err(invalid());
        }
        safety::sanitize(&mut payload.bundle)?;
        payload
    } else {
        BackupPayload {
            bundle: parse(content)?,
            secrets: vec![],
        }
    };
    local::map_paths(&mut payload.bundle, &options.path_mappings)?;
    local::override_paths(&mut payload.bundle, &options.path_overrides)?;
    validate_secrets(&payload)?;
    Ok(payload)
}
fn validate_secrets(payload: &BackupPayload) -> AppResult<()> {
    let mut targets = HashSet::new();
    for secret in &payload.secrets {
        if secret.kind != "api-secret" && (!secret.field.is_empty() || !secret.pointer.is_empty()) {
            return Err(invalid());
        }
        if secret.value.is_empty() || secret.value.len() > 1024 * 1024 || secret.targets.is_empty()
        {
            return Err(invalid());
        }
        for target in &secret.targets {
            if !targets.insert((target, &secret.field, &secret.pointer)) {
                return Err(invalid());
            }
            let valid = match secret.kind.as_str() {
                "workspace-variable" => payload
                    .bundle
                    .variables
                    .iter()
                    .any(|v| v.id == *target && v.is_secret),
                "environment-variable" => payload
                    .bundle
                    .environment_variables
                    .iter()
                    .any(|v| v.id == *target && v.is_secret),
                "api-secret" => payload
                    .bundle
                    .requests
                    .iter()
                    .any(|r| r.id == *target && api_slot(r, &secret.field, &secret.pointer)),
                "ssh-password" | "ssh-key-passphrase" | "database-password" => {
                    payload.bundle.connections.iter().any(|c| {
                        c.id == *target
                            && match &c.config {
                                BundleConnectionConfig::Ssh { auth_method, .. } => {
                                    (secret.kind == "ssh-password" && auth_method == "password")
                                        || (secret.kind == "ssh-key-passphrase"
                                            && auth_method == "private-key")
                                }
                                BundleConnectionConfig::Database { driver, .. } => {
                                    secret.kind == "database-password" && driver != "sqlite"
                                }
                            }
                    })
                }
                _ => false,
            };
            if !valid {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

impl CommandBus {
    pub async fn workspace_bundle_export_with_options(
        &self,
        workspace: String,
        options: WorkspaceBundleOptions,
    ) -> AppResult<WorkspaceBundleExportArtifact> {
        let options = Zeroizing::new(options);
        if options.include_secrets && options.password.is_none() {
            return Err(AppError::Validation(
                "WORKSPACE_BUNDLE_PASSWORD_REQUIRED".into(),
            ));
        }
        // Prevent credential reclamation by writers until every referenced value
        // has been captured. Encryption runs after releasing this short snapshot.
        let mut db = if options.include_secrets {
            self.db.pool().begin_with("BEGIN IMMEDIATE").await?
        } else {
            self.db.pool().begin().await?
        };
        let mut bundle = self
            .workspace_bundle_snapshot_on(&mut db, &workspace)
            .await?;
        bundle.version = 2;
        let references = self
            .collect_local_fields_on(&mut db, &mut bundle, options.keep_local_paths)
            .await?;
        self.collect_bundle_templates_on(&mut db, &mut bundle)
            .await?;
        let mut secrets: Vec<BackupSecret> = Vec::new();
        if options.include_secrets {
            let mut grouped: HashMap<(String, String), usize> = HashMap::new();
            for (id, reference) in references {
                let kind = match &bundle
                    .connections
                    .iter()
                    .find(|c| c.id == id)
                    .ok_or_else(invalid)?
                    .config
                {
                    BundleConnectionConfig::Ssh { auth_method, .. }
                        if auth_method == "password" =>
                    {
                        "ssh-password"
                    }
                    BundleConnectionConfig::Ssh { auth_method, .. }
                        if auth_method == "private-key" =>
                    {
                        "ssh-key-passphrase"
                    }
                    BundleConnectionConfig::Database { driver, .. } if driver != "sqlite" => {
                        "database-password"
                    }
                    _ => continue,
                }
                .to_string();
                let group = (reference.clone(), kind.clone());
                if let Some(index) = grouped.get(&group) {
                    secrets[*index].targets.push(id);
                    continue;
                }
                let value = self
                    .secret_store
                    .read_secret(workspace.clone(), reference.clone())
                    .await
                    .map_err(|_| {
                        AppError::Validation("WORKSPACE_BUNDLE_CREDENTIAL_UNAVAILABLE".into())
                    })?;
                grouped.insert(group, secrets.len());
                secrets.push(BackupSecret {
                    kind,
                    targets: vec![id],
                    value,
                    field: String::new(),
                    pointer: String::new(),
                });
            }
            for (id, kind, value) in self
                .workspace
                .bundle_variable_values_on(&mut db, &workspace)
                .await?
            {
                secrets.push(BackupSecret {
                    kind: if kind == "variable" {
                        "workspace-variable"
                    } else {
                        "environment-variable"
                    }
                    .into(),
                    targets: vec![id],
                    value,
                    field: String::new(),
                    pointer: String::new(),
                });
            }
            for raw in self
                .api_client
                .bundle_requests_on(&mut db, &workspace)
                .await?
            {
                let safe = bundle
                    .requests
                    .iter()
                    .find(|r| r.id == raw.id)
                    .ok_or_else(invalid)?;
                for (field, original, sanitized) in [
                    ("auth", raw.auth_json.as_str(), safe.auth_json.clone()),
                    (
                        "headers",
                        raw.headers_json.as_str(),
                        serde_json::to_string(&safe.headers)?,
                    ),
                    (
                        "query",
                        raw.query_json.as_str(),
                        serde_json::to_string(&safe.query)?,
                    ),
                ] {
                    collect_api_secrets(
                        &raw.id,
                        field,
                        "",
                        &serde_json::from_str(original)?,
                        &serde_json::from_str(&sanitized)?,
                        &mut secrets,
                    );
                }
                if (safe.url.contains("<redacted>")
                    || safe.url.to_ascii_lowercase().contains("%3credacted%3e"))
                    && !raw.url.contains("<redacted>")
                    && !unfour_http_engine::is_bundle_variable_template(&raw.url)
                {
                    secrets.push(BackupSecret {
                        kind: "api-secret".into(),
                        targets: vec![raw.id.clone()],
                        field: "url".into(),
                        pointer: String::new(),
                        value: raw.url,
                    });
                }
            }
        }
        db.rollback().await?;
        let suggested_file_name = filename::suggested_file_name(&bundle.workspace.name);
        let payload = BackupPayload { bundle, secrets };
        validate_secrets(&payload)?;
        let content = if let Some(password) = options.password.as_deref() {
            let plaintext = Zeroizing::new(serde_json::to_vec(&payload)?);
            encryption::encrypt(&plaintext, password)?
        } else {
            serde_json::to_string_pretty(&payload.bundle)?
        };
        if content.len() > encryption::MAX_ARCHIVE_BYTES {
            return Err(invalid());
        }
        Ok(WorkspaceBundleExportArtifact {
            content,
            suggested_file_name,
        })
    }

    pub async fn workspace_bundle_preview_with_options(
        &self,
        content: &str,
        options: WorkspaceBundleOptions,
    ) -> AppResult<WorkspaceBundlePreview> {
        let options = Zeroizing::new(options);
        let payload = decode(content, &options)?;
        let restored: HashSet<_> = payload
            .secrets
            .iter()
            .filter(|s| s.kind != "api-secret")
            .flat_map(|s| s.targets.iter())
            .collect();
        let mut preview = safety::preview(&payload.bundle);
        if options.check_paths {
            local::check_paths(&payload.bundle, &mut preview);
        }
        preview.reconfigure.retain(|issue| {
            !matches!(issue.code.as_str(), "secret" | "connection")
                || !restored.contains(&issue.entity_id)
        });
        // Preview only placeholders here; decrypted values never enter the frontend preview.
        let mut ready_bundle = payload.bundle.clone();
        for secret in payload.secrets.iter().filter(|s| s.kind == "api-secret") {
            for target in &secret.targets {
                let request = ready_bundle
                    .requests
                    .iter_mut()
                    .find(|r| r.id == *target)
                    .ok_or_else(invalid)?;
                mark_api_slot_ready(request, &secret.field, &secret.pointer)?;
            }
        }
        let remaining = safety::preview(&ready_bundle);
        preview.reconfigure.retain(|issue| {
            issue.code != "redacted"
                || remaining.reconfigure.iter().any(|r| {
                    r.entity_id == issue.entity_id && r.field == issue.field && r.code == "redacted"
                })
        });
        preview
            .counts
            .insert("credentials".into(), payload.secrets.len());
        let plan = remap::prepare(payload.bundle, options.mcp_policy.as_deref())?;
        let mut tx = self.db.pool().begin().await?;
        preview.name = self
            .workspace
            .bundle_name_on(&mut tx, &preview.name)
            .await?;
        self.import_bundle_on(&mut tx, plan).await?;
        tx.rollback().await?;
        Ok(preview)
    }

    pub async fn workspace_bundle_import_with_options(
        &self,
        content: String,
        name: String,
        options: WorkspaceBundleOptions,
    ) -> AppResult<Workspace> {
        let options = Zeroizing::new(options);
        let mut payload = decode(&content, &options)?;
        payload.bundle.workspace.name = name;
        let plan = remap::prepare(payload.bundle, options.mcp_policy.as_deref())?;
        // Validate all domain data before touching the keychain.
        let mut tx = self.db.pool().begin().await?;
        self.import_bundle_on(
            &mut tx,
            remap::ImportPlan {
                workspace_id: plan.workspace_id.clone(),
                page: plan.page.clone(),
                sql: plan.sql.clone(),
                flows: plan.flows.clone(),
                ids: plan.ids.clone(),
                local_paths: plan.local_paths.clone(),
                api_templates: plan.api_templates.clone(),
            },
        )
        .await?;
        tx.rollback().await?;
        let _credential_guard = self.credential_stage_guard().await?;
        let workspace = plan.workspace_id.clone();
        let result = self.import_backup(plan, payload.secrets).await;
        if result.is_err() {
            // Journal survives cleanup failures for recovery; never report a partial import as success.
            if self
                .workspace
                .discard_bundle_stages(&workspace)
                .await
                .is_err()
                || self.cleanup_bundle_garbage().await.is_err()
            {
                return Err(AppError::Validation(
                    "WORKSPACE_BUNDLE_CLEANUP_PENDING".into(),
                ));
            }
        }
        result
    }
    async fn import_backup(
        &self,
        plan: remap::ImportPlan,
        secrets: Vec<BackupSecret>,
    ) -> AppResult<Workspace> {
        let workspace_id = plan.workspace_id.clone();
        let mut bindings = Vec::new();
        for secret in &secrets {
            let value = if matches!(
                secret.kind.as_str(),
                "ssh-password" | "ssh-key-passphrase" | "database-password"
            ) {
                let reference = self.secret_store.make_ref(
                    &workspace_id,
                    &secret.kind,
                    &unfour_core::id::new_id(),
                );
                self.workspace
                    .journal_bundle_credential(&workspace_id, &reference)
                    .await?;
                self.secret_store
                    .rotate_credential(
                        workspace_id.clone(),
                        reference.clone(),
                        secret.value.clone(),
                    )
                    .await?;
                reference
            } else {
                secret.value.clone()
            };
            for target in &secret.targets {
                let (id, kind) = plan.ids.get(target).ok_or_else(invalid)?;
                bindings.push((
                    id.clone(),
                    *kind,
                    Zeroizing::new(value.clone()),
                    secret.field.clone(),
                    secret.pointer.clone(),
                ));
            }
        }
        let bus = self.clone();
        self.execute_domain_command(
            CommandContext::local("workspace.bundle.import"),
            None,
            move |db| {
                Box::pin(async move {
                    let mutations = bus.import_bundle_on(db, plan).await?;
                    for (id, kind, value, field, pointer) in bindings {
                        match kind {
                            "sshConnection" => {
                                bus.ssh
                                    .restore_bundle_credential_on(db, &workspace_id, &id, &value)
                                    .await?
                            }
                            "dbConnection" => {
                                bus.database
                                    .restore_bundle_credential_on(db, &workspace_id, &id, &value)
                                    .await?
                            }
                            "variable" | "environmentVariable" => {
                                bus.workspace
                                    .restore_bundle_variable_on(
                                        db,
                                        &workspace_id,
                                        &id,
                                        kind,
                                        &value,
                                    )
                                    .await?
                            }
                            "request" => {
                                bus.api_client
                                    .restore_bundle_secret_on(
                                        db,
                                        &workspace_id,
                                        &id,
                                        &field,
                                        &pointer,
                                        &value,
                                    )
                                    .await?
                            }
                            _ => return Err(invalid()),
                        }
                    }
                    bus.workspace
                        .attach_bundle_credentials_on(db, &workspace_id)
                        .await?;
                    let workspace = bus.workspace.read_workspace_on(db, &workspace_id).await?;
                    Ok(DomainCommandResult::new(workspace, mutations))
                })
            },
        )
        .await
    }
}
