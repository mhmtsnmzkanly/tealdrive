use std::net::SocketAddr;
use std::time::Duration;

use tealdrive::audit::sink::JsonlAuditSink;
use tealdrive::config::AppConfig;
use tealdrive::errors::TealDriveError;
use tealdrive::helper::client::{default_helper_binary_path, HelperProcessClient};
use tealdrive::session::InMemorySessionStore;
use tealdrive::ws::server::{run_server, WsServerState};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("tealdrive-server failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), TealDriveError> {
    let config = AppConfig::default();
    let bind_addr = config
        .server
        .bind_addr
        .parse::<SocketAddr>()
        .map_err(|_| TealDriveError::Internal)?;
    let helper_path = default_helper_binary_path();
    let helper_client = HelperProcessClient::new(helper_path, Duration::from_secs(10));
    let audit_sink = JsonlAuditSink::open_default().map_err(|_| TealDriveError::Internal)?;
    let session_store = InMemorySessionStore::new(config.session.clone());
    let state = WsServerState::new(config, session_store, helper_client, audit_sink);

    run_server(bind_addr, state).await
}
