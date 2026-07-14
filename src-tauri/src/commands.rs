use tauri::AppHandle;
use crate::config::{AppConfig, load_config, save_config};
use crate::mcp_server::ServerState;
use crate::ngrok_manager;
use std::sync::Arc;

#[tauri::command]
pub async fn get_config(app: AppHandle) -> Result<AppConfig, String> {
    // app is already AppHandle, so we just pass a reference to it
    load_config(&app)
}

#[tauri::command]
pub async fn save_config_cmd(app: AppHandle, config: AppConfig) -> Result<(), String> {
    save_config(&app, &config)
}

#[tauri::command]
pub async fn start_server(app: AppHandle, state: tauri::State<'_, Arc<ServerState>>) -> Result<(), String> {
    let config = load_config(&app)?;
    let mut is_running = state.is_running.write().await;
    
    if *is_running {
        return Ok(());
    }
    
    *state.workspace.write().await = config.general.workspace_folder.clone();
    *is_running = true;
    
    let state_clone = state.inner().clone();
    let port = config.mcp.server_port;
    tokio::spawn(async move {
        crate::mcp_server::start_mcp_server(state_clone, port).await;
    });

    if config.general.auto_start_ngrok && !config.ngrok.auth_token.is_empty() {
        let state_clone = state.inner().clone();
        ngrok_manager::start_ngrok(
            state_clone, 
            config.ngrok.executable_path, 
            config.mcp.server_port, 
            config.ngrok.auth_token
        ).await;
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_server(state: tauri::State<'_, Arc<ServerState>>) -> Result<(), String> {
    *state.is_running.write().await = false;
    Ok(())
}

#[tauri::command]
pub async fn get_ngrok_url(state: tauri::State<'_, Arc<ServerState>>) -> Result<Option<String>, String> {
    Ok(state.ngrok_url.read().await.clone())
}