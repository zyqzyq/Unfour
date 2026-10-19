use crate::{
    expression::{invalid, predicate, resolve},
    FlowExecutor, FlowService,
};
use serde_json::{json, Value};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use unfour_core::{models::*, AppError, AppResult};

pub(crate) fn error_code(error: &AppError) -> String {
    match error {
        AppError::FlowActionFailed { source, .. } => error_code(source),
        AppError::HttpStatus(status) => format!("FLOW_HTTP_STATUS_{status}"),
        AppError::Validation(code) if code.starts_with("FLOW_") => code.clone(),
        _ => error.code().into(),
    }
}

enum Interrupt {
    Timeout,
    Cancelled(AppError),
}

enum Observed {
    Ready(AppResult<(Value, Option<String>)>),
    Abandoned(Interrupt),
}

/// How long an interrupted step may finish work that has already started.
const INTERRUPT_GRACE: Duration = Duration::from_secs(2);

impl FlowService {
    pub(crate) async fn execute_run(
        &self,
        run: &mut FlowRun,
        executor: Arc<dyn FlowExecutor>,
    ) -> AppResult<()> {
        let mut context = json!({"inputs": run.context.inputs, "steps": {}});
        let mut index = 0;
        while index < run.definition.steps.len() {
            if self.heartbeat(run).await? {
                run.status = FlowRunStatus::Cancelled;
                return Ok(());
            }
            let step = run.definition.steps[index].clone();
            run.steps[index].status = FlowStepRunStatus::Running;
            run.steps[index].started_at = Some(chrono::Utc::now().to_rfc3339());
            self.persist_run(run).await?;
            let lease = run.clone();
            let token = CancellationToken::new();
            let started = Instant::now();
            let outcome;
            {
                // Poll both SQL futures concurrently: awaiting a heartbeat inside a
                // select branch can deadlock a single-connection pool held by work.
                let heartbeat = async {
                    loop {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        match self.heartbeat(&lease).await {
                            Ok(false) => {}
                            result => {
                                return result.err().unwrap_or_else(|| invalid("FLOW_CANCELLED"))
                            }
                        }
                    }
                };
                tokio::pin!(heartbeat);
                let timeout = tokio::time::sleep(Duration::from_millis(step.timeout_ms));
                tokio::pin!(timeout);
                let work = self.execute_step(
                    run,
                    index,
                    &step,
                    &mut context,
                    executor.as_ref(),
                    token.clone(),
                );
                tokio::pin!(work);
                // A ready step result wins over a timeout or cancel observed in
                // the same poll. Otherwise the grace window still accepts a
                // result that finishes after the signal.
                outcome = loop {
                    tokio::select! {
                        biased;
                        result = &mut work => break Observed::Ready(result),
                        _ = &mut timeout => {
                            token.cancel();
                            break match tokio::time::timeout(INTERRUPT_GRACE, &mut work).await {
                                Ok(result) => {
                                    Observed::Ready(adopt_completed(Interrupt::Timeout, result))
                                }
                                Err(_) => Observed::Abandoned(Interrupt::Timeout),
                            };
                        }
                        error = &mut heartbeat => {
                            token.cancel();
                            break match tokio::time::timeout(INTERRUPT_GRACE, &mut work).await {
                                Ok(result) => Observed::Ready(adopt_completed(
                                    Interrupt::Cancelled(error),
                                    result,
                                )),
                                Err(_) => Observed::Abandoned(Interrupt::Cancelled(error)),
                            };
                        }
                    }
                };
            }
            let outcome = match outcome {
                Observed::Ready(result) => result,
                Observed::Abandoned(interrupt) => {
                    adopt_abandoned(interrupt, &step, &run.steps[index])
                }
            };
            // Drop the heartbeat future before the next write. An in-flight
            // heartbeat may otherwise retain the single available connection.
            run.steps[index].duration_ms = started.elapsed().as_millis() as u64;
            run.steps[index].next_check_at = None;
            let outcome = outcome.and_then(|(output, next)| {
                run.steps[index].output = Some(output.clone());
                if serde_json::to_vec(&run.steps)?.len() > 4_194_304 {
                    run.steps[index].output = None;
                    return Err(invalid("FLOW_HISTORY_LIMIT"));
                }
                Ok((output, next))
            });
            match outcome {
                Ok((output, next)) => {
                    run.steps[index].status = FlowStepRunStatus::Succeeded;
                    context["steps"][&step.id] = output;
                    self.persist_run(run).await?;
                    let target = next.or(step.next);
                    let next_index = match target.as_deref() {
                        Some("$end") => run.steps.len(),
                        Some(id) => run
                            .definition
                            .steps
                            .iter()
                            .position(|s| s.id == id)
                            .ok_or_else(|| invalid("FLOW_INVALID_BRANCH"))?,
                        None => index + 1,
                    };
                    for skipped in &mut run.steps[index + 1..next_index] {
                        skipped.status = FlowStepRunStatus::Skipped;
                    }
                    index = next_index;
                }
                Err(error) => {
                    let code = error_code(&error);
                    let (status, run_status) = match code.as_str() {
                        "FLOW_CANCELLED" => {
                            (FlowStepRunStatus::Cancelled, FlowRunStatus::Cancelled)
                        }
                        "FLOW_TIMEOUT" => (FlowStepRunStatus::TimedOut, FlowRunStatus::TimedOut),
                        _ => (FlowStepRunStatus::Failed, FlowRunStatus::Failed),
                    };
                    run.steps[index].status = status;
                    run.steps[index].error = Some(code.clone());
                    if let Some(details) = run.steps[index]
                        .attempts
                        .last()
                        .and_then(|a| a.output.clone())
                    {
                        run.steps[index].output = Some(details);
                        if serde_json::to_vec(&run.steps)?.len() > 4_194_304 {
                            run.steps[index].output = None;
                        }
                    }
                    if let Some(attempt) = run.steps[index].attempts.last_mut() {
                        if attempt.output.is_none() && attempt.error.is_none() {
                            attempt.error = Some(code.clone());
                            attempt.duration_ms = started.elapsed().as_millis() as u64;
                        }
                    }
                    run.status = run_status;
                    run.error = Some(code);
                    return Ok(());
                }
            }
        }
        run.status = FlowRunStatus::Succeeded;
        Ok(())
    }

