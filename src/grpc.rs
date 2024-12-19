use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tonic::{Request, Response, Status};
use uuid::Uuid;
use chrono::Utc;

pub mod game_control {
    tonic::include_proto!("game_control");
}

use game_control::game_control_server::GameControl;
use game_control::{
    RegisterRequest, RegisterResponse, StatusRequest, StatusResponse, StatusUpdate,
    StatusUpdateResponse, PendingCommand, CommandRequest, CommandResponse,
};

#[derive(Debug, Clone)]
pub struct Client {
    pub name: String,
    pub client_type: String,
    pub version: String,
    pub status: Option<HashMap<String, String>>,
    pub last_seen: i64,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub status: CommandStatus,
    pub parameters: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandStatus {
    Pending,
    Delivered,
    Completed,
}

pub struct GameControlService {
    clients: Arc<RwLock<HashMap<String, Client>>>,
    commands: Arc<RwLock<HashMap<String, Command>>>,
}

impl GameControlService {
    pub fn new() -> Self {
        let service = Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            commands: Arc::new(RwLock::new(HashMap::new())),
        };

        // 启动一个后台任务来清理断开连接的客户端
        let clients = service.clients.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(2)).await;
                let now = Utc::now().timestamp();
                let mut clients = clients.write().await;
                let disconnected: Vec<_> = clients
                    .iter()
                    .filter(|(_, client)| now - client.last_seen > 2)
                    .map(|(id, client)| (id.clone(), client.clone()))
                    .collect();

                for (client_id, client) in disconnected {
                    println!("\n[Client Disconnected] ----------------------------------------");
                    println!("Client ID: {}", client_id);
                    println!("Name: {}", client.name);
                    println!("Type: {}", client.client_type);
                    println!("Last seen: {} seconds ago", now - client.last_seen);
                    clients.remove(&client_id);
                }
            }
        });

        service
    }

    // 获取所有客户端，用于 Web API
    pub async fn get_clients(&self) -> HashMap<String, Client> {
        self.clients.read().await.clone()
    }

    // 添加命令
    pub async fn add_command(&self, client_id: &str, command: PendingCommand) -> Result<(), Status> {
        let mut clients = self.clients.write().await;
        let mut commands = self.commands.write().await;
        
        if let Some(client) = clients.get_mut(client_id) {
            // 检查客户端是否空闲（没有当前正在执行的命令）
            if let Some(metrics) = &client.status {
                if metrics.contains_key("current_command_id") {
                    return Err(Status::failed_precondition("Client is busy processing another command"));
                }
            }
            
            // 创建新的命令
            let cmd = Command {
                status: CommandStatus::Pending,
                parameters: command.parameters.clone(),
            };
            
            // 保存命令
            commands.insert(command.command_id.clone(), cmd);
            
            // 设置客户端状态
            let mut metrics = client.status.clone().unwrap_or_default();
            metrics.insert("current_command_id".to_string(), command.command_id.clone());
            metrics.insert("current_command".to_string(), command.command.clone());
            metrics.insert("command_started_at".to_string(), Utc::now().timestamp().to_string());

            // 添加命令参数
            for (key, value) in command.parameters.iter() {
                metrics.insert(format!("parameter_{}", key), value.clone());
            }

            client.status = Some(metrics);
            println!("Command {} added to client {}", command.command, client_id);
            
            Ok(())
        } else {
            Err(Status::not_found("Client not found"))
        }
    }
}

