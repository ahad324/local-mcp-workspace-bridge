#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod mcp_server;
mod ngrok_manager;
mod commands;

use tauri::Manager; // <-- ADDED THIS
use std::sync::Arc;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config_cmd,
            commands::start_server,
            commands::stop_server,
            commands::get_ngrok_url,
            commands::get_logs,
        ])
        .setup(|app| {
            let config = config::load_config(app.handle()).unwrap_or_default();
            app.manage(std::sync::Mutex::new(config));
            app.manage(Arc::new(mcp_server::ServerState::new()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Failed to run Tauri application");
}