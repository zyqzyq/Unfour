use crate::{
    expression::{invalid, predicate},
    FlowExecutor, FlowService,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use unfour_core::{models::*, AppError, AppResult};

/// Classify typed errors, never their user-facing diagnostic text.
fn transient(error: &AppError) -> bool {
    match error {
        AppError::ApiNetwork(_) | AppError::ApiTimeout(_) | AppError::Timeout(_) => true,
        AppError::HttpStatus(status) => matches!(*status, 408 | 429 | 500..=599),
        AppError::Http(error) => error.is_timeout() || error.is_connect() || error.is_body(),
        AppError::Database(sqlx::Error::PoolTimedOut) => true,
        AppError::Database(sqlx::Error::Io(error)) => matches!(
            error.kind(),
            std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::NotConnected
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::NetworkUnreachable
                | std::io::ErrorKind::HostUnreachable
                | std::io::ErrorKind::NetworkDown
        ),
        _ => false,
    }
}

impl FlowService {
    pub(crate) async fn wait_until(
        &self,
        run: &mut FlowRun,
        index: usize,
        step: &FlowStep,
        context: &Value,
        executor: &dyn FlowExecutor,
        cancel: CancellationToken,
    ) -> AppResult<(Value, Option<String>)> {
        let FlowNode::WaitUntil {
            probe,
            success_when,
            failure_when,
            interval_ms,
            max_attempts,
            probe_error_policy,
            ..
        } = &step.node
        else {
            unreachable!()
        };
        let started = Instant::now();
        loop {
            if cancel.is_cancelled() {
                return Err(invalid("FLOW_CANCELLED"));
            }
            run.steps[index].next_check_at = None;
            let outcome = self
                .attempt(run, index, probe, context, executor, cancel.clone())
                .await?;
            let attempts = run.steps[index].attempts.len();
            run.steps[index].duration_ms = started.elapsed().as_millis() as u64;
            match outcome {
                Ok(output) => {
                    // Only fresh successful results can satisfy either predicate.
                    let mut probe_context = context.clone();
                    probe_context["probe"] = output.clone();
                    if let Some(test) = failure_when {
                        if predicate(test, &probe_context)? {
                            return Err(invalid("FLOW_FAILURE_CONDITION"));
                        }
                    }
                    if predicate(success_when, &probe_context)? {
                        return Ok((
                            json!({"result":output,"attempts":attempts,"elapsedMs":started.elapsed().as_millis() as u64}),
                            None,
                        ));
                    }
                }
                Err(error) => {
                    if cancel.is_cancelled() {
                        return Err(invalid("FLOW_CANCELLED"));
                    }
                    if *probe_error_policy != FlowProbeErrorPolicy::RetryTransientErrors
                        || !transient(&error)
                    {
                        return Err(error);
                    }
                }
            }
            if max_attempts.is_some_and(|limit| attempts >= limit as usize) {
                return Err(invalid("FLOW_MAX_ATTEMPTS"));
            }
            run.steps[index].next_check_at = Some(
                (chrono::Utc::now() + chrono::Duration::milliseconds(*interval_ms as i64))
                    .to_rfc3339(),
            );
            self.persist_run(run).await?;
            tokio::select! {
                _ = cancel.cancelled() => return Err(invalid("FLOW_CANCELLED")),
                _ = tokio::time::sleep(Duration::from_millis(*interval_ms)) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flow_transient_classification_uses_types_and_status_codes() {
        for status in [408, 429, 500, 502, 503, 599] {
            assert!(transient(&AppError::HttpStatus(status)));
        }
        for status in [400, 401, 403, 404, 409, 422, 600] {
            assert!(!transient(&AppError::HttpStatus(status)));
        }
        assert!(transient(&AppError::ApiNetwork("anything".into())));
        assert!(transient(&AppError::ApiTimeout("anything".into())));
        assert!(!transient(&AppError::Validation(
            "network timeout 503".into()
        )));
        assert!(!transient(&AppError::ApiCancelled("cancelled".into())));
        assert!(transient(&AppError::Database(sqlx::Error::PoolTimedOut)));
        assert!(transient(&AppError::Database(sqlx::Error::Io(
            std::io::Error::from(std::io::ErrorKind::ConnectionReset)
        ))));
        assert!(!transient(&AppError::Database(sqlx::Error::RowNotFound)));
        for kind in [
            std::io::ErrorKind::NetworkUnreachable,
            std::io::ErrorKind::HostUnreachable,
            std::io::ErrorKind::NetworkDown,
        ] {
            assert!(transient(&AppError::Database(sqlx::Error::Io(
                std::io::Error::from(kind)
            ))));
        }
        assert!(!transient(&AppError::Database(sqlx::Error::Io(
            std::io::Error::from(std::io::ErrorKind::PermissionDenied)
        ))));
    }
}
