use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub ngrok: NgrokConfig,
    pub mcp: McpConfig,
    pub app: AppSettings,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GeneralConfig {
    pub workspace_folder: String,
    pub server_port: u16,
    pub auto_start_server: bool,
    pub auto_start_ngrok: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NgrokConfig {
    pub executable_path: String,
    pub auth_token: String,
    pub region: String,
    pub tunnel_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpConfig {
    pub server_port: u16,
    pub server_name: String,
    pub server_description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub start_on_boot: bool,
    pub theme: String,
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                workspace_folder: String::new(),
                server_port: 3000,
                auto_start_server: true,
                auto_start_ngrok: true,
            },
            ngrok: NgrokConfig {
                executable_path: "ngrok".to_string(),
                auth_token: String::new(),
                region: String::new(),
                tunnel_name: String::new(),
            },
            mcp: McpConfig {
                server_port: 3001,
                server_name: "Local Workspace Bridge".to_string(),
                server_description: "MCP server for local workspace".to_string(),
            },
            app: AppSettings {
                start_on_boot: false,
                theme: "dark".to_string(),
                log_level: "info".to_string(),
            },
        }
    }
}

pub fn get_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    Ok(config_dir.join("config.json"))
}

pub fn load_config(app: &AppHandle) -> Result<AppConfig, String> {
    let path = get_config_path(app)?;
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(AppConfig::default())
    }
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = get_config_path(app)?;
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}