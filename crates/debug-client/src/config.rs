use std::{path::PathBuf, str::FromStr};

use anyhow::{Context, Result};
use serde::Deserialize;
use url::Url;

use crate::audio::InputSource;

/// File-backed debug client settings.
#[derive(Debug, Clone)]
pub(crate) struct ClientConfig {
    pub(crate) server_url: WebSocketUrl,
    pub(crate) input: InputSource,
    pub(crate) output: OutputTarget,
}

#[derive(Debug, Clone, Deserialize)]
struct RawClientConfig {
    server_url: String,
    input: String,
    output: String,
    #[serde(default)]
    audio: AudioConfig,
}

/// Validated WebSocket URL for the debug media connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebSocketUrl(Url);

impl WebSocketUrl {
    /// Parses a `ws://` or `wss://` URL.
    pub(crate) fn parse(input: &str) -> Result<Self> {
        let url = Url::from_str(input).context("server_url must be a valid URL")?;
        if !matches!(url.scheme(), "ws" | "wss") {
            anyhow::bail!("server_url must use ws:// or wss://");
        }
        Ok(Self(url))
    }

    /// Returns the URL as a string slice for WebSocket libraries.
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Audio settings for frame format compatibility checks.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AudioConfig {
    #[serde(default = "default_sample_rate")]
    pub(crate) sample_rate: u32,
    #[serde(default = "default_encoding")]
    pub(crate) encoding: AudioEncoding,
}

/// Audio codec used at the debug-client protocol boundary.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AudioEncoding {
    Pcmu,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: default_sample_rate(),
            encoding: default_encoding(),
        }
    }
}

const fn default_sample_rate() -> u32 {
    8000
}

const fn default_encoding() -> AudioEncoding {
    AudioEncoding::Pcmu
}

/// Parsed output target for received audio frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputTarget {
    Speaker,
    None,
}

/// Loads debug client config from TOML.
pub(crate) fn load_config(path: &PathBuf) -> Result<ClientConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file at {}", path.display()))?;
    let raw_config: RawClientConfig =
        toml::from_str(&raw).context("failed to parse debug client config")?;
    ClientConfig::try_from(raw_config)
}

impl TryFrom<RawClientConfig> for ClientConfig {
    type Error = anyhow::Error;

    fn try_from(config: RawClientConfig) -> Result<Self> {
        validate_audio_config(&config.audio)?;
        Ok(Self {
            server_url: WebSocketUrl::parse(&config.server_url)?,
            input: parse_input_source(&config.input)?,
            output: parse_output_target(&config.output)?,
        })
    }
}

fn validate_audio_config(config: &AudioConfig) -> Result<()> {
    if config.sample_rate != 8000 {
        anyhow::bail!("audio.sample_rate must be 8000 for PCMU");
    }
    match config.encoding {
        AudioEncoding::Pcmu => {}
    }
    Ok(())
}

/// Parses configured audio input source.
pub(crate) fn parse_input_source(input: &str) -> Result<InputSource> {
    if input.eq_ignore_ascii_case("microphone") {
        return Ok(InputSource::Microphone);
    }
    if let Some(path) = input.strip_prefix("wav:") {
        if path.trim().is_empty() {
            anyhow::bail!("wav input must include a file path after 'wav:'");
        }
        return Ok(InputSource::WavFile(PathBuf::from(path)));
    }
    anyhow::bail!("input must be 'microphone' or 'wav:/path/to/file.wav'")
}

/// Parses configured playback target.
pub(crate) fn parse_output_target(output: &str) -> Result<OutputTarget> {
    if output.eq_ignore_ascii_case("speaker") {
        return Ok(OutputTarget::Speaker);
    }
    if output.eq_ignore_ascii_case("none") {
        return Ok(OutputTarget::None);
    }
    anyhow::bail!("output must be 'speaker' or 'none'")
}

#[cfg(test)]
mod tests {
    use crate::{
        audio::InputSource,
        config::{
            AudioConfig, AudioEncoding, ClientConfig, RawClientConfig, WebSocketUrl,
            parse_input_source,
        },
    };

    #[test]
    fn parses_input_sources() {
        let mic = parse_input_source("microphone");
        assert!(matches!(mic, Ok(InputSource::Microphone)));

        let wav = parse_input_source("wav:/tmp/a.wav");
        assert!(
            matches!(wav, Ok(InputSource::WavFile(path)) if path == std::path::Path::new("/tmp/a.wav"))
        );
    }

    #[test]
    fn parses_websocket_url() {
        assert!(WebSocketUrl::parse("ws://localhost:8080/dev/connect").is_ok());
        assert!(WebSocketUrl::parse("https://localhost:8080/dev/connect").is_err());
    }

    #[test]
    fn converts_raw_config_to_typed_config() {
        let audio = AudioConfig::default();
        assert_eq!(audio.encoding, AudioEncoding::Pcmu);
        let raw = RawClientConfig {
            server_url: "ws://localhost:8080/dev/connect".to_owned(),
            input: "microphone".to_owned(),
            output: "none".to_owned(),
            audio,
        };
        let config = ClientConfig::try_from(raw);
        assert!(config.is_ok());
        let Ok(config) = config else {
            return;
        };
        assert!(matches!(config.input, InputSource::Microphone));
    }
}
