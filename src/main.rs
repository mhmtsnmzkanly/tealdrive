mod auth;
mod fs;
mod policy;
mod ws;
mod protocol;
mod api;
mod worker;
mod audit;
mod config;
mod state;
mod errors;

use axum::{Router, routing::get};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "tealdrive=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("TealDrive backend starting...");

    let state = std::sync::Arc::new(state::AppState::new());

    // Build our application with a route
    let app = Router::new()
        .route("/", get(|| async { "TealDrive API" }))
        .route("/api/auth/login", axum::routing::post(api::auth::login))
        .route("/ws", get(ws::handlers::ws_handler))
        .with_state(state);

    // Run it
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
