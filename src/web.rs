use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Router, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tower_http::services::ServeDir;
use uuid::Uuid;
use chrono::Utc;

use crate::grpc::game_control::PendingCommand;
use crate::grpc::GameControlService;

#[derive(Debug, Serialize)]
struct ClientListResponse {
    clients: Vec<ClientInfo>,
}

#[derive(Debug, Serialize)]
struct ClientInfo {
    id: String,
    name: String,
    client_type: String,
    version: String,
    last_seen: i64,
    metrics: std::collections::HashMap<String, String>,
    current_command: Option<CurrentCommand>,
}

#[derive(Debug, Serialize)]
struct CurrentCommand {
    command_id: String,
    command: String,
    started_at: i64,
}

#[derive(Debug, Deserialize)]
struct SendCommandRequest {
    client_id: String,
    command: String,
    pub parameters: Option<std::collections::HashMap<String, String>>,
}

pub fn router(service: Arc<GameControlService>) -> Router {
    Router::new()
        .route("/api/clients", get(list_clients))
        .route("/api/commands", post(send_command))
        .nest_service("/static", ServeDir::new("static"))
        .fallback_service(ServeDir::new("static"))
        .with_state(service)
}

async fn list_clients(
    State(service): State<Arc<GameControlService>>,
) -> impl IntoResponse {
    let clients = service.get_clients().await;
    let client_list: Vec<_> = clients.iter().map(|(id, client)| {
        // 从客户端状态中提取当前命令信息
        let current_command = if let Some(metrics) = &client.status {
            if let (Some(cmd_id), Some(cmd), Some(started_at)) = (
                metrics.get("current_command_id"),
                metrics.get("current_command"),
                metrics.get("command_started_at")
            ) {
                Some(CurrentCommand {
                    command_id: cmd_id.clone(),
                    command: cmd.clone(),
                    started_at: started_at.parse().unwrap_or(0),
                })
            } else {
                None
            }
        } else {
            None
        };

        ClientInfo {
            id: id.clone(),
            name: client.name.clone(),
            client_type: client.client_type.clone(),
            version: client.version.clone(),
            last_seen: client.last_seen,
            metrics: client.status.clone().unwrap_or_default(),
            current_command,
        }
    }).collect();

    Json(json!({
        "success": true,
        "clients": client_list,
    }))
}

async fn send_command(
    State(service): State<Arc<GameControlService>>,
    Json(request): Json<SendCommandRequest>,
) -> impl IntoResponse {
    let command = PendingCommand {
        command_id: Uuid::new_v4().to_string(),
        command: request.command,
        parameters: request.parameters.unwrap_or_default(),
        created_at: Utc::now().timestamp(),
    };

    if let Err(e) = service.add_command(&request.client_id, command).await {
        return Json(json!({
            "success": false,
            "error": e.to_string(),
        }));
    }

    Json(json!({
        "success": true,
        "message": "Command sent successfully",
    }))
}
