use std::{collections::HashMap, convert::Infallible, path::PathBuf, sync::Arc, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

use crate::{ast, mcp, policy::SecurityPolicy, sandbox, scanner};

#[derive(Clone)]
pub struct AppState {
    pub events: broadcast::Sender<AgentEvent>,
    pub workspace: PathBuf,
    pub policy: Arc<RwLock<SecurityPolicy>>,
    pub mcp_sessions: mcp::Sessions,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PolicyPatch {
    auto_approve_ast_safe: Option<bool>,
    require_human_for_deletions: Option<bool>,
    strict_secret_scanning: Option<bool>,
}

pub async fn serve(bind: &str, workspace: PathBuf) -> anyhow::Result<()> {
    let workspace = workspace.canonicalize()?;
    let (events, _) = broadcast::channel(256);
    let state = Arc::new(AppState {
        events,
        workspace,
        policy: Arc::new(RwLock::new(SecurityPolicy::default())),
        mcp_sessions: Arc::new(RwLock::new(HashMap::new())),
    });
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/ast", post(ast_check))
        .route("/api/secrets", post(secret_check))
        .route("/api/sandbox", post(sandbox_run))
        .route("/api/policy", get(policy_get).patch(policy_patch))
        .route("/ws", get(websocket))
        .route("/mcp/sse", get(mcp_sse))
        .route("/mcp/messages", post(mcp_message))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    println!("AgentGuard listening on http://{bind}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok","service":"agentguard"}))
}

async fn policy_get(State(state): State<Arc<AppState>>) -> Json<SecurityPolicy> {
    Json(state.policy.read().await.clone())
}

async fn policy_patch(
    State(state): State<Arc<AppState>>,
    Json(patch): Json<PolicyPatch>,
) -> Json<SecurityPolicy> {
    let updated = {
        let mut policy = state.policy.write().await;
        if let Some(value) = patch.auto_approve_ast_safe {
            policy.auto_approve_ast_safe = value;
        }
        if let Some(value) = patch.require_human_for_deletions {
            policy.require_human_for_deletions = value;
        }
        if let Some(value) = patch.strict_secret_scanning {
            policy.strict_secret_scanning = value;
        }
        policy.clone()
    };
    state.publish("policy", "passed", "Runtime security policy updated", None);
    Json(updated)
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
    let strict = state.policy.read().await.strict_secret_scanning;
    match scanner::scan_secrets(&request.content) {
        Ok(()) => {
            publish(&state, "secret_scan", "passed", "Secret scan passed", None);
            (StatusCode::OK, Json(serde_json::json!({"safe":true})))
        }
        Err(message) if strict => {
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
        Err(message) => {
            publish(
                &state,
                "secret_scan",
                "passed",
                &format!("Secret detected; strict scanning disabled: {message}"),
                None,
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({"safe":false,"enforced":false,"message":message})),
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
                    "Sandbox exited with code {:?}; synced: {}",
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

async fn mcp_sse(
    State(state): State<Arc<AppState>>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let session_id = mcp::session_id();
    let (sender, receiver) = mpsc::channel(32);
    state.mcp_sessions.write().await.insert(session_id, sender);
    let stream = tokio_stream::wrappers::ReceiverStream::new(receiver)
        .map(move |payload| Ok(Event::default().event("message").data(payload)));
    let endpoint = format!("/mcp/messages?sessionId={session_id}");
    let initial =
        futures_util::stream::once(
            async move { Ok(Event::default().event("endpoint").data(endpoint)) },
        );
    Sse::new(initial.chain(stream)).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn mcp_message(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
    Json(request): Json<mcp::JsonRpcRequest>,
) -> Response {
    let Some(session) = query
        .get("sessionId")
        .and_then(|value| Uuid::parse_str(value).ok())
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"sessionId is required"})),
        )
            .into_response();
    };
    let response = mcp::handle_request(state.clone(), request).await;
    let Some(payload) = response else {
        return StatusCode::ACCEPTED.into_response();
    };
    let sender = state.mcp_sessions.read().await.get(&session).cloned();
    if let Some(sender) = sender {
        if sender.send(payload.clone()).await.is_err() {
            state.mcp_sessions.write().await.remove(&session);
            return StatusCode::GONE.into_response();
        }
        return StatusCode::ACCEPTED.into_response();
    }
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error":"unknown MCP session"})),
    )
        .into_response()
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

impl AppState {
    pub fn publish(&self, kind: &str, status: &str, message: &str, path: Option<String>) {
        let _ = self.events.send(AgentEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now().to_rfc3339(),
            kind: kind.to_string(),
            status: status.to_string(),
            message: message.to_string(),
            command: None,
            path,
        });
    }
}

fn publish(state: &AppState, kind: &str, status: &str, message: &str, path: Option<String>) {
    state.publish(kind, status, message, path);
}