#[tonic::async_trait]
impl GameControl for Arc<GameControlService> {
    async fn send_command(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let command = request.into_inner();
        let command_id = Uuid::new_v4().to_string();
        
        let pending_command = PendingCommand {
            command_id: command_id.clone(),
            command: command.command.clone(),
            parameters: command.parameters.clone(),
            created_at: Utc::now().timestamp(),
        };

        // 尝试添加命令
        match self.add_command(&command.client_id, pending_command).await {
            Ok(_) => {
                println!("Command {} sent to client {}", command.command, command.client_id);
                Ok(Response::new(CommandResponse {
                    success: true,
                    message: "Command accepted".to_string(),
                }))
            }
            Err(e) => {
                println!("Failed to send command to client {}: {}", command.client_id, e);
                Err(e)
            }
        }
    }

    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<RegisterResponse>, Status> {
        let req = request.into_inner();
        let client_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        
        let client = Client {
            name: req.client_name,
            client_type: req.client_type,
            version: req.version,
            status: None,
            last_seen: now,
        };

        println!("\n[Client Registered] ----------------------------------------");
        println!("Client ID: {}", client_id);
        println!("Name: {}", client.name);
        println!("Type: {}", client.client_type);
        println!("Version: {}", client.version);

        self.clients.write().await.insert(client_id.clone(), client);

        Ok(Response::new(RegisterResponse {
            success: true,
            client_id,
            message: "Successfully registered".to_string(),
        }))
    }

    async fn get_status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let request = request.into_inner();
        let mut clients = self.clients.write().await;
        
        if let Some(client) = clients.get_mut(&request.client_id) {
            // 更新最后一次见到的时间
            client.last_seen = Utc::now().timestamp();
            
            // 检查是否有当前正在执行的命令
            let mut current_command = None;
            if let Some(metrics) = &client.status {
                if let (Some(cmd_id), Some(cmd), Some(started_at)) = (
                    metrics.get("current_command_id"),
                    metrics.get("current_command"),
                    metrics.get("command_started_at")
                ) {
                    // 检查命令是否已完成
                    if !metrics.contains_key("completed_command_id") || metrics.get("completed_command_id") != Some(cmd_id) {
                        let mut parameters = HashMap::new();
                        for (key, value) in metrics.iter() {
                            if key.starts_with("parameter_") {
                                parameters.insert(key[10..].to_string(), value.clone());
                            }
                        }
                        current_command = Some(game_control::CurrentCommand {
                            command_id: cmd_id.clone(),
                            command: cmd.clone(),
                            parameters,
                            started_at: started_at.parse().unwrap_or(0),
                        });
                    }
                }
            }

            Ok(Response::new(StatusResponse {
                client_id: request.client_id,
                status: client.client_type.clone(),
                metrics: client.status.clone().unwrap_or_default(),
                timestamp: Utc::now().timestamp(),
                pending_commands: Vec::new(),
                current_command,
            }))
        } else {
            Err(Status::not_found("Client not found"))
        }
    }

    async fn update_status(
        &self,
        request: Request<StatusUpdate>,
    ) -> Result<Response<StatusUpdateResponse>, Status> {
        let update = request.into_inner();
        let mut clients = self.clients.write().await;
        
        if let Some(client) = clients.get_mut(&update.client_id) {
            // 更新最后一次见到的时间
            client.last_seen = Utc::now().timestamp();
            
            // 获取或创建客户端状态
            let mut metrics = client.status.clone().unwrap_or_default();
            
            // 检查是否有命令完成的通知
            if let Some(completed_command_id) = update.metrics.get("completed_command_id") {
                // 更新命令状态
                if let Some(cmd) = self.commands.write().await.get_mut(completed_command_id) {
                    cmd.status = CommandStatus::Completed;
                    
                    // 从客户端状态中移除完成的命令
                    metrics.remove("current_command_id");
                    metrics.remove("current_command");
                    metrics.remove("command_started_at");
                    // 移除命令参数
                    let params_to_remove: Vec<_> = metrics.keys()
                        .filter(|k| k.starts_with("parameter_"))
                        .cloned()
                        .collect();
                    for key in params_to_remove {
                        metrics.remove(&key);
                    }
                }
            }

            // 更新基本指标
            for (key, value) in update.metrics.iter() {
                // 只更新非命令相关的指标
                if !key.starts_with("current_command") && 
                   !key.starts_with("parameter_") && 
                   key != "command_started_at" {
                    metrics.insert(key.clone(), value.clone());
                }
            }

            // 更新客户端状态
            client.status = Some(metrics);

            Ok(Response::new(StatusUpdateResponse {
                success: true,
                message: "Status updated".to_string(),
            }))
        } else {
            Err(Status::not_found("Client not found"))
        }
    }
}
