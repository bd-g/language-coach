use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::Parser;
use ed25519_dalek::VerifyingKey;
use language_coach_protocol::{deepgram as protocol_deepgram, telnyx as protocol_telnyx};
use serde::{Deserialize, Serialize};

/// Command-line arguments for launching the server.
#[derive(Debug, Parser)]
pub(crate) struct Cli {
    #[arg(long)]
    pub(crate) config: PathBuf,
}

/// Runtime configuration loaded from TOML.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AppConfig {
    pub(crate) public_base_url: String,
    pub(crate) telnyx_public_key: String,
    pub(crate) allowlist_numbers: Vec<String>,
    #[serde(default)]
    pub(crate) server: ServerConfig,
    #[serde(default)]
    pub(crate) telnyx: TelnyxConfig,
    #[serde(default)]
    pub(crate) deepgram: DeepgramConfig,
}

/// Socket and session limits for the server.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ServerConfig {
    #[serde(default = "default_bind_addr")]
    pub(crate) bind_addr: String,
    #[serde(default = "default_metrics_bind_addr")]
    pub(crate) metrics_bind_addr: String,
    #[serde(default = "default_max_active_sessions")]
    pub(crate) max_active_sessions: usize,
    #[serde(default = "default_max_session_duration_seconds")]
    pub(crate) max_session_duration_seconds: u64,
    #[serde(default = "default_shutdown_drain_seconds")]
    pub(crate) shutdown_drain_seconds: u64,
}

/// Deepgram connection and model settings.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DeepgramConfig {
    #[serde(default = "default_deepgram_url")]
    pub(crate) url: String,
    #[serde(default = "default_deepgram_connect_timeout_ms")]
    pub(crate) connect_timeout_ms: u64,
    #[serde(default = "default_deepgram_settings_timeout_ms")]
    pub(crate) settings_timeout_ms: u64,
    #[serde(default = "default_deepgram_transient_reconnect_attempts")]
    pub(crate) transient_reconnect_attempts: usize,
    #[serde(default = "default_deepgram_listen_model")]
    pub(crate) listen_model: String,
    #[serde(default = "default_deepgram_listen_version")]
    pub(crate) listen_version: protocol_deepgram::ListenProviderVersion,
    #[serde(default = "default_deepgram_language_hints")]
    pub(crate) language_hints: Vec<String>,
    #[serde(default = "default_deepgram_think_provider")]
    pub(crate) think_provider: protocol_deepgram::ThinkProviderType,
    #[serde(default = "default_deepgram_think_model")]
    pub(crate) think_model: String,
    #[serde(default = "default_deepgram_speak_provider")]
    pub(crate) speak_provider: protocol_deepgram::SpeakProviderType,
    #[serde(default = "default_deepgram_speak_model")]
    pub(crate) speak_model: String,
    #[serde(default = "default_deepgram_speak_speed")]
    pub(crate) speak_speed: f32,
    #[serde(default = "default_deepgram_history_enabled")]
    pub(crate) history_enabled: bool,
    #[serde(default = "default_deepgram_greeting")]
    pub(crate) greeting: String,
}

/// Telnyx webhook and Call Control REST behavior.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TelnyxConfig {
    #[serde(default = "default_telnyx_api_base_url")]
    pub(crate) api_base_url: String,
    #[serde(default = "default_telnyx_rest_timeout_ms")]
    pub(crate) rest_timeout_ms: u64,
    #[serde(default = "default_telnyx_rest_retry_attempts")]
    pub(crate) rest_retry_attempts: usize,
    #[serde(default = "default_telnyx_webhook_replay_window_seconds")]
    pub(crate) webhook_replay_window_seconds: u64,
    #[serde(default = "default_telnyx_dedupe_ttl_seconds")]
    pub(crate) dedupe_ttl_seconds: u64,
    #[serde(default = "default_telnyx_stream_track")]
    pub(crate) stream_track: TelnyxStreamTrack,
    #[serde(default = "default_telnyx_stream_bidirectional_mode")]
    pub(crate) stream_bidirectional_mode: TelnyxBidirectionalMode,
    #[serde(default = "default_telnyx_stream_bidirectional_codec")]
    pub(crate) stream_bidirectional_codec: protocol_telnyx::MediaEncoding,
}

