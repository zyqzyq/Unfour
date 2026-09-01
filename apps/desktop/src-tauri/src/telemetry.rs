use tauri::State;
use unfour_core::AppResult;
use unfour_telemetry::{
    TelemetryConfig, TelemetryPreferences, TelemetrySendOutcome, TelemetryService,
};

#[derive(Clone)]
pub struct TelemetryAppState {
    service: TelemetryService,
}

impl TelemetryAppState {
    pub fn new(service: TelemetryService) -> Self {
        Self { service }
    }
}

pub fn compiled_config() -> AppResult<TelemetryConfig> {
    let endpoint = match env!("UNFOUR_TELEMETRY_ENDPOINT") {
        "" => None,
        value => Some(value),
    };
    TelemetryConfig::for_current_target(
        env!("CARGO_PKG_VERSION"),
        env!("UNFOUR_RELEASE_CHANNEL"),
        env!("UNFOUR_DISTRIBUTION"),
        endpoint,
    )
}

#[tauri::command]
pub async fn telemetry_get_preferences(
    state: State<'_, TelemetryAppState>,
) -> AppResult<TelemetryPreferences> {
    state.service.preferences().await
}

#[tauri::command]
pub async fn telemetry_set_enabled(
    enabled: bool,
    state: State<'_, TelemetryAppState>,
) -> AppResult<TelemetryPreferences> {
    state.service.set_enabled(enabled).await
}

#[tauri::command]
pub async fn telemetry_mark_notice_shown(
    state: State<'_, TelemetryAppState>,
) -> AppResult<TelemetryPreferences> {
    state.service.mark_notice_shown().await
}

#[tauri::command]
pub async fn telemetry_record_active(
    state: State<'_, TelemetryAppState>,
) -> AppResult<TelemetrySendOutcome> {
    Ok(state.service.record_active().await)
}
