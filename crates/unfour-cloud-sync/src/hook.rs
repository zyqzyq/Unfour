use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sqlx::SqliteConnection;
use tokio::sync::mpsc;
use unfour_command_bus::TransactionalCommandHook;
use unfour_core::domain::{CommandContext, DomainMutation, MutationOrigin};
use unfour_core::{AppError, AppResult};

use crate::{Clock, IdGenerator, SyncRepository};

/// Pro's CommandBus transaction hook. It performs SQLite-only work on the
/// caller-owned connection and never reads canonical payloads or calls the
/// network. The optional trigger is an in-memory hint; a rolled-back command
/// leaves no outbox row, so a spurious hint is harmless.
pub struct SyncOutboxHook {
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
    trigger: Option<mpsc::UnboundedSender<String>>,
}

impl SyncOutboxHook {
    pub fn new(
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        trigger: Option<mpsc::UnboundedSender<String>>,
    ) -> Self {
        Self {
            ids,
            clock,
            trigger,
        }
    }
}

impl TransactionalCommandHook for SyncOutboxHook {
    fn on_mutations<'a>(
        &'a self,
        connection: &'a mut SqliteConnection,
        context: &'a CommandContext,
        mutations: &'a [DomainMutation],
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            if context.origin != MutationOrigin::Local {
                return Ok(());
            }
            let local_mutations = mutations
                .iter()
                .filter(|mutation| mutation.origin == MutationOrigin::Local)
                .cloned()
                .collect::<Vec<_>>();
            if local_mutations.is_empty() {
                return Ok(());
            }
            let workspaces = SyncRepository::enqueue_mutations_on(
                connection,
                &local_mutations,
                self.ids.as_ref(),
                self.clock.as_ref(),
            )
            .await
            .map_err(|error| AppError::Config(error.code().to_string()))?;
            if let Some(trigger) = &self.trigger {
                for workspace_id in workspaces {
                    let _ = trigger.send(workspace_id);
                }
            }
            Ok(())
        })
    }
}
