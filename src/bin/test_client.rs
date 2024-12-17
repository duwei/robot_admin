use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tonic::{Request, Streaming};

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 连接到服务器
    let mut client = GameControlClient::connect("http://[::1]:50051").await?;

    // 注册客户端
    let register_request = Request::new(RegisterRequest {
        client_name: "test_client_1".to_string(),
        client_type: "game_test".to_string(),
    });

    let response = client.register(register_request).await?;
    let register_response = response.into_inner();
    println!(
        "Registration response: {} (ID: {})",
        register_response.message, register_response.client_id
    );

    let client_id = register_response.client_id;

    // 启动状态更新流
    let status_request = Request::new(StatusRequest {
        client_id: client_id.clone(),
    });

    let status_stream = client.stream_status(status_request).await?.into_inner();
    
    // 在后台处理状态流
    tokio::spawn(handle_status_stream(status_stream));

    // 模拟游戏状态
    let mut game_state = HashMap::new();
    game_state.insert("health".to_string(), "100".to_string());
    game_state.insert("position".to_string(), "0,0,0".to_string());
    game_state.insert("status".to_string(), "idle".to_string());

    println!("Client is ready to receive commands. Press Ctrl+C to exit.");
    
    // 创建一个新的客户端实例用于接收命令
    let mut command_client = GameControlClient::connect("http://[::1]:50051").await?;
    
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
    }
}