    async fn execute_step(
        &self,
        run: &mut FlowRun,
        index: usize,
        step: &FlowStep,
        context: &mut Value,
        executor: &dyn FlowExecutor,
        cancel: CancellationToken,
    ) -> AppResult<(Value, Option<String>)> {
        match &step.node {
            FlowNode::Condition {
                predicate: test,
                if_true,
                if_false,
            } => {
                let matched = predicate(test, context)?;
                let output = json!({"matched": matched});
                run.steps[index].attempts.push(FlowAttempt {
                    number: 1,
                    input: resolve(&serde_json::to_value(test)?, context)?,
                    output: Some(output.clone()),
                    error: None,
                    duration_ms: 0,
                });
                Ok((
                    output,
                    Some(if matched { if_true } else { if_false }.clone()),
                ))
            }
            FlowNode::Wait { duration_ms } => {
                run.steps[index].attempts.push(FlowAttempt {
                    number: 1,
                    input: json!({"durationMs":duration_ms}),
                    output: None,
                    error: None,
                    duration_ms: 0,
                });
                self.persist_run(run).await?;
                tokio::select! {
                    biased;
                    _ = tokio::time::sleep(Duration::from_millis(*duration_ms)) => {}
                    _ = cancel.cancelled() => return Err(invalid("FLOW_CANCELLED")),
                }
                let output = json!({"waitedMs": duration_ms});
                run.steps[index].attempts[0].output = Some(output.clone());
                run.steps[index].attempts[0].duration_ms = *duration_ms;
                Ok((output, None))
            }
            FlowNode::Action { action } => Ok((
                self.attempt(run, index, action, context, executor, cancel)
                    .await??,
                None,
            )),
            FlowNode::Poll {
                probe,
                predicate: test,
                interval_ms,
                max_attempts,
            } => {
                for number in 1..=*max_attempts {
                    if cancel.is_cancelled() {
                        return Err(invalid("FLOW_CANCELLED"));
                    }
                    let output = self
                        .attempt(run, index, probe, context, executor, cancel.clone())
                        .await??;
                    // This value is replaced only AFTER a fresh probe completes.
                    let mut probe_context = context.clone();
                    probe_context["probe"] = output.clone();
                    if predicate(test, &probe_context)? {
                        return Ok((output, None));
                    }
                    if number == *max_attempts {
                        return Err(invalid("FLOW_MAX_ATTEMPTS"));
                    }
                    tokio::select! {
                        biased;
                        _ = tokio::time::sleep(Duration::from_millis(*interval_ms)) => {}
                        _ = cancel.cancelled() => return Err(invalid("FLOW_CANCELLED")),
                    }
                }
                unreachable!()
            }
            FlowNode::WaitUntil { .. } => {
                self.wait_until(run, index, step, context, executor, cancel)
                    .await
            }
        }
    }
    pub(crate) async fn attempt(
        &self,
        run: &mut FlowRun,
        index: usize,
        action: &FlowAction,
        context: &Value,
        executor: &dyn FlowExecutor,
        cancel: CancellationToken,
    ) -> AppResult<AppResult<Value>> {
        let mut resolved = action.clone();
        resolved.arguments = resolve(&action.arguments, context)?;
        let number = run.steps[index].attempts.len() as u32 + 1;
        run.steps[index].attempts.push(FlowAttempt {
            number,
            input: resolved.arguments.clone(),
            output: None,
            error: None,
            duration_ms: 0,
        });
        self.persist_run(run).await?;
        if cancel.is_cancelled() {
            return Err(invalid("FLOW_CANCELLED"));
        }
        let started = Instant::now();
        let output = executor
            .execute(
                &resolved,
                &run.resources[&run.steps[index].step_id],
                &run.context,
                cancel,
                &self.persistence,
            )
            .await;
        let record = run.steps[index]
            .attempts
            .last_mut()
            .expect("attempt exists");
        record.duration_ms = started.elapsed().as_millis() as u64;
        match &output {
            Ok(value) => {
                if serde_json::to_vec(value)?.len() > 262_144 {
                    return Err(invalid("FLOW_OUTPUT_TOO_LARGE"));
                }
                record.output = Some(value.clone());
            }
            Err(error) => {
                record.error = Some(error_code(error));
                if let AppError::FlowActionFailed { details, .. } = error {
                    if serde_json::to_vec(details)?.len() <= 262_144 {
                        record.output = Some(details.clone());
                    }
                }
            }
        }
        if serde_json::to_vec(&run.steps)?.len() > 4_194_304 {
            run.steps[index]
                .attempts
                .last_mut()
                .expect("attempt exists")
                .output = None;
            return Err(invalid("FLOW_HISTORY_LIMIT"));
        }
        self.persist_run(run).await?;
        // Keep executor failures separate from local persistence/limit errors:
        // only the former are eligible for a probe retry.
        Ok(output)
    }
}

