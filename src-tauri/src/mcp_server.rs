use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
    response::sse::{Sse, Event},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::{self as stream};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use regex::Regex;
use tower_http::cors::{Any, CorsLayer};

pub struct ServerState {
    pub is_running: RwLock<bool>,
    pub ngrok_url: RwLock<Option<String>>,
    pub workspace: RwLock<String>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            is_running: RwLock::new(false),
            ngrok_url: RwLock::new(None),
            workspace: RwLock::new(String::new()),
        }
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
) -> Json<JsonRpcResponse> {
    let workspace = state.workspace.read().await.clone();
    
    match req.method.as_str() {
        "initialize" => {
            let result = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "Local Workspace Bridge", "version": "0.1.0" }
            });
            Json(JsonRpcResponse::success(req.id, result))
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
            Json(JsonRpcResponse::success(req.id, tools))
        }
        "tools/call" => {
            if let Some(params) = req.params {
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
                
                match execute_tool(&workspace, tool_name, arguments).await {
                    Ok(res) => Json(JsonRpcResponse::success(req.id, res)),
                    Err(e) => Json(JsonRpcResponse::error(req.id, -32000, e)),
                }
            } else {
                Json(JsonRpcResponse::error(req.id, -32602, "Missing params".to_string()))
            }
        }
        _ => Json(JsonRpcResponse::error(req.id, -32601, "Method not found".to_string())),
    }
}

async fn execute_tool(workspace: &str, tool_name: &str, args: Value) -> Result<Value, String> {
    match tool_name {
        "read_file" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            let full_path = validate_path(workspace, path)?;
            let content = tokio::fs::read_to_string(full_path).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": content }] }))
        }
        "write_file" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            let content = args.get("content").and_then(|c| c.as_str()).ok_or("Missing content")?;
            let full_path = validate_path(workspace, path)?;
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
            }
            tokio::fs::write(full_path, content).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": "Success" }] }))
        }
        "list_files" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            let full_path = validate_path(workspace, path)?;
            let mut files = Vec::new();
            for entry in WalkDir::new(&full_path).max_depth(2).into_iter().filter_map(|e| e.ok()) {
                if let Ok(rel) = entry.path().strip_prefix(&full_path) {
                    if !rel.as_os_str().is_empty() {
                        files.push(rel.to_string_lossy().to_string());
                    }
                }
            }
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": files.join("\n") }] }))
        }
        "search_files" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            let query = args.get("query").and_then(|q| q.as_str()).ok_or("Missing query")?;
            let full_path = validate_path(workspace, path)?;
            let regex = Regex::new(query).map_err(|e| e.to_string())?;
            let mut matches = Vec::new();
            
            for entry in WalkDir::new(&full_path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                        for (i, line) in content.lines().enumerate() {
                            if regex.is_match(line) {
                                matches.push(format!("{}:{}:{}", entry.path().display(), i + 1, line));
                            }
                        }
                    }
                }
            }
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": matches.join("\n") }] }))
        }
        "delete_file" => {
            let path = args.get("path").and_then(|p| p.as_str()).ok_or("Missing path")?;
            let full_path = validate_path(workspace, path)?;
            tokio::fs::remove_file(full_path).await.map_err(|e| e.to_string())?;
            Ok(serde_json::json!({ "content": [{ "type": "text", "text": "Deleted" }] }))
        }
        _ => Err("Unknown tool".to_string()),
    }
}

// BULLETPROOF PATH VALIDATION
fn validate_path(workspace: &str, path: &str) -> Result<PathBuf, String> {
    let workspace_path = Path::new(workspace);
    
    // 1. Normalize workspace to ensure it ends with a separator for prefix checking
    let mut ws_str = workspace_path.to_string_lossy().to_string();
    if !ws_str.ends_with('\\') && !ws_str.ends_with('/') {
        ws_str.push('\\');
    }
    let ws_lower = ws_str.to_lowercase();

    // 2. Determine target path (handles both absolute and relative paths from Grok)
    let target_path = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        workspace_path.join(path)
    };

    // 3. Resolve . and .. manually so we don't need the file to exist yet
    let mut resolved = PathBuf::new();
    for component in target_path.components() {
        match component {
            std::path::Component::ParentDir => { resolved.pop(); }
            std::path::Component::CurDir => {}
            _ => resolved.push(component),
        }
    }

    // 4. Strict prefix check to prevent directory traversal
    let res_lower = resolved.to_string_lossy().to_lowercase();
    if !res_lower.starts_with(&ws_lower) {
        return Err(format!("Access denied: Path is outside workspace"));
    }

    Ok(resolved)
}

async fn sse_handler() -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::iter(vec![Ok(Event::default().event("endpoint").data("/mcp"))]);
    Sse::new(stream)
}

pub async fn start_mcp_server(state: Arc<ServerState>, port: u16) {
    let app = Router::new()
        .route("/", get(sse_handler).post(handle_mcp_request))
        .route("/sse", get(sse_handler))
        .route("/mcp", post(handle_mcp_request))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind MCP server: {}", e);
            return;
        }
    };
    
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("MCP server error: {}", e);
    }
}