/// Telnyx media stream tracks accepted by Call Control answer commands.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) enum TelnyxStreamTrack {
    #[serde(rename = "inbound_track")]
    Inbound,
    #[serde(rename = "outbound_track")]
    Outbound,
    #[serde(rename = "both_tracks")]
    Both,
}

/// Telnyx bidirectional streaming transport mode.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) enum TelnyxBidirectionalMode {
    #[serde(rename = "rtp")]
    Rtp,
    #[serde(rename = "mp3")]
    Mp3,
}

impl Default for TelnyxConfig {
    fn default() -> Self {
        Self {
            api_base_url: default_telnyx_api_base_url(),
            rest_timeout_ms: default_telnyx_rest_timeout_ms(),
            rest_retry_attempts: default_telnyx_rest_retry_attempts(),
            webhook_replay_window_seconds: default_telnyx_webhook_replay_window_seconds(),
            dedupe_ttl_seconds: default_telnyx_dedupe_ttl_seconds(),
            stream_track: default_telnyx_stream_track(),
            stream_bidirectional_mode: default_telnyx_stream_bidirectional_mode(),
            stream_bidirectional_codec: default_telnyx_stream_bidirectional_codec(),
        }
    }
}

impl Default for DeepgramConfig {
    fn default() -> Self {
        Self {
            url: default_deepgram_url(),
            connect_timeout_ms: default_deepgram_connect_timeout_ms(),
            settings_timeout_ms: default_deepgram_settings_timeout_ms(),
            transient_reconnect_attempts: default_deepgram_transient_reconnect_attempts(),
            listen_model: default_deepgram_listen_model(),
            listen_version: default_deepgram_listen_version(),
            language_hints: default_deepgram_language_hints(),
            think_provider: default_deepgram_think_provider(),
            think_model: default_deepgram_think_model(),
            speak_provider: default_deepgram_speak_provider(),
            speak_model: default_deepgram_speak_model(),
            speak_speed: default_deepgram_speak_speed(),
            history_enabled: default_deepgram_history_enabled(),
            greeting: default_deepgram_greeting(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
            metrics_bind_addr: default_metrics_bind_addr(),
            max_active_sessions: default_max_active_sessions(),
            max_session_duration_seconds: default_max_session_duration_seconds(),
            shutdown_drain_seconds: default_shutdown_drain_seconds(),
        }
    }
}

const fn default_max_active_sessions() -> usize {
    3
}

const fn default_max_session_duration_seconds() -> u64 {
    1800
}

fn default_bind_addr() -> String {
    "0.0.0.0:8080".to_owned()
}

fn default_telnyx_api_base_url() -> String {
    "https://api.telnyx.com/v2".to_owned()
}

const fn default_telnyx_rest_timeout_ms() -> u64 {
    2000
}

const fn default_telnyx_rest_retry_attempts() -> usize {
    2
}

const fn default_telnyx_webhook_replay_window_seconds() -> u64 {
    300
}

const fn default_telnyx_dedupe_ttl_seconds() -> u64 {
    900
}

const fn default_telnyx_stream_track() -> TelnyxStreamTrack {
    TelnyxStreamTrack::Both
}

const fn default_telnyx_stream_bidirectional_mode() -> TelnyxBidirectionalMode {
    TelnyxBidirectionalMode::Rtp
}

const fn default_telnyx_stream_bidirectional_codec() -> protocol_telnyx::MediaEncoding {
    protocol_telnyx::MediaEncoding::Pcmu
}

fn default_metrics_bind_addr() -> String {
    "0.0.0.0:9091".to_owned()
}

const fn default_shutdown_drain_seconds() -> u64 {
    10
}

fn default_deepgram_url() -> String {
    "wss://agent.deepgram.com/v1/agent/converse".to_owned()
}

const fn default_deepgram_connect_timeout_ms() -> u64 {
    3000
}

const fn default_deepgram_settings_timeout_ms() -> u64 {
    3000
}

const fn default_deepgram_transient_reconnect_attempts() -> usize {
    1
}

fn default_deepgram_listen_model() -> String {
    "flux-general-multi".to_owned()
}

const fn default_deepgram_listen_version() -> protocol_deepgram::ListenProviderVersion {
    protocol_deepgram::ListenProviderVersion::V2
}

fn default_deepgram_language_hints() -> Vec<String> {
    vec!["en".to_owned(), "es".to_owned()]
}

const fn default_deepgram_think_provider() -> protocol_deepgram::ThinkProviderType {
    protocol_deepgram::ThinkProviderType::OpenAi
}

fn default_deepgram_think_model() -> String {
    "gpt-5-mini".to_owned()
}

const fn default_deepgram_speak_provider() -> protocol_deepgram::SpeakProviderType {
    protocol_deepgram::SpeakProviderType::Deepgram
}

fn default_deepgram_speak_model() -> String {
    "aura-2-selena-es".to_owned()
}

const fn default_deepgram_speak_speed() -> f32 {
    1.0
}

const fn default_deepgram_history_enabled() -> bool {
    true
}

fn default_deepgram_greeting() -> String {
    "Hola, soy tu coach de conversacion en ingles. Vamos a practicar una situacion real.".to_owned()
}

/// Loads TOML config from disk.
pub(crate) fn load_config(path: &PathBuf) -> Result<AppConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file at {}", path.display()))?;
    toml::from_str(&raw).context("failed to parse config toml")
}

