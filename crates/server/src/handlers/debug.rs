use axum::{
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

use crate::{
    auth::{constant_time_eq, extract_debug_token},
    media_session::handle_media_session,
    state::{AppState, try_reserve_session},
};

/// Handles direct debug websocket connections.
pub(crate) async fn dev_connect(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = extract_debug_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    if !constant_time_eq(token.as_bytes(), state.debug_token.as_bytes()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    if !try_reserve_session(&state, "debug", "debug") {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }

    ws.protocols(["language-coach-debug"])
        .on_upgrade(move |socket| async move {
            handle_media_session(socket, state, "debug", "debug".to_owned()).await;
        })
}
