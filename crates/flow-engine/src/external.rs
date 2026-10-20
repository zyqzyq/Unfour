//! Transaction-local definition snapshots and external whole-document writes.
use crate::{definitions, expression::invalid, FlowService};
use sqlx::SqliteConnection;
use unfour_core::{domain::*, models::FlowDefinition, AppError, AppResult};

impl FlowService {
    pub async fn read_definition_snapshot_on(
        connection: &mut SqliteConnection,
        key: &DomainEntityKey,
    ) -> AppResult<DomainSnapshot> {
        if key.entity_type != DomainEntityType::FlowDefinition {
            return Err(invalid("FLOW_INVALID_ENTITY"));
        }
        let (revision, json, deleted): (i64, String, Option<String>) = sqlx::query_as(
            "SELECT revision, definition_json, deleted_at FROM flow_definitions WHERE workspace_id = ? AND id = ?",
        ).bind(&key.workspace_id).bind(&key.entity_id).fetch_optional(connection).await?
            .ok_or_else(|| AppError::NotFound("flow".into()))?;
        if let Some(deleted_at) = deleted {
            return Ok(DomainSnapshot::Tombstone(TombstoneSnapshot {
                entity: key.clone(),
                revision,
                deleted_at,
            }));
        }
        let mut raw: serde_json::Value = serde_json::from_str(&json)?;
        let definition: FlowDefinition = serde_json::from_value(raw.clone())?;
        if definition.id != key.entity_id
            || definition.workspace_id != key.workspace_id
            || definition.revision != revision
        {
            return Err(invalid("FLOW_INVALID_DEFINITION"));
        }
        definitions::validate(&definition)?;
        // Upgrade only the supported legacy input shorthand. Preserve other
        // fields so a stricter downstream adapter can reject unknown content.
        raw["inputs"] = serde_json::to_value(&definition.inputs)?;
        Ok(DomainSnapshot::FlowDefinition(FlowDefinitionSnapshot {
            id: key.entity_id.clone(),
            workspace_id: key.workspace_id.clone(),
            revision,
            definition: raw,
        }))
    }

    /// Caller fences remote versions/conflicts and commits sync metadata in this
    /// same transaction. No resource lookup, UUID remapping, history or Outbox IO.
    pub async fn apply_external_definition_on(
        connection: &mut SqliteConnection,
        context: &CommandContext,
        change: ExternalFlowDefinitionApply,
    ) -> AppResult<DomainCommandResult<ExternalMaterialization>> {
        if context.origin != MutationOrigin::External {
            return Err(invalid("FLOW_EXTERNAL_ORIGIN_REQUIRED"));
        }
        let (workspace, id) = match &change {
            ExternalFlowDefinitionApply::Upsert(value) => (&value.workspace_id, &value.id),
            ExternalFlowDefinitionApply::Delete(value) => {
                if value.entity.entity_type != DomainEntityType::FlowDefinition {
                    return Err(invalid("FLOW_INVALID_ENTITY"));
                }
                (&value.entity.workspace_id, &value.entity.entity_id)
            }
        };
        if id.is_empty() {
            return Err(invalid("FLOW_INVALID_ENTITY"));
        }
        let row: Option<(String, i64, String, Option<String>)> = sqlx::query_as(
            "SELECT workspace_id, revision, definition_json, deleted_at FROM flow_definitions WHERE id = ?",
        ).bind(id).fetch_optional(&mut *connection).await?;
        if row.as_ref().is_some_and(|row| row.0 != *workspace) {
            return Err(invalid("FLOW_WORKSPACE_MISMATCH"));
        }
        let previous = row.as_ref().map_or(0, |row| row.1);
        let next_revision = || {
            previous
                .checked_add(1)
                .filter(|_| previous >= 0)
                .ok_or_else(|| invalid("FLOW_REVISION_CONFLICT"))
        };
        let revision;
        let operation;
        match &change {
            ExternalFlowDefinitionApply::Upsert(value) => {
                definitions::require_workspace_on(connection, workspace).await?;
                let mut definition: FlowDefinition =
                    serde_json::from_value(value.definition.clone())?;
                if definition.id != *id || definition.workspace_id != *workspace {
                    return Err(invalid("FLOW_WORKSPACE_MISMATCH"));
                }
                definition.revision = previous;
                definitions::validate(&definition)?;
                if let Some((_, _, json, None)) = &row {
                    let existing: FlowDefinition = serde_json::from_str(json)?;
                    if serde_json::to_value(existing)? == serde_json::to_value(&definition)? {
                        return Ok(DomainCommandResult::unchanged(
                            ExternalMaterialization::AlreadyEquivalent,
                        ));
                    }
                }
                revision = next_revision()?;
                definition.revision = revision;
                sqlx::query("INSERT INTO flow_definitions (id, workspace_id, revision, definition_json, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, NULL) ON CONFLICT(id) DO UPDATE SET revision = excluded.revision, definition_json = excluded.definition_json, updated_at = excluded.updated_at, deleted_at = NULL")
                    .bind(id).bind(workspace).bind(revision).bind(serde_json::to_string(&definition)?)
                    .bind(chrono::Utc::now().to_rfc3339()).execute(connection).await?;
                operation = MutationOperation::Upsert;
            }
            ExternalFlowDefinitionApply::Delete(value) => {
                chrono::DateTime::parse_from_rfc3339(&value.deleted_at)
                    .map_err(|_| invalid("FLOW_INVALID_TOMBSTONE"))?;
                if row.as_ref().is_none_or(|row| row.3.is_some()) {
                    return Ok(DomainCommandResult::unchanged(
                        ExternalMaterialization::AlreadyEquivalent,
                    ));
                }
                revision = next_revision()?;
                sqlx::query("UPDATE flow_definitions SET revision = ?, definition_json = json_set(definition_json, '$.revision', ?), deleted_at = ?, updated_at = ? WHERE workspace_id = ? AND id = ?")
                    .bind(revision).bind(revision).bind(&value.deleted_at).bind(&value.deleted_at)
                    .bind(workspace).bind(id).execute(connection).await?;
                operation = MutationOperation::Delete;
            }
        }
        Ok(DomainCommandResult::new(
            ExternalMaterialization::Applied(revision),
            vec![DomainMutation::new(
                context.origin,
                operation,
                DomainEntityKey::new(DomainEntityType::FlowDefinition, workspace, id),
                revision,
            )],
        ))
    }
}
