use crate::{expression::invalid, validation, FlowService};
use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityKey, DomainEntityType, DomainMutation,
    MutationOperation,
};
use unfour_core::{models::FlowDefinition, AppError, AppResult};

pub(crate) async fn require_workspace_on(
    connection: &mut SqliteConnection,
    workspace: &str,
) -> AppResult<()> {
    let live: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ? AND deleted_at IS NULL)",
    )
    .bind(workspace)
    .fetch_one(connection)
    .await?;
    if !live {
        return Err(AppError::NotFound("workspace".into()));
    }
    Ok(())
}

fn mutation(
    context: &CommandContext,
    workspace: &str,
    id: &str,
    revision: i64,
    operation: MutationOperation,
) -> DomainMutation {
    DomainMutation::new(
        context.origin,
        operation,
        DomainEntityKey::new(DomainEntityType::FlowDefinition, workspace, id),
        revision,
    )
}

pub(crate) fn validate(definition: &FlowDefinition) -> AppResult<()> {
    validation::validate(definition)?;
    let original = serde_json::to_value(definition)?;
    let mut cleaned = original.clone();
    crate::expression::redact_definition(&mut cleaned);
    if original != cleaned {
        return Err(invalid("FLOW_INLINE_SECRET_NOT_ALLOWED"));
    }
    Ok(())
}

impl FlowService {
    pub async fn save(&self, definition: FlowDefinition) -> AppResult<FlowDefinition> {
        let mut tx = self.db.pool().begin().await?;
        let result = self
            .save_on(&mut tx, &CommandContext::local("flow.save"), definition)
            .await?;
        tx.commit().await?;
        Ok(result.value)
    }

    /// Caller owns the business/activity/outbox transaction.
    pub async fn save_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        mut definition: FlowDefinition,
    ) -> AppResult<DomainCommandResult<FlowDefinition>> {
        require_workspace_on(connection, &definition.workspace_id).await?;
        validate(&definition)?;
        let now = chrono::Utc::now().to_rfc3339();
        if definition.id.is_empty() {
            definition.id = unfour_core::id::new_id();
            definition.revision = 1;
            sqlx::query("INSERT INTO flow_definitions (id, workspace_id, revision, definition_json, updated_at) VALUES (?, ?, ?, ?, ?)")
                .bind(&definition.id).bind(&definition.workspace_id).bind(definition.revision)
                .bind(serde_json::to_string(&definition)?).bind(now).execute(&mut *connection).await?;
        } else {
            let previous = definition.revision;
            definition.revision = previous
                .checked_add(1)
                .filter(|_| previous > 0)
                .ok_or_else(|| invalid("FLOW_REVISION_CONFLICT"))?;
            let result = sqlx::query("UPDATE flow_definitions SET revision = ?, definition_json = ?, updated_at = ? WHERE workspace_id = ? AND id = ? AND revision = ? AND deleted_at IS NULL")
                .bind(definition.revision).bind(serde_json::to_string(&definition)?).bind(now)
                .bind(&definition.workspace_id).bind(&definition.id).bind(previous).execute(&mut *connection).await?;
            if result.rows_affected() != 1 {
                return Err(invalid("FLOW_REVISION_CONFLICT"));
            }
        }
        let change = mutation(
            context,
            &definition.workspace_id,
            &definition.id,
            definition.revision,
            MutationOperation::Upsert,
        );
        Ok(DomainCommandResult::new(definition, vec![change]))
    }

    pub async fn delete(&self, workspace: &str, id: &str, expected_revision: i64) -> AppResult<()> {
        let mut tx = self.db.pool().begin().await?;
        self.delete_on(
            &mut tx,
            &CommandContext::local("flow.delete"),
            workspace,
            id,
            expected_revision,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace: &str,
        id: &str,
        expected_revision: i64,
    ) -> AppResult<DomainCommandResult<()>> {
        require_workspace_on(connection, workspace).await?;
        expected_revision
            .checked_add(1)
            .filter(|_| expected_revision > 0)
            .ok_or_else(|| invalid("FLOW_REVISION_CONFLICT"))?;
        let now = chrono::Utc::now().to_rfc3339();
        let revision: Option<i64> = sqlx::query_scalar(
            "UPDATE flow_definitions SET deleted_at = ?, updated_at = ?, revision = revision + 1, definition_json = json_set(definition_json, '$.revision', revision + 1) WHERE workspace_id = ? AND id = ? AND revision = ? AND deleted_at IS NULL RETURNING revision",
        ).bind(&now).bind(&now).bind(workspace).bind(id).bind(expected_revision)
            .fetch_optional(connection).await?;
        let revision = revision.ok_or_else(|| invalid("FLOW_REVISION_CONFLICT"))?;
        Ok(DomainCommandResult::new(
            (),
            vec![mutation(
                context,
                workspace,
                id,
                revision,
                MutationOperation::Delete,
            )],
        ))
    }

    /// Workspace removal purges runtime snapshots and outputs, retaining definition tombstones.
    /// Does not require a live workspace: repeated external deletes also repair leftovers.
    pub async fn delete_workspace_on(
        connection: &mut SqliteConnection,
        context: &CommandContext,
        workspace: &str,
        deleted_at: &str,
    ) -> AppResult<Vec<DomainMutation>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "UPDATE flow_definitions SET deleted_at = ?, updated_at = ?, revision = revision + 1, definition_json = json_set(definition_json, '$.revision', revision + 1) WHERE workspace_id = ? AND deleted_at IS NULL RETURNING id, revision",
        ).bind(deleted_at).bind(deleted_at).bind(workspace).fetch_all(&mut *connection).await?;
        sqlx::query("DELETE FROM flow_runs WHERE workspace_id = ?")
            .bind(workspace)
            .execute(connection)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(id, revision)| {
                mutation(context, workspace, &id, revision, MutationOperation::Delete)
            })
            .collect())
    }

    pub async fn export_definitions_on(
        &self,
        connection: &mut SqliteConnection,
        workspace: &str,
    ) -> AppResult<Vec<FlowDefinition>> {
        require_workspace_on(connection, workspace).await?;
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT definition_json FROM flow_definitions WHERE workspace_id = ? AND deleted_at IS NULL ORDER BY id",
        ).bind(workspace).fetch_all(connection).await?;
        rows.iter()
            .map(|row| Ok(serde_json::from_str(row)?))
            .collect()
    }

    /// Fresh remapped bundle definitions only; caller owns atomic workspace import.
    pub async fn import_definition_on(
        &self,
        connection: &mut SqliteConnection,
        context: &CommandContext,
        mut definition: FlowDefinition,
    ) -> AppResult<DomainMutation> {
        require_workspace_on(connection, &definition.workspace_id).await?;
        definition.revision = 1;
        validate(&definition)?;
        sqlx::query("INSERT INTO flow_definitions (id, workspace_id, revision, definition_json, updated_at) VALUES (?, ?, 1, ?, ?)")
            .bind(&definition.id).bind(&definition.workspace_id).bind(serde_json::to_string(&definition)?)
            .bind(chrono::Utc::now().to_rfc3339()).execute(connection).await?;
        Ok(mutation(
            context,
            &definition.workspace_id,
            &definition.id,
            1,
            MutationOperation::Upsert,
        ))
    }
}
