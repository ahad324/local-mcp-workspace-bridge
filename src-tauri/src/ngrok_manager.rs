use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use regex::Regex;
use std::sync::Arc;
use crate::mcp_server::ServerState;

pub async fn start_ngrok(state: Arc<ServerState>, executable: String, port: u16, auth_token: String) {
    if !auth_token.is_empty() {
        let _ = std::process::Command::new(&executable)
            .arg("config")
            .arg("add-authtoken")
            .arg(&auth_token)
            .output();
    }

    let mut child = match TokioCommand::new(&executable)
        .arg("http")
        .arg(port.to_string())
        .arg("--log=stdout")
        .arg("--log-format=logfmt")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to start ngrok: {}", e);
                return;
            }
        };

    let stdout = child.stdout.take().expect("Failed to take ngrok stdout");
    let mut reader = BufReader::new(stdout).lines();
    let url_regex = Regex::new(r"url=(https://[a-zA-Z0-9\-\.]+\.ngrok[-a-zA-Z0-9\.]*)").expect("Invalid regex");

    tokio::spawn(async move {
        while let Ok(Some(line)) = reader.next_line().await {
            if let Some(caps) = url_regex.captures(&line) {
                if let Some(url) = caps.get(1) {
                    let mut ngrok_url = state.ngrok_url.write().await;
                    *ngrok_url = Some(url.as_str().to_string());
                    break;
                }
            }
        }
        let _ = child.wait().await;
    });
}