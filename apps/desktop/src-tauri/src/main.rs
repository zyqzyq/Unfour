// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    match unfour_lib::handle_build_metadata_cli() {
        Ok(true) => {}
        Ok(false) => unfour_lib::run(),
        Err(error) => {
            eprintln!("unfour build metadata error: {error}");
            std::process::exit(2);
        }
    }
}
