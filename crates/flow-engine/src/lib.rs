pub mod expression;
mod runner;
mod storage;
mod validation;

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
        if !input.confirm_effects {
            return Err(unfour_core::AppError::ConfirmationRequired {
                message: "Flow may perform remote side effects; cancellation does not undo them"
                    .into(),
                details: serde_json::json!({"flowId": input.flow_id}),
            });
        }
        let definition = self.get(&input.workspace_id, &input.flow_id).await?;
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
                    status: "pending".into(),
                    duration_ms: 0,
                    attempts: vec![],
                    error: None,
                })
                .collect(),
            definition,
            context: input,
            resources: serde_json::json!({}),
            status: "running".into(),
            error: None,
            started_at: now,
            finished_at: None,
        };
        let prepared = async {
            let inputs = run
                .context
                .inputs
                .as_object()
                .ok_or_else(|| expression::invalid("FLOW_INPUTS_OBJECT_REQUIRED"))?;
            if run
                .definition
                .inputs
                .iter()
                .any(|key| !inputs.contains_key(key))
            {
                return Err(expression::invalid("FLOW_REQUIRED_INPUT_MISSING"));
            }
            if serde_json::to_vec(inputs)?.len() > 262_144
                || run
                    .context
                    .secret_input_names
                    .iter()
                    .any(|name| !inputs.contains_key(name))
            {
                return Err(expression::invalid("FLOW_INVALID_INPUTS"));
            }
            for step in &run.definition.steps {
                let (action, probe) = match &step.node {
                    FlowNode::Action { action } => (action, false),
                    FlowNode::Poll { probe, .. } => (probe, true),
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
                step.status = "skipped".into();
            }
            run.status = "validationFailed".into();
            run.error = Some(runner::error_code(&error));
            run.finished_at = Some(chrono::Utc::now().to_rfc3339());
        }
        self.insert_run(&run).await?;
        let response = self.get_run(&run.workspace_id, &run.id).await?;
        if run.status == "running" {
            let service = self.clone();
            tokio::spawn(async move {
                let outcome = service.execute_run(&mut run, executor).await;
                if let Err(error) = outcome {
                    run.status = "failed".into();
                    run.error = Some(runner::error_code(&error));
                }
                for step in &mut run.steps {
                    if step.status == "pending" {
                        step.status = "skipped".into();
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
