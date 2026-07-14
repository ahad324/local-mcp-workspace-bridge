use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
    response::sse::{Sse, Event},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use tokio::sync::{RwLock, watch};
use tokio_stream::{self as stream};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use regex::Regex;
use tower_http::cors::{Any, CorsLayer};
use tokio::process::Child as TokioChild;

#[derive(Serialize, Clone, Debug)]
pub struct LogEntry {
    pub time: u128,
    pub level: String,
    pub message: String,
}

pub struct ServerState {
    pub is_running: RwLock<bool>,
    pub ngrok_url: RwLock<Option<String>>,
    pub workspace: RwLock<String>,
    pub shutdown_tx: RwLock<Option<watch::Sender<bool>>>,
    pub ngrok_child: RwLock<Option<TokioChild>>,
    pub logs: Mutex<Vec<LogEntry>>, // Added for polling logs
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            is_running: RwLock::new(false),
            ngrok_url: RwLock::new(None),
            workspace: RwLock::new(String::new()),
            shutdown_tx: RwLock::new(None),
            ngrok_child: RwLock::new(None),
            logs: Mutex::new(Vec::new()),
        }
    }
}

// Helper to add logs safely from anywhere
pub fn add_log(state: &Arc<ServerState>, level: &str, message: &str) {
    let entry = LogEntry {
        time: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        level: level.to_string(),
        message: message.to_string(),
    };
    if let Ok(mut logs) = state.logs.lock() {
        logs.push(entry);
        if logs.len() > 200 { logs.remove(0); } // Keep memory low
    }
}

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: "2.0".to_string(), id, result: Some(result), error: None }
    }
    fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self { jsonrpc: "2.0".to_string(), id, result: None, error: Some(JsonRpcError { code, message }) }
    }
}

async fn handle_mcp_request(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let workspace = state.workspace.read().await.clone();
    
    // JSON-RPC Spec: If 'id' is missing/null, it's a notification. Do not reply.
    if req.id.is_none() {
        add_log(&state, "INFO", &format!("Notification received: {}", req.method));
        return StatusCode::NO_CONTENT.into_response(); // <-- FIXED
    }

    add_log(&state, "REQ", &format!("Method: {} | Params: {}", req.method, serde_json::to_string(&req.params).unwrap_or_default()));

    let response = match req.method.as_str() {
        "initialize" => {
            let result = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "Local Workspace Bridge", "version": "0.1.0" }
            });
            JsonRpcResponse::success(req.id, result)
        }
        "tools/list" => {
            let tools = serde_json::json!({
                "tools": [
                    { "name": "read_file", "description": "Read file content", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } },
                    { "name": "write_file", "description": "Write file content", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" }, "content": { "type": "string" } }, "required": ["path", "content"] } },
                    { "name": "list_files", "description": "List directory", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } },
                    { "name": "search_files", "description": "Search text in files", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" }, "query": { "type": "string" } }, "required": ["path", "query"] } },
                    { "name": "delete_file", "description": "Delete a file", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } }
                ]
            });
            JsonRpcResponse::success(req.id, tools)
        }
        "tools/call" => {
            if let Some(params) = req.params {
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
                
                match execute_tool(&state, &workspace, tool_name, arguments).await {
                    Ok(res) => JsonRpcResponse::success(req.id, res),
                    Err(e) => JsonRpcResponse::error(req.id, -32000, e.clone()),
                }
            } else {
                JsonRpcResponse::error(req.id, -32602, "Missing params".to_string())
            }
        }
        _ => JsonRpcResponse::error(req.id, -32601, "Method not found".to_string()),
    };

    let status = if response.error.is_some() { "ERR" } else { "RES" };
    let msg = if let Some(err) = &response.error { format!("Error: {}", err.message) } else { "Success".to_string() };
    add_log(&state, status, &msg);

    Json(response).into_response() // <-- FIXED
}

