use std::{collections::HashMap, net::SocketAddr, str::FromStr, sync::Arc};

use anyhow::{Context, Result};
use axum::{
    Router,
    routing::{get, post},
};
use clap::Parser;
use tokio::{net::TcpListener, sync::Mutex, time::sleep};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::{
    config::{Cli, load_config, parse_ed25519_key, validate_config},
    handlers::{
        debug::dev_connect,
        health::{healthz, readyz},
        telnyx::{telnyx_media, telnyx_webhook},
    },
    metrics,
    state::{AppState, AppStateInit},
};

/// Starts the HTTP server with loaded config and initialized state.
pub(crate) async fn run() -> Result<()> {
    init_tls_provider();
    init_tracing();

    let cli = Cli::parse();
    let config = load_config(&cli.config)?;
    validate_config(&config)?;
    let telnyx_key = parse_ed25519_key(&config.telnyx_public_key)?;
    let debug_token = std::env::var("DIRECT_ACCESS_TOKEN")
        .context("DIRECT_ACCESS_TOKEN must be set in environment")?;
    let deepgram_api_key =
        std::env::var("DEEPGRAM_API_KEY").context("DEEPGRAM_API_KEY must be set in environment")?;
    let telnyx_api_key =
        std::env::var("TELNYX_API_KEY").context("TELNYX_API_KEY must be set in environment")?;
    let bind_addr = SocketAddr::from_str(&config.server.bind_addr)
        .context("invalid server.bind_addr in config")?;
    let metrics_bind_addr = SocketAddr::from_str(&config.server.metrics_bind_addr)
        .context("invalid server.metrics_bind_addr in config")?;
    let shutdown_drain_seconds = config.server.shutdown_drain_seconds;
    let metrics = metrics::Metrics::new()?;

    let state = AppState::new(AppStateInit {
        config: Arc::new(config),
        telnyx_key: Arc::new(telnyx_key),
        debug_token: Arc::new(debug_token),
        telnyx_api_key: Arc::new(telnyx_api_key),
        deepgram_api_key: Arc::new(deepgram_api_key),
        metrics: metrics.clone(),
        http_client: reqwest::Client::new(),
        call_tokens: Arc::new(Mutex::new(HashMap::new())),
        seen_webhook_events: Arc::new(Mutex::new(HashMap::new())),
    });

    let metrics_app = metrics::router(metrics.clone());
    let state_for_shutdown = state.clone();

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/dev/connect", get(dev_connect))
        .route("/telnyx/media", get(telnyx_media))
        .route("/telnyx/webhook", post(telnyx_webhook))
        .with_state(state);

    info!(%bind_addr, %metrics_bind_addr, "starting server");
    let listener = TcpListener::bind(bind_addr).await?;
    let metrics_listener = TcpListener::bind(metrics_bind_addr).await?;

    let metrics_server = tokio::spawn(async move {
        if let Err(error) = axum::serve(metrics_listener, metrics_app).await {
            warn!(%error, "metrics server exited");
        }
    });

    let app_server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let signal_result = tokio::signal::ctrl_c().await;
        if signal_result.is_ok() {
            state_for_shutdown
                .readiness
                .store(false, std::sync::atomic::Ordering::SeqCst);
            sleep(std::time::Duration::from_secs(shutdown_drain_seconds)).await;
        }
    });
    app_server.await?;

    metrics_server.abort();
    Ok(())
}

fn init_tls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .json()
        .init();
}
