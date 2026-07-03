//! Shared Tauri composition layer.
//!
//! This crate owns everything in the app shell that is edition-independent:
//! the shared plugins, the command-bus setup, the managed [`AppState`], and the
//! `commands` adapters. Each edition's binary (core `apps/desktop`, Pro
//! `apps/desktop-pro`) supplies only its own `invoke_handler!` list and
//! `generate_context!` — which are compile-time, per-binary concerns — and
//! delegates the rest to [`configure`].

pub mod commands;

use std::sync::{Arc, Mutex};
use tauri::{ipc::Channel, Manager};
use unfour_command_bus::CommandBus;
use unfour_local_storage::LocalDb;
use unfour_secret_store::SecretStore;

/// Sink for live SSH terminal output. The frontend registers a Tauri IPC
/// [`Channel`] (via the `ssh_register_terminal_channel` command); the terminal
/// output callback then streams over it. Channels ride the same reliable
/// transport as commands, unlike the event system, which stalls under the
/// high-rate emit burst of a full-screen redraw on WebView2/Windows.
pub type TerminalChannelSlot = Arc<Mutex<Option<Channel<serde_json::Value>>>>;

pub struct AppState {
    pub command_bus: CommandBus,
    pub terminal_channel: TerminalChannelSlot,
}

/// Apply the shared plugins and command-bus setup to a Tauri builder.
///
/// The caller is responsible for the per-edition tail of the chain:
/// `.invoke_handler(tauri::generate_handler![..])` and
/// `.run(tauri::generate_context!())`.
pub fn configure(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.plugin(tauri_plugin_opener::init()).setup(|app| {
        let command_bus = tauri::async_runtime::block_on(async {
            let paths = unfour_paths::initialize_unfour_storage()?;
            let db = LocalDb::connect_path(paths.database_path).await?;
            db.migrate().await?;
            CommandBus::from_db_with_secret_store(db, SecretStore::new("unfour")).await
        })?;

        let terminal_channel: TerminalChannelSlot = Arc::new(Mutex::new(None));

        #[cfg(feature = "ssh-native")]
        {
            let event_app = app.handle().clone();
            let channel_slot = terminal_channel.clone();
            command_bus.set_terminal_output_callback(std::sync::Arc::new(move |payload| {
                use tauri::Emitter;
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload) {
                    // Prefer the IPC channel (reliable under burst). Fall back
                    // to the event system only until the frontend registers
                    // its channel.
                    let sent_via_channel = channel_slot
                        .lock()
                        .ok()
                        .and_then(|guard| guard.as_ref().map(|ch| ch.send(value.clone()).is_ok()))
                        .unwrap_or(false);
                    if !sent_via_channel {
                        let _ = event_app.emit("ssh://terminal-data", value);
                    }
                }
            }));
        }

        app.manage(AppState {
            command_bus,
            terminal_channel,
        });

        Ok(())
    })
}
