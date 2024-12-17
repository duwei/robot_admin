use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::mpsc;
use std::sync::Weak;

pub mod game_control {
    tonic::include_proto!("game_control");
}

use game_control::game_control_server::{GameControl, GameControlServer};
use game_control::{
    CommandRequest, CommandResponse, RegisterRequest, RegisterResponse, StatusRequest, StatusResponse,
};

#[derive(Debug)]
struct Client {
    name: String,
    status_tx: Option<mpsc::Sender<Result<StatusResponse, Status>>>,
}

#[derive(Debug)]
pub struct GameControlService {
    clients: Arc<RwLock<HashMap<String, Client>>>,
}

impl GameControlService {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_clients(&self) -> HashMap<String, String> {
        let clients = self.clients.read().await;
        tracing::info!("Getting client list: {:?}", clients);
        clients
            .iter()
            .map(|(id, client)| (id.clone(), client.name.clone()))
            .collect()
    }

    pub async fn remove_client(&self, client_id: &str) {
        let mut clients = self.clients.write().await;
        if clients.remove(client_id).is_some() {
            tracing::info!("Client disconnected and removed: {}", client_id);
        }
    }
}

#[tonic::async_trait]
impl GameControl for GameControlService {
    async fn send_command(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let req = request.into_inner();
        tracing::info!("Received command request for client {}: {}", req.client_id, req.command);
        
        // Check if client exists
        let clients = self.clients.read().await;
        if !clients.contains_key(&req.client_id) {
            tracing::error!("Client {} not found", req.client_id);
            return Err(Status::not_found("Client not found"));
        }

        // Here you would implement the actual command sending logic
        Ok(Response::new(CommandResponse {
            success: true,
            message: format!("Command '{}' sent to client", req.command),
        }))
    }

    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let req = request.into_inner();
        let client_id = format!("{}_{}", req.client_type, uuid::Uuid::new_v4());
        
        tracing::info!(
            "Registering new client - Name: {}, Type: {}, ID: {}",
            req.client_name,
            req.client_type,
            client_id
        );

        // Store client information
        self.clients.write().await.insert(client_id.clone(), Client {
            name: req.client_name.clone(),
            status_tx: None,
        });
        
        tracing::info!("Client registered successfully. Current clients: {:?}", 
            self.clients.read().await);

        Ok(Response::new(RegisterResponse {
            client_id,
            success: true,
            message: "Successfully registered".to_string(),
        }))
    }

    type StreamStatusStream = ReceiverStream<Result<StatusResponse, Status>>;

    async fn stream_status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<Self::StreamStatusStream>, Status> {
        let req = request.into_inner();
        tracing::info!("Received status stream request for client {}", req.client_id);
        
        // Check if client exists
        let mut clients = self.clients.write().await;
        let client = clients.get_mut(&req.client_id).ok_or_else(|| {
            tracing::error!("Client {} not found", req.client_id);
            Status::not_found("Client not found")
        })?;

        // Create a channel for status updates
        let (tx, rx) = mpsc::channel(128);
        client.status_tx = Some(tx.clone());
        
        // Create a dummy status stream (in a real implementation, this would be actual client status)
        let client_id = req.client_id.clone();
        let clients_ref = Arc::downgrade(&self.clients);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                let status = StatusResponse {
                    client_id: client_id.clone(),
                    status: "Running".to_string(),
                    metrics: HashMap::new(),
                    timestamp: chrono::Utc::now().timestamp(),
                };
                if tx.send(Ok(status)).await.is_err() {
                    // If send fails, client has disconnected
                    if let Some(clients) = clients_ref.upgrade() {
                        let service = GameControlService { clients };
                        service.remove_client(&client_id).await;
                    }
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tonic::async_trait]
impl GameControl for Arc<GameControlService> {
    async fn send_command(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        self.as_ref().send_command(request).await
    }

    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        self.as_ref().register(request).await
    }

    type StreamStatusStream = ReceiverStream<Result<StatusResponse, Status>>;

    async fn stream_status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<Self::StreamStatusStream>, Status> {
        self.as_ref().stream_status(request).await
    }
}
