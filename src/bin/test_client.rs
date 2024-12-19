use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time;
use tonic::{transport::Channel, Request, Status};
use chrono::Utc;

use robot_admin::grpc::game_control::game_control_client::GameControlClient;
use robot_admin::grpc::game_control::{
    RegisterRequest, StatusRequest, StatusUpdate, CurrentCommand,
};

// 全局状态
#[derive(Clone)]
struct ClientState {
    client_id: Option<String>,
    current_command: Option<CurrentCommand>,
    last_update: i64,
    reconnect_count: u32,
    command_start_time: Option<i64>,
}

impl ClientState {
    fn new() -> Self {
        Self {
            client_id: None,
            current_command: None,
            last_update: 0,
            reconnect_count: 0,
            command_start_time: None,
        }
    }

    fn start_command(&mut self, command: CurrentCommand) {
        self.current_command = Some(command);
        self.command_start_time = Some(Utc::now().timestamp());
    }

    fn complete_command(&mut self) -> Option<String> {
        if let Some(cmd) = &self.current_command {
            let command_id = cmd.command_id.clone();
            self.current_command = None;
            self.command_start_time = None;
            Some(command_id)
        } else {
            None
        }
    }

    fn is_command_finished(&self) -> bool {
        if let Some(start_time) = self.command_start_time {
            // 假设每个命令执行5秒
            Utc::now().timestamp() - start_time >= 5
        } else {
            false
        }
    }
}

async fn connect_with_retry() -> Result<GameControlClient<Channel>, Box<dyn std::error::Error + Send + Sync>> {
    let mut retry_count = 0;
    let max_retries = 10; // 最多重试10次
    let mut delay = Duration::from_secs(1);

    loop {
        match GameControlClient::connect("http://127.0.0.1:50051").await {
            Ok(client) => {
                println!("Successfully connected to server");
                return Ok(client);
            }
            Err(e) => {
                retry_count += 1;
                if retry_count >= max_retries {
                    return Err(Box::new(e));
                }
                println!("Failed to connect (attempt {}/{}): {}", retry_count, max_retries, e);
                println!("Retrying in {} seconds...", delay.as_secs());
                tokio::time::sleep(delay).await;
                // 指数退避，但最多等待30秒
                delay = std::cmp::min(delay * 2, Duration::from_secs(30));
            }
        }
    }
}

async fn register_client(client: &mut GameControlClient<Channel>) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let request = Request::new(RegisterRequest {
        client_name: "Test Client".to_string(),
        client_type: "Test".to_string(),
        version: "1.0.0".to_string(),
        max_players: 1000,
    });

    println!("\n[Sending Register Request] ----------------------------------------");
    println!("Name: Test Client");
    println!("Type: Test");
    println!("Version: 1.0.0");
    println!("Max Players: 1000");

    let response = client.register(request).await?;
    let response = response.into_inner();
    
    println!("\n[Register Response] ----------------------------------------");
    println!("Success: {}", response.success);
    println!("Client ID: {}", response.client_id);
    println!("Message: {}", response.message);
    
    Ok(response.client_id)
}

async fn update_status(
    client: &mut GameControlClient<Channel>,
    client_id: &str,
    current_command: Option<&CurrentCommand>,
    completed_command_id: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut metrics = HashMap::new();
    metrics.insert("status".to_string(), "running".to_string());
    metrics.insert("memory_usage".to_string(), "128MB".to_string());
    metrics.insert("cpu_usage".to_string(), "25%".to_string());

    // 如果有已完成的命令，添加到指标中
    if let Some(command_id) = completed_command_id {
        println!("\n[Command Completed] ----------------------------------------");
        println!("Command ID: {}", command_id);
        metrics.insert("completed_command_id".to_string(), command_id);
    }

    // 如果有当前命令，添加命令相关的指标
    if let Some(cmd) = current_command {
        metrics.insert("current_command_id".to_string(), cmd.command_id.clone());
        metrics.insert("current_command".to_string(), cmd.command.clone());
        metrics.insert("command_started_at".to_string(), cmd.started_at.to_string());
        
        println!("\n[Executing Command] ----------------------------------------");
        println!("Command ID: {}", cmd.command_id);
        println!("Command: {}", cmd.command);
        println!("Started At: {}", cmd.started_at);
        
        // 添加命令参数
        for (key, value) in &cmd.parameters {
            let param_key = format!("parameter_{}", key);
            println!("Parameter {}: {}", key, value);
            metrics.insert(param_key.clone(), value.clone());
        }
    }

    let request = Request::new(StatusUpdate {
        client_id: client_id.to_string(),
        metrics: metrics.clone(),
    });

    println!("\n[Sending Status Update] ----------------------------------------");
    println!("Client ID: {}", client_id);
    println!("Status: {}", metrics.get("status").unwrap_or(&"N/A".to_string()));
    println!("Memory: {}", metrics.get("memory_usage").unwrap_or(&"N/A".to_string()));
    println!("CPU: {}", metrics.get("cpu_usage").unwrap_or(&"N/A".to_string()));
    if let Some(cmd_id) = metrics.get("current_command_id") {
        println!("Current Command ID: {}", cmd_id);
    }

    client.update_status(request).await?;
    Ok(())
}

async fn get_status(
    client: &mut GameControlClient<Channel>,
    client_id: &str,
) -> Result<Option<CurrentCommand>, Box<dyn std::error::Error + Send + Sync>> {
    let request = Request::new(StatusRequest {
        client_id: client_id.to_string(),
    });

    println!("\n[Requesting Status] ----------------------------------------");
    println!("Client ID: {}", client_id);

    let response = client.get_status(request).await?;
    let response = response.into_inner();
    
    if let Some(cmd) = &response.current_command {
        println!("\n[Received Command] ----------------------------------------");
        println!("Command ID: {}", cmd.command_id);
        println!("Command: {}", cmd.command);
        println!("Started At: {}", cmd.started_at);
        println!("Parameters:");
        for (key, value) in &cmd.parameters {
            println!("  {}: {}", key, value);
        }
    }
    
    Ok(response.current_command)
}