/// Validates config constraints at startup.
pub(crate) fn validate_config(config: &AppConfig) -> Result<()> {
    if !config.public_base_url.starts_with("https://") {
        anyhow::bail!("public_base_url must start with https://");
    }
    if config.allowlist_numbers.is_empty() {
        anyhow::bail!("allowlist_numbers must not be empty");
    }
    if !config
        .allowlist_numbers
        .iter()
        .all(|value| value.starts_with('+') && value.len() >= 8)
    {
        anyhow::bail!("allowlist_numbers must be E.164-like values");
    }
    if config.server.max_active_sessions == 0 {
        anyhow::bail!("max_active_sessions must be > 0");
    }
    if config.server.max_session_duration_seconds == 0 {
        anyhow::bail!("max_session_duration_seconds must be > 0");
    }
    if config.server.shutdown_drain_seconds == 0 {
        anyhow::bail!("shutdown_drain_seconds must be > 0");
    }
    if !config.telnyx.api_base_url.starts_with("https://") {
        anyhow::bail!("telnyx.api_base_url must start with https://");
    }
    if config.telnyx.rest_timeout_ms == 0 {
        anyhow::bail!("telnyx.rest_timeout_ms must be > 0");
    }
    if config.telnyx.webhook_replay_window_seconds == 0 {
        anyhow::bail!("telnyx.webhook_replay_window_seconds must be > 0");
    }
    if config.telnyx.dedupe_ttl_seconds == 0 {
        anyhow::bail!("telnyx.dedupe_ttl_seconds must be > 0");
    }
    if !config.deepgram.url.starts_with("wss://") {
        anyhow::bail!("deepgram.url must start with wss://");
    }
    if config.deepgram.connect_timeout_ms == 0 {
        anyhow::bail!("deepgram.connect_timeout_ms must be > 0");
    }
    if config.deepgram.settings_timeout_ms == 0 {
        anyhow::bail!("deepgram.settings_timeout_ms must be > 0");
    }
    if config.deepgram.listen_model.trim().is_empty()
        || config.deepgram.think_model.trim().is_empty()
        || config.deepgram.speak_model.trim().is_empty()
    {
        anyhow::bail!("deepgram model/provider values must be non-empty");
    }
    if !(0.7..=1.3).contains(&config.deepgram.speak_speed) {
        anyhow::bail!("deepgram.speak_speed must be between 0.7 and 1.3");
    }
    if config.deepgram.language_hints.is_empty() {
        anyhow::bail!("deepgram.language_hints must not be empty");
    }
    Ok(())
}

/// Parses a Telnyx Ed25519 public key from base64 or hex.
pub(crate) fn parse_ed25519_key(input: &str) -> Result<VerifyingKey> {
    let bytes = STANDARD.decode(input).or_else(|_| decode_hex(input))?;
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("telnyx_public_key must decode to 32 bytes"))?;
    VerifyingKey::from_bytes(&key_bytes).context("invalid Ed25519 verifying key bytes")
}

fn decode_hex(input: &str) -> Result<Vec<u8>> {
    let trimmed = input.trim();
    if !trimmed.len().is_multiple_of(2) {
        anyhow::bail!("hex value must have even length");
    }
    (0..trimmed.len())
        .step_by(2)
        .map(|index| {
            let pair = trimmed
                .as_bytes()
                .get(index..index + 2)
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .context("hex value must contain only ascii hex digits")?;
            u8::from_str_radix(pair, 16).map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>>>()
}