fn adopt_completed(
    interrupt: Interrupt,
    result: AppResult<(Value, Option<String>)>,
) -> AppResult<(Value, Option<String>)> {
    match result {
        Ok(completed) => Ok(completed),
        Err(error) if error_code(&error) == "FLOW_CANCELLED" => Err(clean_interrupt(interrupt)),
        Err(error) => Err(error),
    }
}

fn adopt_abandoned(
    interrupt: Interrupt,
    step: &FlowStep,
    step_run: &FlowStepRun,
) -> AppResult<(Value, Option<String>)> {
    if let Some(output) = completed_action_output(step, step_run) {
        return Ok((output, None));
    }
    if let Some(code) = step_run
        .attempts
        .last()
        .and_then(|attempt| attempt.error.clone())
    {
        if code == "FLOW_CANCELLED" {
            return Err(clean_interrupt(interrupt));
        }
        return Err(recorded_attempt_error(&code));
    }
    // The attempt was handed to an external capability and never reported back.
    // A later retry could repeat a side effect that already happened.
    if external_attempt_open(step, step_run) {
        return Err(invalid("FLOW_OUTCOME_UNCERTAIN"));
    }
    Err(clean_interrupt(interrupt))
}

fn clean_interrupt(interrupt: Interrupt) -> AppError {
    match interrupt {
        Interrupt::Timeout => invalid("FLOW_TIMEOUT"),
        Interrupt::Cancelled(cause) => cause,
    }
}

/// Rebuild a code already stored on an attempt without turning it into
/// `VALIDATION_ERROR`. `FLOW_*` codes pass through `invalid`.
fn recorded_attempt_error(code: &str) -> AppError {
    match code {
        "API_CANCELLED" => AppError::ApiCancelled("cancelled".into()),
        "API_TIMEOUT" => AppError::ApiTimeout("timeout".into()),
        "NETWORK_ERROR" => AppError::ApiNetwork("network".into()),
        "QUERY_TIMEOUT" => AppError::Timeout("timeout".into()),
        "DATABASE_ERROR" => AppError::Database(sqlx::Error::PoolTimedOut),
        _ => invalid(code),
    }
}

fn completed_action_output(step: &FlowStep, step_run: &FlowStepRun) -> Option<Value> {
    if !matches!(step.node, FlowNode::Action { .. }) {
        return None;
    }
    let attempt = step_run.attempts.last()?;
    attempt
        .error
        .is_none()
        .then(|| attempt.output.clone())
        .flatten()
}

fn external_attempt_open(step: &FlowStep, step_run: &FlowStepRun) -> bool {
    let external = matches!(
        step.node,
        FlowNode::Action { .. } | FlowNode::Poll { .. } | FlowNode::WaitUntil { .. }
    );
    external
        && step_run
            .attempts
            .last()
            .is_some_and(|attempt| attempt.output.is_none() && attempt.error.is_none())
}
