use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{ast, sandbox, scanner, server::AppState};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

pub fn session_id() -> Uuid {
    Uuid::new_v4()
}

pub async fn handle_request(state: Arc<AppState>, request: JsonRpcRequest) -> Option<String> {
    if request.jsonrpc != "2.0" || request.id.is_none() {
        return None;
    }
    let id = request.id.clone().unwrap_or(Value::Null);
    let response = match request.method.as_str() {
        "initialize" => success(
            id,
            json!({
                "protocolVersion": "2025-03-26",
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "agentguard", "version": "0.2.0" }
            }),
        ),
        "tools/list" => success(id, tools()),
        "tools/call" => call_tool(state, id, request.params).await,
        _ => failure(
            id,
            -32601,
            format!("Method not found: {}", request.method),
            None,
        ),
    };
    serde_json::to_string(&response).ok()
}

fn tools() -> Value {
    json!({"tools": [
        {"name":"agentguard_validate_ast","description":"Validate JavaScript, TypeScript, or TSX syntax with tree-sitter.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"code":{"type":"string"}},"required":["code"]}},
        {"name":"agentguard_scan_secrets","description":"Scan source content for known credentials and high-entropy secrets.","inputSchema":{"type":"object","properties":{"content":{"type":"string"}},"required":["content"]}},
        {"name":"agentguard_sandbox_run","description":"Run a shell command in a shadow workspace and sync only successful changes.","inputSchema":{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}}
    ]})
}

async fn call_tool(state: Arc<AppState>, id: Value, params: Value) -> JsonRpcResponse {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return failure(id, -32602, "Missing tool name".to_string(), None);
    };
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    match name {
        "agentguard_validate_ast" => {
            let Some(code) = args.get("code").and_then(Value::as_str) else {
                return failure(id, -32602, "code is required".to_string(), None);
            };
            let path = args.get("path").and_then(Value::as_str);
            let language = path
                .and_then(ast::language_for_path)
                .unwrap_or(ast::SourceLanguage::TypeScript);
            match ast::validate_syntax_as(code, language) {
                Ok(()) => {
                    publish_tool_event(
                        &state,
                        "ast",
                        "passed",
                        "MCP AST validation passed",
                        path.map(str::to_owned),
                    );
                    success(
                        id,
                        json!({"content":[{"type":"text","text":"AST validation passed"}],"isError":false}),
                    )
                }
                Err(errors) => {
                    publish_tool_event(
                        &state,
                        "ast",
                        "blocked",
                        "MCP AST validation blocked",
                        path.map(str::to_owned),
                    );
                    success(
                        id,
                        json!({"content":[{"type":"text","text":errors.join("\n")}],"isError":true}),
                    )
                }
            }
        }
        "agentguard_scan_secrets" => {
            let Some(content) = args.get("content").and_then(Value::as_str) else {
                return failure(id, -32602, "content is required".to_string(), None);
            };
            let policy = state.policy.read().await.clone();
            match scanner::scan_secrets(content) {
                Ok(()) => {
                    publish_tool_event(
                        &state,
                        "secret_scan",
                        "passed",
                        "MCP secret scan passed",
                        None,
                    );
                    success(
                        id,
                        json!({"content":[{"type":"text","text":"Secret scan passed"}],"isError":false}),
                    )
                }
                Err(message) if policy.strict_secret_scanning => {
                    publish_tool_event(
                        &state,
                        "secret_scan",
                        "blocked",
                        &format!("MCP secret scan blocked: {message}"),
                        None,
                    );
                    success(
                        id,
                        json!({"content":[{"type":"text","text":message}],"isError":true}),
                    )
                }
                Err(message) => {
                    publish_tool_event(
                        &state,
                        "secret_scan",
                        "passed",
                        "Secret detected; strict scanning is disabled",
                        None,
                    );
                    success(
                        id,
                        json!({"content":[{"type":"text","text":format!("Warning: {message}; policy allowed continuation")}],"isError":false}),
                    )
                }
            }
        }
        "agentguard_sandbox_run" => {
            let Some(command) = args.get("command").and_then(Value::as_str) else {
                return failure(id, -32602, "command is required".to_string(), None);
            };
            let result = sandbox::execute_in_shadow(command, &state.workspace).await;
            match result {
                Ok(result) => {
                    let is_error = !result.success;
                    publish_tool_event(
                        &state,
                        "sandbox",
                        if is_error { "blocked" } else { "passed" },
                        "MCP sandbox execution completed",
                        None,
                    );
                    let text = match serde_json::to_string_pretty(&result) {
                        Ok(value) => value,
                        Err(_) => "sandbox result unavailable".to_string(),
                    };
                    success(
                        id,
                        json!({"content":[{"type":"text","text":text}],"isError":is_error}),
                    )
                }
                Err(error) => failure(id, -32000, error.to_string(), None),
            }
        }
        _ => failure(id, -32602, format!("Unknown tool: {name}"), None),
    }
}

fn success(id: Value, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}
fn failure(id: Value, code: i32, message: String, data: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message,
            data,
        }),
    }
}

fn publish_tool_event(
    state: &AppState,
    kind: &str,
    status: &str,
    message: &str,
    path: Option<String>,
) {
    state.publish(kind, status, message, path);
}

pub type Sessions = Arc<tokio::sync::RwLock<HashMap<Uuid, tokio::sync::mpsc::Sender<String>>>>;
