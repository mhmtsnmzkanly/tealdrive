use axum::{
    extract::{State, WebSocketUpgrade, ws::WebSocket},
    response::Response,
};
use crate::state::SharedState;
use crate::ws::connection::WsConnection;
use tracing::info;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: SharedState) {
    info!("New WebSocket connection established");
    let mut conn = WsConnection::new(socket, state);
    if let Err(e) = conn.run().await {
        info!("WebSocket connection closed with error: {:?}", e);
    } else {
        info!("WebSocket connection closed normally");
    }
}
