mod grpc;
mod web;

use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::grpc::game_control::game_control_server::GameControlServer;
use crate::grpc::GameControlService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create and share the game control service
    let game_service = Arc::new(GameControlService::new());
    let game_service_grpc = game_service.clone();

    // Start the gRPC server
    let grpc_addr = "[::1]:50051".parse().unwrap();
    let grpc_service = GameControlServer::new(game_service_grpc);
    
    tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(grpc_service)
            .serve(grpc_addr)
            .await
        {
            eprintln!("gRPC server error: {}", e);
        }
    });

    tracing::info!("gRPC server listening on {}", grpc_addr);

    // Start the web server
    let web_addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let app = web::create_router(game_service);

    tracing::info!("Web server listening on {}", web_addr);
    axum::serve(
        tokio::net::TcpListener::bind(web_addr).await?,
        app.into_make_service(),
    )
    .await?;

    Ok(())
}
