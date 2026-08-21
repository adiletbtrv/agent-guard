use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{ast, sandbox, scanner};

#[derive(Clone)]
pub struct AppState {
    pub events: broadcast::Sender<AgentEvent>,
    pub workspace: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub id: Uuid,
    pub timestamp: String,
    pub kind: String,
    pub status: String,
    pub message: String,
    pub command: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AstRequest {
    code: String,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SecretRequest {
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SandboxRequest {
    command: String,
    cwd: Option<PathBuf>,
}

pub async fn serve(bind: &str, workspace: PathBuf) -> anyhow::Result<()> {
    let workspace = workspace.canonicalize()?;
    let (events, _) = broadcast::channel(256);
    let state = Arc::new(AppState { events, workspace });
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/ast", post(ast_check))
        .route("/api/secrets", post(secret_check))
        .route("/api/sandbox", post(sandbox_run))
        .route("/ws", get(websocket))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    println!("AgentGuard listening on http://{bind}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok","service":"agentguard"}))
}

async fn ast_check(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AstRequest>,
) -> impl IntoResponse {
    let language = request
        .path
        .as_deref()
        .and_then(ast::language_for_path)
        .unwrap_or(ast::SourceLanguage::TypeScript);
    let result = ast::validate_syntax_as(&request.code, language);
    let response = match result {
        Ok(()) => {
            publish(
                &state,
                "ast",
                "passed",
                "AST validation passed",
                request.path.clone(),
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({"valid":true,"errors":[]})),
            )
        }
        Err(errors) => {
            publish(
                &state,
                "ast",
                "blocked",
                &format!("AST validation blocked: {} error(s)", errors.len()),
                request.path.clone(),
            );
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"valid":false,"errors":errors})),
            )
        }
    };
    response
}

async fn secret_check(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SecretRequest>,
) -> impl IntoResponse {
    match scanner::scan_secrets(&request.content) {
        Ok(()) => {
            publish(&state, "secret_scan", "passed", "Secret scan passed", None);
            (StatusCode::OK, Json(serde_json::json!({"safe":true})))
        }
        Err(message) => {
            publish(
                &state,
                "secret_scan",
                "blocked",
                &format!("Secret found: {message}"),
                None,
            );
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"safe":false,"message":message})),
            )
        }
    }
}

async fn sandbox_run(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SandboxRequest>,
) -> impl IntoResponse {
    let cwd = request.cwd.unwrap_or_else(|| state.workspace.clone());
    publish(
        &state,
        "sandbox",
        "running",
        &format!("Running in shadow workspace: {}", request.command),
        Some(cwd.display().to_string()),
    );
    match sandbox::execute_in_shadow(&request.command, &cwd).await {
        Ok(result) => {
            let status = if result.success { "passed" } else { "blocked" };
            publish(
                &state,
                "sandbox",
                status,
                &format!(
                    "Sandbox exited with code {}; synced: {}",
                    result.exit_code, result.synced
                ),
                Some(result.shadow_path.display().to_string()),
            );
            (StatusCode::OK, Json(serde_json::json!(result)))
        }
        Err(error) => {
            publish(&state, "sandbox", "error", &error.to_string(), None);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":error.to_string()})),
            )
        }
    }
}

async fn websocket(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket_session(socket, state))
}

async fn websocket_session(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut events = state.events.subscribe();
    let send_task = tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            let Ok(payload) = serde_json::to_string(&event) else {
                continue;
            };
            if sender.send(Message::Text(payload)).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(message)) = receiver.next().await {
        if matches!(message, Message::Close(_)) {
            break;
        }
    }
    send_task.abort();
}

fn publish(state: &AppState, kind: &str, status: &str, message: &str, path: Option<String>) {
    let _ = state.events.send(AgentEvent {
        id: Uuid::new_v4(),
        timestamp: Utc::now().to_rfc3339(),
        kind: kind.to_string(),
        status: status.to_string(),
        message: message.to_string(),
        command: None,
        path,
    });
}
