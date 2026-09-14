//! Per-call cancellation carried across the synchronous adapter boundary.
use std::{cell::RefCell, future::Future, time::Duration};
use tokio::sync::watch;
use unfour_core::{AppError, AppResult};

pub(crate) const CALL_TIMEOUT: Duration = Duration::from_secs(120);
thread_local! { static CANCEL: RefCell<Option<watch::Receiver<bool>>> = const { RefCell::new(None) }; }

pub(crate) fn is_cancelled() -> bool {
    CANCEL.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|receiver| *receiver.borrow())
    })
}

pub(crate) fn with_cancellation<T>(receiver: watch::Receiver<bool>, f: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            CANCEL.with(|c| *c.borrow_mut() = None);
        }
    }
    CANCEL.with(|c| *c.borrow_mut() = Some(receiver));
    let _reset = Reset;
    f()
}

pub(crate) async fn bounded<T>(future: impl Future<Output = AppResult<T>>) -> AppResult<T> {
    bounded_for(future, CALL_TIMEOUT).await
}

async fn bounded_for<T>(
    future: impl Future<Output = AppResult<T>>,
    timeout: Duration,
) -> AppResult<T> {
    let receiver = CANCEL.with(|c| c.borrow().clone());
    tokio::select! {
        biased;
        _ = cancelled(receiver) => Err(AppError::Timeout("MCP execution cancelled".into())),
        result = tokio::time::timeout(timeout, future) => result.unwrap_or_else(|_| Err(AppError::Timeout("MCP execution exceeded its safety deadline".into()))),
    }
}

/// Give the existing API cancellation path a bounded opportunity to roll back
/// script mutations and clean up its execution registry before dropping it.
pub(crate) async fn cooperative<T>(
    future: impl Future<Output = AppResult<T>>,
    cancel: impl FnOnce() -> bool,
) -> AppResult<T> {
    let receiver = CANCEL.with(|c| c.borrow().clone());
    tokio::pin!(future);
    tokio::select! {
        biased;
        _ = cancelled(receiver) => {},
        _ = tokio::time::sleep(CALL_TIMEOUT) => {},
        result = &mut future => return result,
    }
    if cancel() {
        let _ = tokio::time::timeout(Duration::from_secs(2), &mut future).await;
    }
    Err(AppError::Timeout(
        "MCP execution cancelled or exceeded its safety deadline".into(),
    ))
}

async fn cancelled(receiver: Option<watch::Receiver<bool>>) {
    let Some(mut receiver) = receiver else {
        return std::future::pending().await;
    };
    loop {
        if *receiver.borrow_and_update() {
            return;
        }
        if receiver.changed().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unlimited_execution_still_has_a_safety_deadline() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result: AppResult<()> = rt.block_on(bounded_for(
            std::future::pending(),
            Duration::from_millis(10),
        ));
        assert!(matches!(result, Err(AppError::Timeout(_))));
    }

    #[test]
    fn cancellation_before_api_registration_never_starts_the_request() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (tx, rx) = watch::channel(true);
        let result: AppResult<()> = with_cancellation(rx, || {
            rt.block_on(cooperative(
                async { panic!("cancelled request must not start") },
                || false,
            ))
        });
        assert!(result.is_err());
        drop(tx);
    }
    #[test]
    fn cancellation_drops_actual_execution_future() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (tx, rx) = watch::channel(false);
        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        struct Guard(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let guard = Guard(dropped.clone());
        let result: AppResult<()> = with_cancellation(rx, || {
            rt.block_on(bounded(async move {
                let _guard = guard;
                tx.send(true).unwrap();
                std::future::pending().await
            }))
        });
        assert!(result.is_err());
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    }
}
