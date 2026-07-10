// Tauri's resource selects Common Controls v6 before the Windows test harness starts.
#[cfg(all(test, target_os = "windows"))]
#[link(name = "resource", kind = "static")]
unsafe extern "C" {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = unfour_app::UnfourAppConfig {
        edition: unfour_app::AppEdition::Community,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        ..Default::default()
    };

    unfour_app::configure_core_app(tauri::Builder::default(), config)
        .invoke_handler(unfour_app::generate_handlers![])
        .run(tauri::generate_context!())
        .expect("error while running Unfour");
}
