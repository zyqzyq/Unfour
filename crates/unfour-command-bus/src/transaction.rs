use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;
use sqlx::SqliteConnection;
use unfour_core::domain::{
    CommandContext, DomainCommandResult, DomainEntityKey, DomainMutation, MutationOperation,
    MutationOrigin,
};
use unfour_core::AppResult;
use unfour_local_storage::ActivityLogService;

use crate::CommandBus;

pub trait TransactionalCommandHook: Send + Sync {
    /// Runs inside the Command Bus-owned transaction. Implementations must not
    /// commit the connection or perform network requests.
    fn on_mutations<'a>(
        &'a self,
        connection: &'a mut SqliteConnection,
        context: &'a CommandContext,
        mutations: &'a [DomainMutation],
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>>;
}

#[derive(Clone, Default)]
pub struct CommandBusExtensions {
    transactional_hooks: Arc<[Arc<dyn TransactionalCommandHook>]>,
}

impl CommandBusExtensions {
    pub fn new(transactional_hooks: Vec<Arc<dyn TransactionalCommandHook>>) -> Self {
        Self {
            transactional_hooks: transactional_hooks.into(),
        }
    }

    pub fn transactional_hooks(&self) -> &[Arc<dyn TransactionalCommandHook>] {
        &self.transactional_hooks
    }
}

pub(crate) struct CommandActivity {
    pub workspace_id: Option<String>,
    pub action: &'static str,
    pub target: Option<String>,
    pub details: Value,
}

pub(crate) type CommandExecutorFuture<'a, T> =
    Pin<Box<dyn Future<Output = AppResult<DomainCommandResult<T>>> + Send + 'a>>;

impl CommandBus {
    pub(crate) async fn execute_domain_command<T, F>(
        &self,
        context: CommandContext,
        activity: Option<CommandActivity>,
        executor: F,
    ) -> AppResult<T>
    where
        T: Send,
        F: for<'a> FnOnce(&'a mut SqliteConnection) -> CommandExecutorFuture<'a, T>,
    {
        let mut transaction = self.db.pool().begin().await?;
        let outcome = executor(&mut transaction).await?;
        self.finalize_domain_command(&mut transaction, &context, activity, &outcome, false)
            .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    /// Like [`Self::execute_domain_command`], but builds activity from the
    /// command result so create/import flows can record the primary entity id
    /// and post-execution details inside the same transaction.
    pub(crate) async fn execute_domain_command_with_activity<T, F, A>(
        &self,
        context: CommandContext,
        activity: A,
        executor: F,
    ) -> AppResult<T>
    where
        T: Send,
        A: FnOnce(&T) -> CommandActivity + Send,
        F: for<'a> FnOnce(&'a mut SqliteConnection) -> CommandExecutorFuture<'a, T>,
    {
        self.execute_domain_command_with_activity_policy(context, false, activity, executor)
            .await
    }

    /// Like [`Self::execute_domain_command_with_activity`], but records the
    /// local activity even when the domain operation only changes device-local
    /// state and therefore returns no cloud mutation. Transactional hooks
    /// still run only when mutations are present.
    pub(crate) async fn execute_domain_command_with_activity_even_without_mutation<T, F, A>(
        &self,
        context: CommandContext,
        activity: A,
        executor: F,
    ) -> AppResult<T>
    where
        T: Send,
        A: FnOnce(&T) -> CommandActivity + Send,
        F: for<'a> FnOnce(&'a mut SqliteConnection) -> CommandExecutorFuture<'a, T>,
    {
        self.execute_domain_command_with_activity_policy(context, true, activity, executor)
            .await
    }

    async fn execute_domain_command_with_activity_policy<T, F, A>(
        &self,
        context: CommandContext,
        record_activity_without_mutation: bool,
        activity: A,
        executor: F,
    ) -> AppResult<T>
    where
        T: Send,
        A: FnOnce(&T) -> CommandActivity + Send,
        F: for<'a> FnOnce(&'a mut SqliteConnection) -> CommandExecutorFuture<'a, T>,
    {
        let mut transaction = self.db.pool().begin().await?;
        let outcome = executor(&mut transaction).await?;
        let activity = activity(&outcome.value);
        self.finalize_domain_command(
            &mut transaction,
            &context,
            Some(activity),
            &outcome,
            record_activity_without_mutation,
        )
        .await?;
        transaction.commit().await?;
        Ok(outcome.value)
    }

    async fn finalize_domain_command<T>(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        context: &CommandContext,
        activity: Option<CommandActivity>,
        outcome: &DomainCommandResult<T>,
        record_activity_without_mutation: bool,
    ) -> AppResult<()> {
        if outcome.mutations.is_empty() && !record_activity_without_mutation {
            return Ok(());
        }
        if let Some(activity) = activity {
            // Create commands do not know their generated entity id until the
            // executor returns. Derive missing local activity scope from
            // mutations when the caller left target/workspace unset.
            // Single-mutation creates use that entity; multi-mutation local
            // creates (for example default collection + request) use the last
            // Upsert as the primary entity. Bulk callers that need a different
            // target must supply it explicitly or via
            // execute_domain_command_with_activity.
            let created_entity = resolve_created_entity(context, &activity, &outcome.mutations);
            let workspace_id = activity
                .workspace_id
                .as_deref()
                .or_else(|| created_entity.map(|entity| entity.workspace_id.as_str()));
            let target = activity
                .target
                .as_deref()
                .or_else(|| created_entity.map(|entity| entity.entity_id.as_str()));
            ActivityLogService::record_on(
                transaction,
                workspace_id,
                activity.action,
                target,
                activity.details,
            )
            .await?;
        }
        if outcome.mutations.is_empty() {
            return Ok(());
        }
        for hook in self.extensions.transactional_hooks() {
            hook.on_mutations(transaction, context, &outcome.mutations)
                .await?;
        }
        Ok(())
    }
}

fn resolve_created_entity<'a>(
    context: &CommandContext,
    activity: &CommandActivity,
    mutations: &'a [DomainMutation],
) -> Option<&'a DomainEntityKey> {
    if context.origin != MutationOrigin::Local || activity.target.is_some() {
        return None;
    }
    if mutations.len() == 1 {
        return Some(&mutations[0].entity);
    }
    mutations
        .iter()
        .rev()
        .find(|mutation| mutation.operation == MutationOperation::Upsert)
        .map(|mutation| &mutation.entity)
}
