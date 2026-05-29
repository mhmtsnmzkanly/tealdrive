use axum::{
    extract::State,
    Json,
    response::IntoResponse,
};
use crate::auth::{PamAuth, session::Session};
use crate::state::SharedState;
use serde::Deserialize;
use nix::unistd::User;
use tracing::{info, warn};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<SharedState>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    info!("Login attempt for user: {}", payload.username);

    if PamAuth::authenticate(&payload.username, &payload.password) {
        // Get Linux user info
        let user = match User::from_name(&payload.username) {
            Ok(Some(u)) => u,
            _ => {
                warn!("Authenticated user {} not found in system", payload.username);
                return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "User not found").into_response();
            }
        };

        let session = state.sessions.create_session(
            payload.username,
            user.uid.as_raw(),
            user.gid.as_raw(),
        );

        info!("Login successful, session created: {}", session.id);
        (axum::http::StatusCode::OK, Json(session)).into_response()
    } else {
        warn!("Login failed for user: {}", payload.username);
        (axum::http::StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
    }
}
