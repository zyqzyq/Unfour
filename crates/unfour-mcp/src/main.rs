use std::io::{self, BufReader};
use std::sync::Arc;
use std::time::Duration;

use unfour_mcp::{LocalCommandBusAdapter, Shutdown};

const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 0;
const MAX_IDLE_TIMEOUT_SECS: u64 = 86_400;
const IDLE_TIMEOUT_ENV: &str = "UNFOUR_MCP_IDLE_TIMEOUT_SECS";

fn main() {
    let _logging_guard = initialize_logging();

    // Unified shutdown signal shared between the stdio loop and the signal
    // handlers. The first trigger wins; every observer sees the same value.
    let shutdown = Shutdown::new();

    let stdin = io::stdin();
    let stdout = io::stdout();

    let adapter = match LocalCommandBusAdapter::default_storage() {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!(
                "unfour-mcp failed to initialize command bus: {}: {}",
                error.code, error.message
            );
            std::process::exit(1);
        }
    };

    // Install Ctrl+C / SIGTERM handlers. The stdio read is a blocking syscall
    // that cannot be interrupted from another thread, so on a signal we release
    // background tasks (bounded) and then hard-exit the whole process.
    install_signal_handlers(shutdown.clone(), adapter.clone());

    let result = unfour_mcp::run_stdio_with_adapter_and_idle_timeout(
        adapter.clone(),
        BufReader::new(stdin),
        stdout.lock(),
        idle_timeout_from_env(),
    );

    // Normal exit path: EOF on stdin or a clean client disconnect. `run_stdio_with_adapter`
    // already shut the runtime down; mark the signal for completeness.
    shutdown.trigger();

    match result {
        Ok(()) => {}
        Err(error) => {
            // A broken stdout pipe (client already gone) is an expected shutdown,
            // not a failure. The loop already returns `Ok` for it; guard here too.
            if error.kind() == io::ErrorKind::BrokenPipe {
                return;
            }
            eprintln!("unfour-mcp stdio server failed: {error}");
            std::process::exit(1);
        }
    }
}

/// Return the maximum period with no MCP protocol messages before the sidecar
/// exits. `0` disables the idle backstop; invalid values use the packaged
/// default. The cap avoids accidentally turning a typo into a practically
/// unbounded process lifetime.
fn idle_timeout_from_env() -> Option<Duration> {
    parse_idle_timeout(std::env::var(IDLE_TIMEOUT_ENV).ok().as_deref())
}

fn parse_idle_timeout(value: Option<&str>) -> Option<Duration> {
    let seconds = value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_IDLE_TIMEOUT_SECS);

    if seconds == 0 {
        None
    } else {
        Some(Duration::from_secs(seconds.min(MAX_IDLE_TIMEOUT_SECS)))
    }
}

/// Spawn a tiny dedicated tokio runtime on a thread that only waits for the
/// termination signals, so the blocking stdio loop on the main thread is never
/// disturbed. On a signal we trigger the shared [`Shutdown`] flag, perform a
/// bounded release of background tasks, then exit the whole process.
fn install_signal_handlers(shutdown: Shutdown, adapter: Arc<LocalCommandBusAdapter>) {
    #[cfg(unix)]
    {
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("unfour-mcp signal runtime");
            runtime.block_on(async {
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("install SIGTERM handler");
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = sigterm.recv() => {}
                }
                shutdown.trigger();
                adapter.shutdown();
                std::process::exit(0);
            });
        });
    }
    #[cfg(windows)]
    {
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("unfour-mcp signal runtime");
            runtime.block_on(async {
                let _ = tokio::signal::ctrl_c().await;
                shutdown.trigger();
                adapter.shutdown();
                std::process::exit(0);
            });
        });
    }
}

fn initialize_logging() -> Option<unfour_diag::LoggingGuard> {
    let paths = unfour_paths::initialize_unfour_storage().ok()?;
    let mut config = unfour_diag::LoggingConfig::oss_dev(paths.logs_dir);
    config.app_name = "unfour-mcp".to_string();
    config.version = env!("CARGO_PKG_VERSION").to_string();
    unfour_diag::init_logging(config).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_timeout_is_disabled_by_default_and_for_invalid_values() {
        assert_eq!(parse_idle_timeout(None), None);
        assert_eq!(parse_idle_timeout(Some("invalid")), None);
    }

    #[test]
    fn idle_timeout_zero_disables_backstop() {
        assert_eq!(parse_idle_timeout(Some("0")), None);
    }

    #[test]
    fn idle_timeout_is_capped() {
        assert_eq!(
            parse_idle_timeout(Some("999999")),
            Some(Duration::from_secs(MAX_IDLE_TIMEOUT_SECS))
        );
    }
}
