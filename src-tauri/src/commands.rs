use tauri::AppHandle;
use crate::config::{AppConfig, load_config, save_config};
use crate::mcp_server::{ServerState, LogEntry};
use crate::ngrok_manager;
use std::sync::Arc;

#[tauri::command]
pub async fn get_config(app: AppHandle) -> Result<AppConfig, String> { load_config(&app) }

#[tauri::command]
pub async fn save_config_cmd(app: AppHandle, config: AppConfig) -> Result<(), String> { save_config(&app, &config) }

// NEW COMMAND: Polls logs from the backend
#[tauri::command]
pub async fn get_logs(state: tauri::State<'_, Arc<ServerState>>) -> Result<Vec<LogEntry>, String> {
    if let Ok(mut logs) = state.logs.lock() {
        let current_logs = logs.clone();
        logs.clear(); // Clear them so we don't send duplicates
        Ok(current_logs)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn start_server(app: AppHandle, state: tauri::State<'_, Arc<ServerState>>) -> Result<(), String> {
    let config = load_config(&app)?;
    let mut is_running = state.is_running.write().await;
    if *is_running { return Ok(()); }
    
    *state.workspace.write().await = config.general.workspace_folder.clone();
    *is_running = true;
    
    let (tx, rx) = tokio::sync::watch::channel(false);
    *state.shutdown_tx.write().await = Some(tx);

    let state_clone = state.inner().clone();
    let port = config.mcp.server_port;
    tokio::spawn(async move { crate::mcp_server::start_mcp_server(state_clone, port, rx).await; });

    if config.general.auto_start_ngrok && !config.ngrok.auth_token.is_empty() {
        let state_clone = state.inner().clone();
        ngrok_manager::start_ngrok(state_clone, config.ngrok.executable_path, config.mcp.server_port, config.ngrok.auth_token).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn stop_server(state: tauri::State<'_, Arc<ServerState>>) -> Result<(), String> {
    if let Some(tx) = state.shutdown_tx.write().await.take() { let _ = tx.send(true); }
    if let Some(mut child) = state.ngrok_child.write().await.take() { let _ = child.kill().await; }
    *state.is_running.write().await = false;
    *state.ngrok_url.write().await = None;
    Ok(())
}

#[tauri::command]
pub async fn get_ngrok_url(state: tauri::State<'_, Arc<ServerState>>) -> Result<Option<String>, String> {
    Ok(state.ngrok_url.read().await.clone())
}