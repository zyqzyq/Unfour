pub mod expression;
mod runner;
mod storage;
mod validation;
mod wait_until;

use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio_util::sync::CancellationToken;
use unfour_core::{models::*, AppResult};
use unfour_local_storage::LocalDb;

pub type FlowFuture<'a, T> = Pin<Box<dyn Future<Output = AppResult<T>> + Send + 'a>>;

/// The command bus implements this port using existing capability services.
/// No UI, Tauri, MCP or capability-engine dependencies are needed here.
pub trait FlowExecutor: Send + Sync {
    fn prepare<'a>(
        &'a self,
        action: &'a FlowAction,
        context: &'a FlowRunInput,
        probe: bool,
    ) -> FlowFuture<'a, Value>;
    fn execute<'a>(
        &'a self,
        action: &'a FlowAction,
        snapshot: &'a Value,
        context: &'a FlowRunInput,
        cancel: CancellationToken,
    ) -> FlowFuture<'a, Value>;
}

#[derive(Clone)]
pub struct FlowService {
    db: LocalDb,
}

impl FlowService {
    pub fn new(db: LocalDb) -> Self {
        Self { db }
    }

    pub async fn run(
        &self,
        input: FlowRunInput,
        executor: Arc<dyn FlowExecutor>,
    ) -> AppResult<FlowRun> {
        self.run_at_revision(input, executor, None).await
    }

    /// Check and execute the same owned definition, without a later reload.
    pub async fn run_at_revision(
        &self,
        input: FlowRunInput,
        executor: Arc<dyn FlowExecutor>,
        expected_revision: Option<i64>,
    ) -> AppResult<FlowRun> {
        if !input.confirm_effects {
            return Err(unfour_core::AppError::ConfirmationRequired {
                message: "Flow may perform remote side effects; cancellation does not undo them"
                    .into(),
                details: serde_json::json!({"flowId": input.flow_id}),
            });
        }
        let definition = self.get(&input.workspace_id, &input.flow_id).await?;
        if expected_revision.is_some_and(|revision| revision != definition.revision) {
            return Err(expression::invalid("FLOW_CONFIRMATION_STALE"));
        }
        validation::validate(&definition)?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut run = FlowRun {
            id: unfour_core::id::new_id(),
            workspace_id: input.workspace_id.clone(),
            flow_id: input.flow_id.clone(),
            steps: definition
                .steps
                .iter()
                .map(|s| FlowStepRun {
                    step_id: s.id.clone(),
                    status: FlowStepRunStatus::Pending,
                    duration_ms: 0,
                    attempts: vec![],
                    error: None,
                    started_at: None,
                    next_check_at: None,
                    output: None,
                })
                .collect(),
            definition,
            context: input,
            resources: serde_json::json!({}),
            status: FlowRunStatus::Running,
            error: None,
            started_at: now,
            finished_at: None,
        };
        let manual_secrets = run.context.secret_input_names.clone();
        let validated_inputs = (|| -> AppResult<()> {
            validation::resolve_inputs(&run.definition, &mut run.context.inputs)?;
            let inputs = run
                .context
                .inputs
                .as_object()
                .ok_or_else(|| expression::invalid("FLOW_INPUTS_OBJECT_REQUIRED"))?;
            if manual_secrets.iter().any(|name| !inputs.contains_key(name)) {
                return Err(expression::invalid("FLOW_UNKNOWN_SECRET_INPUT"));
            }
            if serde_json::to_vec(inputs)?.len() > 262_144 {
                return Err(expression::invalid("FLOW_INVALID_INPUTS"));
            }
            Ok(())
        })();
        // Rejected input validation cannot establish which values a misspelled
        // manual secret name intended to protect. Persist no raw runtime inputs.
        if validated_inputs.is_err() {
            run.context.inputs = serde_json::json!({});
        }
        // Validate manual names separately, but redact schema secrets even in rejected runs.
        for field in &run.definition.inputs {
            if field.secret && !run.context.secret_input_names.contains(&field.name) {
                run.context.secret_input_names.push(field.name.clone());
            }
        }
        let prepared = async {
            validated_inputs?;
            for step in &run.definition.steps {
                let (action, probe) = match &step.node {
                    FlowNode::Action { action } => (action, false),
                    FlowNode::Poll { probe, .. } | FlowNode::WaitUntil { probe, .. } => {
                        (probe, true)
                    }
                    _ => continue,
                };
                run.resources[&step.id] = executor.prepare(action, &run.context, probe).await?;
                if serde_json::to_vec(&run.resources)?.len() > 2_097_152 {
                    return Err(expression::invalid("FLOW_RESOURCE_SNAPSHOT_TOO_LARGE"));
                }
            }
            Ok(())
        }
        .await;
        if let Err(error) = prepared {
            for step in &mut run.steps {
                step.status = FlowStepRunStatus::Skipped;
            }
            run.status = FlowRunStatus::ValidationFailed;
            run.error = Some(runner::error_code(&error));
            run.finished_at = Some(chrono::Utc::now().to_rfc3339());
        }
        self.insert_run(&run).await?;
        let response = self.get_run(&run.workspace_id, &run.id).await?;
        if run.status == FlowRunStatus::Running {
            let service = self.clone();
            tokio::spawn(async move {
                let outcome = service.execute_run(&mut run, executor).await;
                if let Err(error) = outcome {
                    run.status = FlowRunStatus::Failed;
                    run.error = Some(runner::error_code(&error));
                }
                for step in &mut run.steps {
                    if step.status == FlowStepRunStatus::Pending {
                        step.status = FlowStepRunStatus::Skipped;
                    }
                }
                run.finished_at = Some(chrono::Utc::now().to_rfc3339());
                // A failed final write leaves a leased running record; reads recover it as interrupted.
                let _ = service.persist_run(&run).await;
            });
        }
        Ok(response)
    }
}
