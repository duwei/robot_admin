use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tonic::{Request, Streaming};
use rand::seq::SliceRandom;

use robot_admin::grpc::game_control::game_control_client::GameControlClient;
use robot_admin::grpc::game_control::{
    CommandResponse, RegisterRequest, StatusRequest, StatusResponse,
};

async fn handle_status_stream(
    mut stream: Streaming<StatusResponse>,
) {
    while let Ok(Some(status)) = stream.message().await {
        println!(
            "Received status update: {} - {}",
            status.client_id, status.status
        );
        
        // 打印指标
        for (key, value) in status.metrics {
            println!("  {}: {}", key, value);
        }
    }
    println!("Status stream ended");
}

async fn connect_with_retry() -> Result<GameControlClient<tonic::transport::Channel>, tonic::transport::Error> {
    let mut retry_count = 0;
    let max_retries = 5;
    let base_delay = Duration::from_secs(1);

    loop {
        match GameControlClient::connect("http://[::1]:50051").await {
            Ok(client) => {
                println!("Successfully connected to server");
                return Ok(client);
            }
            Err(e) => {
                retry_count += 1;
                if retry_count > max_retries {
                    return Err(e);
                }
                let delay = base_delay * retry_count;
                println!("Connection failed, retrying in {:?}... (attempt {}/{})", delay, retry_count, max_retries);
                sleep(delay).await;
            }
        }
    }
}

fn generate_random_name() -> String {
    let adjectives = ["Red", "Blue", "Fast", "Calm", "Bold", "Wise", "Cool", "Wild"];
    let nouns = ["Fox", "Bear", "Wolf", "Lion", "Hawk", "Tiger", "Deer", "Seal"];
    
    let mut rng = rand::thread_rng();
    let adj = adjectives.choose(&mut rng).unwrap();
    let noun = nouns.choose(&mut rng).unwrap();
    
    format!("{}_{}", adj.to_lowercase(), noun.to_lowercase())
}

async fn register_client(
    client: &mut GameControlClient<tonic::transport::Channel>,
) -> Result<String, Box<dyn std::error::Error>> {
    let register_request = Request::new(RegisterRequest {
        client_name: generate_random_name(),
        client_type: "load_test".to_string(),
        max_players: 100,
        version: env!("CARGO_PKG_VERSION").to_string(),
    });

    let response = client.register(register_request).await?;
    let register_response = response.into_inner();
    println!(
        "Registration response: {} (ID: {})",
        register_response.message, register_response.client_id
    );

    Ok(register_response.client_id)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut game_state = HashMap::new();
    game_state.insert("health".to_string(), "100".to_string());
    game_state.insert("position".to_string(), "0,0,0".to_string());
    game_state.insert("status".to_string(), "idle".to_string());

    println!("Starting client with automatic reconnection...");
    
    loop {
        // 连接服务器（带重试）
        let mut client = match connect_with_retry().await {
            Ok(client) => client,
            Err(e) => {
                println!("Failed to connect after max retries: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // 注册客户端
        let client_id = match register_client(&mut client).await {
            Ok(id) => id,
            Err(e) => {
                println!("Failed to register: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // 启动状态更新流
        let status_request = Request::new(StatusRequest {
            client_id: client_id.clone(),
        });

        let status_stream = match client.stream_status(status_request).await {
            Ok(response) => response.into_inner(),
            Err(e) => {
                println!("Failed to start status stream: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        
        // 在后台处理状态流
        let status_handle = tokio::spawn(handle_status_stream(status_stream));

        println!("Client is ready to receive commands. Press Ctrl+C to exit.");
        
        // 创建一个新的客户端实例用于接收命令
        let mut command_client = match GameControlClient::connect("http://[::1]:50051").await {
            Ok(client) => client,
            Err(e) => {
                println!("Failed to create command client: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        
        loop {
            // 每5秒更新一次状态
            sleep(Duration::from_secs(5)).await;
            
            // 更新游戏状态
            let pos = game_state.get("position").unwrap();
            let mut coords: Vec<i32> = pos.split(',').map(|s| s.parse().unwrap()).collect();
            coords[0] += 1; // 模拟移动
            game_state.insert("position".to_string(), coords.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","));
            
            println!("Current state:");
            for (key, value) in &game_state {
                println!("  {}: {}", key, value);
            }

            // 检查状态流是否已经结束
            if status_handle.is_finished() {
                println!("Status stream ended, reconnecting...");
                break;
            }
        }

        println!("Connection lost, reconnecting...");
        sleep(Duration::from_secs(1)).await;
    }
}
