use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use language_coach_protocol::telnyx::{Webhook as TelnyxWebhook, WebhookEventType};
use tracing::{error, info, warn};

use crate::{
    media_session::handle_media_session,
    state::{
        AppState, MediaQuery, PendingCallToken, ProcessedWebhookEvent, caller_last4,
        generate_token, prune_call_tokens, prune_seen_webhooks, release_reserved_session,
        try_reserve_session,
    },
    telnyx_api::{answer_call, hangup_call},
};
use tokio::time::Instant;

/// Handles authenticated Telnyx media websocket upgrades.
pub(crate) async fn telnyx_media(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<MediaQuery>,
) -> Response {
    let consumed = {
        let mut tokens = state.call_tokens.lock().await;
        for expired in prune_call_tokens(&mut tokens, state.config.telnyx.dedupe_ttl_seconds) {
            release_reserved_session(&state, "telnyx", &expired.caller_last4);
        }
        tokens.remove(&query.call_token)
    };
    let Some(token) = consumed else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    ws.on_upgrade(move |socket| async move {
        handle_media_session(socket, state, "telnyx", token.caller_last4).await;
    })
}

/// Verifies and accepts Telnyx webhook events.
pub(crate) async fn telnyx_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    if let Err(error) = verify_telnyx_signature(
        &state.telnyx_key,
        &headers,
        body.as_bytes(),
        state.config.telnyx.webhook_replay_window_seconds,
    ) {
        warn!(%error, "telnyx webhook rejected");
        state.metrics.inc_telnyx_webhook("unknown", "forbidden");
        return StatusCode::FORBIDDEN.into_response();
    }

    let webhook: TelnyxWebhook = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(error) => {
            warn!(%error, "invalid telnyx webhook JSON");
            state.metrics.inc_telnyx_webhook("unknown", "bad_request");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    {
        let mut seen = state.seen_webhook_events.lock().await;
        prune_seen_webhooks(&mut seen, state.config.telnyx.dedupe_ttl_seconds);
        if seen.contains_key(&webhook.data.id) {
            state
                .metrics
                .inc_duplicate_webhook(webhook.data.event_type.as_str());
            state
                .metrics
                .inc_telnyx_webhook(webhook.data.event_type.as_str(), "duplicate");
            return StatusCode::OK.into_response();
        }
        seen.insert(
            webhook.data.id.clone(),
            ProcessedWebhookEvent {
                processed_at: Instant::now(),
            },
        );
    }

    let outcome = match &webhook.data.event_type {
        WebhookEventType::CallInitiated => handle_call_initiated(&state, &webhook).await,
        WebhookEventType::CallHangup => handle_call_hangup(&state, &webhook).await,
        WebhookEventType::Other(_) => {
            error!(event_id = %webhook.data.id, event_type = %webhook.data.event_type, "unknown telnyx webhook event type");
            WebhookOutcome::IgnoredUnknown
        }
    };

    match outcome {
        WebhookOutcome::Processed => {
            state
                .metrics
                .inc_telnyx_webhook(webhook.data.event_type.as_str(), "processed");
            StatusCode::OK.into_response()
        }
        WebhookOutcome::IgnoredUnknown => {
            state
                .metrics
                .inc_telnyx_webhook(webhook.data.event_type.as_str(), "ignored_unknown");
            StatusCode::OK.into_response()
        }
        WebhookOutcome::Retry => {
            state
                .seen_webhook_events
                .lock()
                .await
                .remove(&webhook.data.id);
            state
                .metrics
                .inc_telnyx_webhook(webhook.data.event_type.as_str(), "retry");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebhookOutcome {
    Processed,
    IgnoredUnknown,
    Retry,
}

async fn handle_call_initiated(state: &AppState, webhook: &TelnyxWebhook) -> WebhookOutcome {
    let Some(call_control_id) = webhook
        .data
        .payload
        .as_ref()
        .and_then(|payload| payload.call_control_id.as_deref())
    else {
        warn!(event_id = %webhook.data.id, "call.initiated missing call_control_id");
        state
            .metrics
            .inc_telnyx_command("answer", "skipped", "missing_call_control_id");
        return WebhookOutcome::Processed;
    };

    let caller_number = webhook
        .data
        .payload
        .as_ref()
        .and_then(|payload| payload.from.as_deref());
    let allowlisted = caller_number.is_some_and(|number| {
        state
            .config
            .allowlist_numbers
            .iter()
            .any(|allowed| allowed == number)
    });

    let caller_last4 = caller_number.map_or_else(|| "unknown".to_owned(), caller_last4);

    if !allowlisted {
        return reject_non_allowlisted_call(state, webhook, call_control_id, &caller_last4).await;
    }

    {
        let mut tokens = state.call_tokens.lock().await;
        for expired in prune_call_tokens(&mut tokens, state.config.telnyx.dedupe_ttl_seconds) {
            release_reserved_session(state, "telnyx", &expired.caller_last4);
        }
    }

    if !try_reserve_session(state, "telnyx", &caller_last4) {
        return reject_capacity_call(state, webhook, call_control_id).await;
    }

    let token = generate_token(48);
    state.call_tokens.lock().await.insert(
        token.clone(),
        PendingCallToken {
            created_at: Instant::now(),
            call_control_id: call_control_id.to_owned(),
            caller_last4: caller_last4.clone(),
        },
    );

    let stream_url = format!(
        "{}/telnyx/media?call_token={token}",
        state.config.public_base_url.trim_end_matches('/')
    );
    let answer_result = answer_call(
        &state.http_client,
        &state.config.telnyx,
        &state.telnyx_api_key,
        call_control_id,
        &stream_url,
    )
    .await;
    match answer_result {
        Ok(()) => {
            state
                .metrics
                .inc_telnyx_command("answer", "ok", "call_initiated");
            info!(
                event_id = %webhook.data.id,
                call_control_id,
                caller_last4,
                "answered call and enabled streaming"
            );
            WebhookOutcome::Processed
        }
        Err(error) => {
            warn!(%error, call_control_id, "failed to answer call");
            state
                .metrics
                .inc_telnyx_command("answer", "error", "call_initiated");
            state.call_tokens.lock().await.remove(&token);
            release_reserved_session(state, "telnyx", &caller_last4);
            WebhookOutcome::Retry
        }
    }
}

async fn reject_non_allowlisted_call(
    state: &AppState,
    webhook: &TelnyxWebhook,
    call_control_id: &str,
    caller_last4: &str,
) -> WebhookOutcome {
    warn!(
        event_id = %webhook.data.id,
        call_control_id,
        caller_last4,
        "rejecting non-allowlisted caller"
    );
    let result = hangup_call(
        &state.http_client,
        &state.config.telnyx,
        &state.telnyx_api_key,
        call_control_id,
    )
    .await;
    record_hangup_result(
        state,
        call_control_id,
        result,
        "allowlist_reject",
        "failed to hang up rejected call",
    )
}

async fn reject_capacity_call(
    state: &AppState,
    webhook: &TelnyxWebhook,
    call_control_id: &str,
) -> WebhookOutcome {
    warn!(
        event_id = %webhook.data.id,
        call_control_id,
        "rejecting call because capacity is full"
    );
    let result = hangup_call(
        &state.http_client,
        &state.config.telnyx,
        &state.telnyx_api_key,
        call_control_id,
    )
    .await;
    record_hangup_result(
        state,
        call_control_id,
        result,
        "capacity_reject",
        "failed to hang up over-capacity call",
    )
}

fn record_hangup_result(
    state: &AppState,
    call_control_id: &str,
    result: Result<()>,
    reason: &str,
    warn_message: &str,
) -> WebhookOutcome {
    match result {
        Ok(()) => {
            state.metrics.inc_telnyx_command("hangup", "ok", reason);
            WebhookOutcome::Processed
        }
        Err(error) => {
            warn!(%error, call_control_id, "{warn_message}");
            state.metrics.inc_telnyx_command("hangup", "error", reason);
            WebhookOutcome::Retry
        }
    }
}

async fn handle_call_hangup(state: &AppState, webhook: &TelnyxWebhook) -> WebhookOutcome {
    let Some(call_control_id) = webhook
        .data
        .payload
        .as_ref()
        .and_then(|payload| payload.call_control_id.as_deref())
    else {
        info!(event_id = %webhook.data.id, "call.hangup missing call_control_id");
        return WebhookOutcome::Processed;
    };

    let mut call_tokens = state.call_tokens.lock().await;
    let before = call_tokens.len();
    call_tokens.retain(|_, token| {
        if token.call_control_id == call_control_id {
            release_reserved_session(state, "telnyx", &token.caller_last4);
            false
        } else {
            true
        }
    });
    let removed = before.saturating_sub(call_tokens.len());
    info!(
        event_id = %webhook.data.id,
        call_control_id,
        removed,
        "processed call hangup"
    );
    WebhookOutcome::Processed
}

fn verify_telnyx_signature(
    key: &VerifyingKey,
    headers: &HeaderMap,
    body: &[u8],
    replay_window_seconds: u64,
) -> Result<()> {
    let signature = headers
        .get("telnyx-signature-ed25519")
        .context("missing telnyx-signature-ed25519")?
        .to_str()
        .context("invalid telnyx-signature-ed25519")?;
    let timestamp = headers
        .get("telnyx-timestamp")
        .context("missing telnyx-timestamp")?
        .to_str()
        .context("invalid telnyx-timestamp")?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let parsed = timestamp
        .parse::<u64>()
        .context("telnyx timestamp must be a unix epoch integer")?;
    if now.abs_diff(parsed) > replay_window_seconds {
        anyhow::bail!("webhook timestamp is outside replay window");
    }

    let mut message = timestamp.as_bytes().to_vec();
    message.push(b'|');
    message.extend_from_slice(body);

    let bytes = STANDARD.decode(signature)?;
    let signature = Signature::from_slice(&bytes)?;
    key.verify(&message, &signature)
        .context("webhook signature verification failed")
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use axum::{Router, http::StatusCode, routing::post};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use ed25519_dalek::{Signer, SigningKey};
    use language_coach_protocol::telnyx::{
        Webhook as TelnyxWebhook, WebhookData, WebhookEventType, WebhookPayload,
    };
    use tokio::{net::TcpListener, sync::Mutex};

    use super::{handle_call_hangup, handle_call_initiated};
    use crate::{
        config::{AppConfig, DeepgramConfig, ServerConfig, TelnyxConfig},
        metrics::Metrics,
        state::{AppState, AppStateInit, PendingCallToken, ProcessedWebhookEvent},
    };

    #[tokio::test]
    async fn initiated_allowlisted_call_stores_token_after_successful_answer() {
        let api_server = spawn_ok_telnyx_api().await;
        assert!(api_server.is_some());
        let Some((api_base_url, api_task)) = api_server else {
            return;
        };
        let state = build_state(api_base_url, vec!["+15551234567".to_owned()], 3);
        assert!(state.is_some());
        let Some(state) = state else {
            api_task.abort();
            return;
        };
        let webhook = initiated_webhook("event-1", "call-1", Some("+15551234567"));

        handle_call_initiated(&state, &webhook).await;

        let tokens = state.call_tokens.lock().await;
        assert_eq!(tokens.len(), 1);
        let found = tokens
            .values()
            .any(|pending| pending.call_control_id == "call-1");
        assert!(found);
        assert_eq!(
            state
                .metrics
                .telnyx_command_count("answer", "ok", "call_initiated"),
            1
        );
        api_task.abort();
    }

    #[tokio::test]
    async fn initiated_rejects_non_allowlisted_without_storing_token() {
        let api_server = spawn_ok_telnyx_api().await;
        assert!(api_server.is_some());
        let Some((api_base_url, api_task)) = api_server else {
            return;
        };
        let state = build_state(api_base_url, vec!["+15550000000".to_owned()], 3);
        assert!(state.is_some());
        let Some(state) = state else {
            api_task.abort();
            return;
        };
        let webhook = initiated_webhook("event-2", "call-2", Some("+15551234567"));

        handle_call_initiated(&state, &webhook).await;

        let tokens = state.call_tokens.lock().await;
        assert!(tokens.is_empty());
        assert_eq!(
            state
                .metrics
                .telnyx_command_count("hangup", "ok", "allowlist_reject"),
            1
        );
        api_task.abort();
    }

    #[tokio::test]
    async fn initiated_rejects_at_capacity_without_storing_token() {
        let api_server = spawn_ok_telnyx_api().await;
        assert!(api_server.is_some());
        let Some((api_base_url, api_task)) = api_server else {
            return;
        };
        let state = build_state(api_base_url, vec!["+15551234567".to_owned()], 1);
        assert!(state.is_some());
        let Some(state) = state else {
            api_task.abort();
            return;
        };
        state
            .active_sessions
            .store(1, std::sync::atomic::Ordering::SeqCst);
        let webhook = initiated_webhook("event-2b", "call-2b", Some("+15551234567"));

        handle_call_initiated(&state, &webhook).await;

        let tokens = state.call_tokens.lock().await;
        assert!(tokens.is_empty());
        assert_eq!(
            state
                .metrics
                .telnyx_command_count("hangup", "ok", "capacity_reject"),
            1
        );
        api_task.abort();
    }

    #[tokio::test]
    async fn webhook_answer_failure_is_retryable_and_not_deduped() {
        let api_server = spawn_telnyx_api(StatusCode::BAD_GATEWAY).await;
        assert!(api_server.is_some());
        let Some((api_base_url, api_task)) = api_server else {
            return;
        };
        let state = build_state(api_base_url, vec!["+15551234567".to_owned()], 3);
        assert!(state.is_some());
        let Some(state) = state else {
            api_task.abort();
            return;
        };
        let webhook = initiated_webhook("event-retry", "call-retry", Some("+15551234567"));
        let body = serde_json::to_string(&webhook).unwrap_or_default();
        let headers = signed_headers(&body);

        let response =
            super::telnyx_webhook(axum::extract::State(state.clone()), headers, body).await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(state.call_tokens.lock().await.is_empty());
        assert!(state.seen_webhook_events.lock().await.is_empty());
        api_task.abort();
    }

    #[tokio::test]
    async fn hangup_removes_tokens_for_matching_call_control_id() {
        let state = build_state(
            "https://127.0.0.1:9".to_owned(),
            vec!["+15551234567".to_owned()],
            1,
        );
        assert!(state.is_some());
        let Some(state) = state else {
            return;
        };

        {
            state
                .active_sessions
                .store(2, std::sync::atomic::Ordering::SeqCst);
            let mut tokens = state.call_tokens.lock().await;
            tokens.insert(
                "token-a".to_owned(),
                PendingCallToken {
                    created_at: tokio::time::Instant::now(),
                    call_control_id: "call-1".to_owned(),
                    caller_last4: "4567".to_owned(),
                },
            );
            tokens.insert(
                "token-b".to_owned(),
                PendingCallToken {
                    created_at: tokio::time::Instant::now(),
                    call_control_id: "call-2".to_owned(),
                    caller_last4: "4567".to_owned(),
                },
            );
        }

        let webhook = TelnyxWebhook {
            data: WebhookData {
                id: "event-3".to_owned(),
                event_type: WebhookEventType::CallHangup,
                payload: Some(WebhookPayload {
                    call_control_id: Some("call-1".to_owned()),
                    from: None,
                }),
            },
        };

        handle_call_hangup(&state, &webhook).await;

        let tokens = state.call_tokens.lock().await;
        assert_eq!(tokens.len(), 1);
        let only_remaining = tokens
            .values()
            .all(|pending| pending.call_control_id == "call-2");
        assert!(only_remaining);
    }

    fn initiated_webhook(
        event_id: &str,
        call_control_id: &str,
        from: Option<&str>,
    ) -> TelnyxWebhook {
        TelnyxWebhook {
            data: WebhookData {
                id: event_id.to_owned(),
                event_type: WebhookEventType::CallInitiated,
                payload: Some(WebhookPayload {
                    call_control_id: Some(call_control_id.to_owned()),
                    from: from.map(std::string::ToString::to_string),
                }),
            },
        }
    }

    fn build_state(
        api_base_url: String,
        allowlist_numbers: Vec<String>,
        max_sessions: usize,
    ) -> Option<AppState> {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let telnyx_key = Arc::new(signing_key.verifying_key());
        let metrics = Metrics::new().ok()?;

        let server = ServerConfig {
            max_active_sessions: max_sessions,
            ..ServerConfig::default()
        };

        let telnyx = TelnyxConfig {
            api_base_url,
            ..TelnyxConfig::default()
        };

        Some(AppState::new(AppStateInit {
            config: Arc::new(AppConfig {
                public_base_url: "https://language-coach.example.com".to_owned(),
                telnyx_public_key: "unused".to_owned(),
                allowlist_numbers,
                server,
                telnyx,
                deepgram: DeepgramConfig::default(),
            }),
            telnyx_key,
            debug_token: Arc::new("debug-token".to_owned()),
            telnyx_api_key: Arc::new("telnyx-token".to_owned()),
            deepgram_api_key: Arc::new("deepgram-token".to_owned()),
            metrics,
            http_client: reqwest::Client::new(),
            call_tokens: Arc::new(Mutex::new(HashMap::new())),
            seen_webhook_events: Arc::new(Mutex::new(
                HashMap::<String, ProcessedWebhookEvent>::new(),
            )),
        }))
    }

    async fn spawn_ok_telnyx_api() -> Option<(String, tokio::task::JoinHandle<()>)> {
        spawn_telnyx_api(StatusCode::OK).await
    }

    async fn spawn_telnyx_api(status: StatusCode) -> Option<(String, tokio::task::JoinHandle<()>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
        let addr = listener.local_addr().ok()?;
        let app = Router::new().route(
            "/calls/{call_control_id}/actions/{command}",
            post(move || async move { status }),
        );
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Some((format!("http://{addr}"), task))
    }

    fn signed_headers(body: &str) -> axum::http::HeaderMap {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map_or(0, |duration| duration.as_secs())
            .to_string();
        let mut message = timestamp.as_bytes().to_vec();
        message.push(b'|');
        message.extend_from_slice(body.as_bytes());
        let signature = signing_key.sign(&message);

        let mut headers = axum::http::HeaderMap::new();
        if let Ok(value) = axum::http::HeaderValue::from_str(&timestamp) {
            headers.insert("telnyx-timestamp", value);
        }
        if let Ok(value) = axum::http::HeaderValue::from_str(&STANDARD.encode(signature.to_bytes()))
        {
            headers.insert("telnyx-signature-ed25519", value);
        }
        headers
    }
}
