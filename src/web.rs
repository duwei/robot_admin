use axum::{
    extract::State,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::services::ServeDir;

use crate::grpc::game_control::{CommandRequest, CommandResponse};
use crate::grpc::game_control::game_control_server::GameControl;
use crate::grpc::GameControlService;

#[derive(Deserialize)]
pub struct CommandData {
    client_id: String,
    command: String,
    parameters: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
pub struct ApiResponse {
    success: bool,
    message: String,
}

#[derive(Serialize)]
pub struct ClientInfo {
    client_id: String,
    client_name: String,
    client_type: String,
    max_players: u32,
    version: String,
}

#[derive(Serialize)]
pub struct ClientListResponse {
    clients: Vec<ClientInfo>,
}

pub fn create_router(game_service: Arc<GameControlService>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/command", post(send_command))
        .route("/api/clients", get(list_clients))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(game_service)
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn list_clients(
    State(game_service): State<Arc<GameControlService>>,
) -> Json<ClientListResponse> {
    let clients = game_service.get_clients().await;
    let client_list: Vec<ClientInfo> = clients
        .iter()
        .map(|(id, client)| ClientInfo {
            client_id: id.clone(),
            client_name: client.name.clone(),
            client_type: client.client_type.clone(),
            max_players: client.max_players,
            version: client.version.clone(),
        })
        .collect();
    Json(ClientListResponse {
        clients: client_list,
    })
}

async fn send_command(
    State(game_service): State<Arc<GameControlService>>,
    Json(command_data): Json<CommandData>,
) -> Json<ApiResponse> {
    let request = tonic::Request::new(CommandRequest {
        client_id: command_data.client_id,
        command: command_data.command,
        parameters: command_data.parameters,
    });

    let response = match game_service.send_command(request).await {
        Ok(response) => response.into_inner(),
        Err(status) => {
            return Json(ApiResponse {
                success: false,
                message: status.message().to_string(),
            })
        }
    };

    Json(ApiResponse {
        success: response.success,
        message: response.message,
    })
}