async fn handle_error(
    error: Box<dyn std::error::Error + Send + Sync>,
    client: &mut GameControlClient<Channel>,
    state: &Arc<Mutex<ClientState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 检查是否是连接错误
    if error.to_string().contains("transport error") || error.to_string().contains("connection refused") {
        println!("\n[Connection Error] ----------------------------------------");
        println!("Lost connection to server, attempting to reconnect...");
        
        // 尝试重新连接
        match connect_with_retry().await {
            Ok(new_client) => {
                *client = new_client;
                // 重新注册
                match register_client(client).await {
                    Ok(new_client_id) => {
                        let mut state = state.lock().unwrap();
                        state.client_id = Some(new_client_id.clone());
                        println!("Successfully reconnected and registered with new client ID: {}", new_client_id);
                        Ok(())
                    }
                    Err(e) => {
                        println!("Failed to re-register: {}", e);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                println!("Failed to reconnect: {}", e);
                Err(e)
            }
        }
    }
    // 检查是否是 "Client not found" 错误
    else if let Some(status) = error.downcast_ref::<Status>() {
        if status.code() == tonic::Code::NotFound && status.message() == "Client not found" {
            println!("\n[Reconnecting] ----------------------------------------");
            println!("Server does not recognize client, re-registering...");
            
            // 重新注册
            match register_client(client).await {
                Ok(new_client_id) => {
                    let mut state = state.lock().unwrap();
                    state.client_id = Some(new_client_id.clone());
                    println!("Successfully re-registered with new client ID: {}", new_client_id);
                    Ok(())
                }
                Err(e) => {
                    println!("Failed to re-register: {}", e);
                    Err(e)
                }
            }
        } else {
            Err(error)
        }
    } else {
        Err(error)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("\n[Starting Test Client] ----------------------------------------");
    println!("Connecting to server at http://127.0.0.1:50051");

    let state = Arc::new(Mutex::new(ClientState::new()));
    
    // 连接到服务器（带重试）
    let mut client = connect_with_retry().await?;
    
    // 注册客户端
    let client_id = register_client(&mut client).await?;
    state.lock().unwrap().client_id = Some(client_id.clone());
    
    // 创建两个客户端实例，一个用于状态更新，一个用于命令处理
    let status_client = connect_with_retry().await?;
    let command_client = connect_with_retry().await?;
    
    println!("\n[Starting Status Update Loop] ----------------------------------------");
    // 启动状态更新循环
    let update_state = state.clone();
    let status_handle = tokio::spawn(async move {
        let mut client = status_client;
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            
            let client_id = {
                let state = update_state.lock().unwrap();
                state.client_id.clone()
            };
            
            if let Some(client_id) = client_id {
                // 获取命令状态
                match get_status(&mut client, &client_id).await {
                    Ok(command) => {
                        // 在获取锁的作用域内更新状态
                        let mut state = update_state.lock().unwrap();
                        if command.is_some() && state.current_command.is_none() {
                            state.start_command(command.unwrap());
                        }
                        state.last_update = Utc::now().timestamp();
                    }
                    Err(e) => {
                        // 处理错误，如果是连接错误或 "Client not found" 错误则重新连接或注册
                        if let Err(e) = handle_error(e, &mut client, &update_state).await {
                            println!("\n[Error] ----------------------------------------");
                            println!("Failed to get status: {}", e);
                            let mut state = update_state.lock().unwrap();
                            state.reconnect_count += 1;
                        }
                    }
                }
            }
            
            // 短暂休眠以避免过于频繁的请求
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    
    println!("\n[Starting Command Processing Loop] ----------------------------------------");
    // 启动命令处理循环
    let command_state = state;
    let command_handle = tokio::spawn(async move {
        let mut client = command_client;
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            
            let (client_id, current_command, completed_command_id) = {
                let mut state = command_state.lock().unwrap();
                let client_id = state.client_id.clone();
                let completed_command_id = if state.is_command_finished() {
                    state.complete_command()
                } else {
                    None
                };
                (client_id, state.current_command.clone(), completed_command_id)
            };
            
            if let Some(client_id) = client_id {
                // 发送状态更新，此时已经释放了锁
                if let Err(e) = update_status(
                    &mut client,
                    &client_id,
                    current_command.as_ref(),
                    completed_command_id,
                ).await {
                    // 处理错误，如果是连接错误或 "Client not found" 错误则重新连接或注册
                    if let Err(e) = handle_error(e, &mut client, &command_state).await {
                        println!("\n[Error] ----------------------------------------");
                        println!("Failed to update status: {}", e);
                    }
                }
            }
            
            // 短暂休眠以避免过于频繁的请求
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    
    println!("\n[Ready] ----------------------------------------");
    println!("Press Ctrl+C to exit...");
    
    // 等待两个任务完成或者用户按下 Ctrl+C
    tokio::select! {
        status_result = status_handle => {
            if let Err(e) = status_result {
                println!("Status update loop ended with error: {}", e);
            } else {
                println!("Status update loop ended");
            }
        }
        command_result = command_handle => {
            if let Err(e) = command_result {
                println!("Command processing loop ended with error: {}", e);
            } else {
                println!("Command processing loop ended");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n[Shutting down] ----------------------------------------");
            println!("Received Ctrl+C, shutting down...");
        }
    }
    
    Ok(())
}
