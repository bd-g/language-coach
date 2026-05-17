use std::sync::atomic::Ordering;

use axum::{extract::State, http::StatusCode, response::IntoResponse};

use crate::state::AppState;

/// Liveness probe endpoint.
pub(crate) async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

/// Readiness probe endpoint.
pub(crate) async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    if state.readiness.load(Ordering::Relaxed) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
