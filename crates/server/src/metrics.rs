use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{Router, response::IntoResponse, routing::get};
use prometheus::{
    Encoder, Histogram, HistogramOpts, IntCounterVec, IntGaugeVec, Registry, TextEncoder,
};

/// Prometheus metrics registry and instrument handles for the service.
#[derive(Clone)]
pub(crate) struct Metrics {
    registry: Registry,
    active_sessions: IntGaugeVec,
    sessions_total: IntCounterVec,
    deepgram_errors_total: IntCounterVec,
    deepgram_connect_duration_seconds: Histogram,
    telnyx_webhooks_total: IntCounterVec,
    telnyx_commands_total: IntCounterVec,
    duplicate_webhooks_total: IntCounterVec,
}

impl Metrics {
    /// Creates and registers all service metrics.
    pub(crate) fn new() -> Result<Arc<Self>> {
        let registry = Registry::new();

        let active_sessions = IntGaugeVec::new(
            prometheus::Opts::new(
                "language_coach_active_sessions",
                "Active sessions by mode and caller label",
            ),
            &["mode", "caller_last4"],
        )?;
        let sessions_total = IntCounterVec::new(
            prometheus::Opts::new(
                "language_coach_sessions_total",
                "Total sessions by mode/status/reason/caller label",
            ),
            &["mode", "status", "reason", "caller_last4"],
        )?;
        let deepgram_errors_total = IntCounterVec::new(
            prometheus::Opts::new(
                "language_coach_deepgram_errors_total",
                "Deepgram error count by reason and caller label",
            ),
            &["reason", "caller_last4"],
        )?;
        let deepgram_connect_duration_seconds = Histogram::with_opts(HistogramOpts::new(
            "language_coach_deepgram_connect_duration_seconds",
            "Deepgram connect+setup latency",
        ))?;
        let telnyx_webhooks_total = IntCounterVec::new(
            prometheus::Opts::new(
                "language_coach_telnyx_webhooks_total",
                "Telnyx webhook outcomes by event type and status",
            ),
            &["event_type", "status"],
        )?;
        let telnyx_commands_total = IntCounterVec::new(
            prometheus::Opts::new(
                "language_coach_telnyx_commands_total",
                "Telnyx command outcomes by command, status, and reason",
            ),
            &["command", "status", "reason"],
        )?;
        let duplicate_webhooks_total = IntCounterVec::new(
            prometheus::Opts::new(
                "language_coach_duplicate_webhooks_total",
                "Duplicate webhook events by event type",
            ),
            &["event_type"],
        )?;

        registry.register(Box::new(active_sessions.clone()))?;
        registry.register(Box::new(sessions_total.clone()))?;
        registry.register(Box::new(deepgram_errors_total.clone()))?;
        registry.register(Box::new(deepgram_connect_duration_seconds.clone()))?;
        registry.register(Box::new(telnyx_webhooks_total.clone()))?;
        registry.register(Box::new(telnyx_commands_total.clone()))?;
        registry.register(Box::new(duplicate_webhooks_total.clone()))?;

        Ok(Arc::new(Self {
            registry,
            active_sessions,
            sessions_total,
            deepgram_errors_total,
            deepgram_connect_duration_seconds,
            telnyx_webhooks_total,
            telnyx_commands_total,
            duplicate_webhooks_total,
        }))
    }

    /// Increments active session gauge for the given mode.
    pub(crate) fn inc_active_sessions(&self, mode: &str, caller_last4: &str) {
        self.active_sessions
            .with_label_values(&[mode, caller_last4])
            .inc();
    }

    /// Decrements active session gauge for the given mode.
    pub(crate) fn dec_active_sessions(&self, mode: &str, caller_last4: &str) {
        self.active_sessions
            .with_label_values(&[mode, caller_last4])
            .dec();
    }

    /// Increments total session counter for a terminal status.
    pub(crate) fn inc_sessions_total(
        &self,
        mode: &str,
        status: &str,
        reason: &str,
        caller_last4: &str,
    ) {
        self.sessions_total
            .with_label_values(&[mode, status, reason, caller_last4])
            .inc();
    }

    /// Increments Deepgram error counter for a reason class.
    pub(crate) fn inc_deepgram_error(&self, reason: &str, caller_last4: &str) {
        self.deepgram_errors_total
            .with_label_values(&[reason, caller_last4])
            .inc();
    }

    /// Returns a timer for Deepgram connect duration.
    pub(crate) fn observe_deepgram_connect_timer(&self) -> prometheus::HistogramTimer {
        self.deepgram_connect_duration_seconds.start_timer()
    }

    /// Increments webhook outcome counter for the given event and status.
    pub(crate) fn inc_telnyx_webhook(&self, event_type: &str, status: &str) {
        self.telnyx_webhooks_total
            .with_label_values(&[event_type, status])
            .inc();
    }

    /// Increments Telnyx command outcome counter.
    pub(crate) fn inc_telnyx_command(&self, command: &str, status: &str, reason: &str) {
        self.telnyx_commands_total
            .with_label_values(&[command, status, reason])
            .inc();
    }

    /// Increments duplicate webhook counter for an event type.
    pub(crate) fn inc_duplicate_webhook(&self, event_type: &str) {
        self.duplicate_webhooks_total
            .with_label_values(&[event_type])
            .inc();
    }

    #[cfg(test)]
    pub(crate) fn telnyx_command_count(&self, command: &str, status: &str, reason: &str) -> u64 {
        self.telnyx_commands_total
            .with_label_values(&[command, status, reason])
            .get()
    }
}

/// Builds the private metrics router for Fly scrape.
pub(crate) fn router(metrics: Arc<Metrics>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics)
}

async fn metrics_handler(
    axum::extract::State(metrics): axum::extract::State<Arc<Metrics>>,
) -> impl IntoResponse {
    let families = metrics.registry.gather();
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let encode_result = encoder.encode(&families, &mut buffer);
    if encode_result.is_err() {
        return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    match String::from_utf8(buffer).context("metrics output is not utf8") {
        Ok(payload) => payload.into_response(),
        Err(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
