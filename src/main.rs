mod grpc;
mod web;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::serve;
use robot_admin::grpc::GameControlService;
use robot_admin::grpc::game_control::game_control_server::GameControlServer;
use tonic::transport::Server as TonicServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the gRPC service
    let game_service = Arc::new(GameControlService::new());
    let grpc_service = game_service.clone();

    // Start the gRPC server
    let grpc_addr = "127.0.0.1:50051".parse()?;
    println!("Starting gRPC server on {}", grpc_addr);
    
    tokio::spawn(async move {
        TonicServer::builder()
            .add_service(GameControlServer::new(grpc_service))
            .serve(grpc_addr)
            .await
            .unwrap();
    });

    // Create the web service
    let app = robot_admin::web::router(game_service);

    // Start the web server
    let web_addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Starting web server on {}", web_addr);
    
    serve(
        tokio::net::TcpListener::bind(web_addr).await?,
        app.into_make_service()
    ).await?;

    Ok(())
}
