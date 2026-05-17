use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use ed25519_dalek::VerifyingKey;
use rand::{Rng, distr::Alphanumeric};
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::{config::AppConfig, metrics::Metrics};

/// One-time token metadata for websocket media authorization.
#[derive(Debug, Clone)]
pub(crate) struct PendingCallToken {
    pub(crate) created_at: Instant,
    pub(crate) call_control_id: String,
    pub(crate) caller_last4: String,
}

/// Processed webhook event metadata for dedupe.
#[derive(Debug, Clone)]
pub(crate) struct ProcessedWebhookEvent {
    pub(crate) processed_at: Instant,
}

/// Shared request state for all server handlers.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<AppConfig>,
    pub(crate) telnyx_key: Arc<VerifyingKey>,
    pub(crate) debug_token: Arc<String>,
    pub(crate) telnyx_api_key: Arc<String>,
    pub(crate) deepgram_api_key: Arc<String>,
    pub(crate) metrics: Arc<Metrics>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) readiness: Arc<AtomicBool>,
    pub(crate) active_sessions: Arc<AtomicUsize>,
    pub(crate) call_tokens: Arc<Mutex<HashMap<String, PendingCallToken>>>,
    pub(crate) seen_webhook_events: Arc<Mutex<HashMap<String, ProcessedWebhookEvent>>>,
}

/// Startup dependencies used to construct `AppState`.
pub(crate) struct AppStateInit {
    pub(crate) config: Arc<AppConfig>,
    pub(crate) telnyx_key: Arc<VerifyingKey>,
    pub(crate) debug_token: Arc<String>,
    pub(crate) telnyx_api_key: Arc<String>,
    pub(crate) deepgram_api_key: Arc<String>,
    pub(crate) metrics: Arc<Metrics>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) call_tokens: Arc<Mutex<HashMap<String, PendingCallToken>>>,
    pub(crate) seen_webhook_events: Arc<Mutex<HashMap<String, ProcessedWebhookEvent>>>,
}

impl AppState {
    /// Creates a new state container with active readiness and zero sessions.
    pub(crate) fn new(init: AppStateInit) -> Self {
        Self {
            config: init.config,
            telnyx_key: init.telnyx_key,
            debug_token: init.debug_token,
            telnyx_api_key: init.telnyx_api_key,
            deepgram_api_key: init.deepgram_api_key,
            metrics: init.metrics,
            http_client: init.http_client,
            readiness: Arc::new(AtomicBool::new(true)),
            active_sessions: Arc::new(AtomicUsize::new(0)),
            call_tokens: init.call_tokens,
            seen_webhook_events: init.seen_webhook_events,
        }
    }
}

/// Query args for one-time call token auth.
#[derive(Debug, Deserialize)]
pub(crate) struct MediaQuery {
    pub(crate) call_token: String,
}

/// Tries to reserve one active session slot.
pub(crate) fn try_reserve_session(state: &AppState, mode: &str, caller_last4: &str) -> bool {
    let max = state.config.server.max_active_sessions;
    let reserved = state
        .active_sessions
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            (current < max).then_some(current + 1)
        })
        .is_ok();
    if reserved {
        state.metrics.inc_active_sessions(mode, caller_last4);
    }
    reserved
}

/// Releases a reserved session slot without recording a terminal session outcome.
pub(crate) fn release_reserved_session(state: &AppState, mode: &str, caller_last4: &str) {
    state.active_sessions.fetch_sub(1, Ordering::SeqCst);
    state.metrics.dec_active_sessions(mode, caller_last4);
}

/// Creates a random token with URL-safe alphanumeric characters.
pub(crate) fn generate_token(length: usize) -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

/// Drops the active session counter when the media loop exits.
pub(crate) struct SessionGuard {
    active_sessions: Arc<AtomicUsize>,
    metrics: Arc<Metrics>,
    mode: &'static str,
    caller_last4: String,
    status: &'static str,
    reason: &'static str,
}

impl SessionGuard {
    pub(crate) fn new(
        active_sessions: Arc<AtomicUsize>,
        metrics: Arc<Metrics>,
        mode: &'static str,
        caller_last4: String,
    ) -> Self {
        Self {
            active_sessions,
            metrics,
            mode,
            caller_last4,
            status: "ended",
            reason: "normal",
        }
    }

    pub(crate) fn set_terminal(&mut self, status: &'static str, reason: &'static str) {
        self.status = status;
        self.reason = reason;
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.active_sessions.fetch_sub(1, Ordering::SeqCst);
        self.metrics
            .dec_active_sessions(self.mode, &self.caller_last4);
        self.metrics
            .inc_sessions_total(self.mode, self.status, self.reason, &self.caller_last4);
    }
}

/// Returns the final four digits of a caller number for logs and metrics.
pub(crate) fn caller_last4(number: &str) -> String {
    let digits = number
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<Vec<_>>();
    let start = digits.len().saturating_sub(4);
    digits.into_iter().skip(start).collect()
}

/// Removes expired call tokens based on configured TTL and returns removed reservations.
pub(crate) fn prune_call_tokens(
    tokens: &mut HashMap<String, PendingCallToken>,
    ttl_seconds: u64,
) -> Vec<PendingCallToken> {
    let ttl = Duration::from_secs(ttl_seconds);
    let now = Instant::now();
    let expired_keys = tokens
        .iter()
        .filter(|(_, token)| now.duration_since(token.created_at) > ttl)
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    expired_keys
        .into_iter()
        .filter_map(|key| tokens.remove(&key))
        .collect()
}

/// Removes old webhook dedupe records based on configured TTL.
pub(crate) fn prune_seen_webhooks(
    events: &mut HashMap<String, ProcessedWebhookEvent>,
    ttl_seconds: u64,
) {
    let ttl = Duration::from_secs(ttl_seconds);
    let now = Instant::now();
    events.retain(|_, event| now.duration_since(event.processed_at) <= ttl);
}
