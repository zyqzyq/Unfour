pub mod api;
pub mod app;
pub mod database;
pub mod diagnostics;
pub mod mcp;
pub mod secret_store;
pub mod ssh;
pub mod workspace;

pub use api::*;
pub use app::*;
pub use database::*;
pub use diagnostics::*;
pub use mcp::*;
pub use secret_store::*;
pub use ssh::*;
pub use workspace::*;

use std::future::Future;
use std::time::Instant;
use unfour_core::AppResult;

pub(crate) async fn trace_command<T>(
    command_name: &'static str,
    future: impl Future<Output = AppResult<T>>,
) -> AppResult<T> {
    let command_id = unfour_diag::new_command_id();
    let started = Instant::now();
    unfour_diag::log_command_started(command_name, &command_id);
    let result = future.await;
    match &result {
        Ok(_) => unfour_diag::log_command_completed(
            command_name,
            &command_id,
            started.elapsed().as_millis(),
        ),
        Err(error) => unfour_diag::log_command_failed(
            command_name,
            &command_id,
            started.elapsed().as_millis(),
            unfour_diag::app_error_kind(error),
        ),
    }
    result
}
