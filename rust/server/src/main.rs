use std::sync::Arc;

use axum::Router;
use config::{AppState, ServerConfig};
use storage::Client;

mod config;
mod storage;


#[tokio::main]
async fn main() {
    let server_config = ServerConfig::from_env();

    let app = setup_router(&server_config);
    let listener = tokio::net::TcpListener::bind(server_config.server_url()).await.expect("Failed to bind to port");
    axum::serve(listener, app).await.expect("Server failed to start");
}

fn setup_router(server_config: &ServerConfig) -> Router {
    let client = Client::new(server_config.storage_uri.clone());

    let app_state = Arc::new(AppState {
        client: client,
        env: server_config.clone(),
    });

    Router::new()
        .with_state(app_state)
}