async fn execute_tool(state: &Arc<ServerState>, workspace: &str, tool_name: &str, args: Value) -> Result<Value, String> {
    match tool_name {
        "read_file" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            add_log(state, "PATH", &format!("Raw: {}", path));
            let full_path = validate_path(workspace, path)?;
            let content = tokio::fs::read_to_string(&full_path).await.map_err(|e| format!("Read failed: {}", e))?;
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": content }] }))
        }
        "write_file" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            let content = args.get("content").and_then(|c| c.as_str()).ok_or("Missing content")?;
            add_log(state, "PATH", &format!("Raw: {}", path));
            let full_path = validate_path(workspace, path)?;
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| format!("Dir creation failed: {}", e))?;
            }
            tokio::fs::write(&full_path, content).await.map_err(|e| format!("Write failed: {}", e))?;
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": "Success" }] }))
        }
        "list_files" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            add_log(state, "PATH", &format!("Raw: {}", path));
            let full_path = validate_path(workspace, path)?;
            let mut files = Vec::new();
            for entry in WalkDir::new(&full_path).max_depth(2).into_iter().filter_map(|e| e.ok()) {
                if let Ok(rel) = entry.path().strip_prefix(&full_path) {
                    if !rel.as_os_str().is_empty() { files.push(rel.to_string_lossy().to_string()); }
                }
            }
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": files.join("\n") }] }))
        }
        "search_files" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            let query = args.get("query").and_then(|q| q.as_str()).ok_or("Missing query")?;
            add_log(state, "PATH", &format!("Raw: {} | Query: {}", path, query));
            let full_path = validate_path(workspace, path)?;
            let regex = Regex::new(query).map_err(|e| e.to_string())?;
            let mut matches = Vec::new();
            for entry in WalkDir::new(&full_path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                        for (i, line) in content.lines().enumerate() {
                            if regex.is_match(line) { matches.push(format!("{}:{}:{}", entry.path().display(), i + 1, line)); }
                        }
                    }
                }
            }
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": matches.join("\n") }] }))
        }
        "delete_file" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            add_log(state, "PATH", &format!("Raw: {}", path));
            let full_path = validate_path(workspace, path)?;
            tokio::fs::remove_file(&full_path).await.map_err(|e| format!("Delete failed: {}", e))?;
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": "Deleted" }] }))
        }
        _ => Err("Unknown tool".to_string()),
    }
}

fn validate_path(workspace: &str, path: &str) -> Result<PathBuf, String> {
    let mut ws = workspace.replace('\\', "/").to_lowercase();
    if !ws.ends_with('/') { ws.push('/'); }

    // Check if it's a TRUE absolute path (e.g., C:\, G:\, or \\server\)
    let has_drive = path.len() >= 2 && path.as_bytes()[1] == b':';
    let is_unc = path.starts_with("\\\\");
    let is_true_absolute = has_drive || is_unc;

    let target = if is_true_absolute {
        PathBuf::from(path)
    } else {
        // Strip leading slashes/backslashes so /apps/backend becomes apps/backend
        let clean_path = path.trim_start_matches('/').trim_start_matches('\\');
        Path::new(workspace).join(clean_path)
    };

    // Resolve . and .. manually
    let mut resolved = PathBuf::new();
    for comp in target.components() {
        match comp {
            std::path::Component::ParentDir => { resolved.pop(); }
            std::path::Component::CurDir => {}
            _ => resolved.push(comp),
        }
    }

    // Security check
    let res_str = resolved.to_string_lossy().replace('\\', "/").to_lowercase();

    if !res_str.starts_with(&ws) && res_str != ws.trim_end_matches('/') {
        return Err(format!("Access denied: Path '{}' is outside workspace prefix '{}'", res_str, ws));
    }

    Ok(resolved)
}

async fn sse_handler() -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::iter(vec![Ok(Event::default().event("endpoint").data("/mcp"))]);
    Sse::new(stream)
}

pub async fn start_mcp_server(state: Arc<ServerState>, port: u16, mut shutdown_rx: watch::Receiver<bool>) {
    let app = Router::new()
        .route("/", get(sse_handler).post(handle_mcp_request))
        .route("/sse", get(sse_handler))
        .route("/mcp", post(handle_mcp_request))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
        Ok(l) => l,
        Err(e) => { eprintln!("Failed to bind MCP server: {}", e); return; }
    };
    
    if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(async move { let _ = shutdown_rx.changed().await; }).await {
        eprintln!("MCP server error: {}", e);
    }
}