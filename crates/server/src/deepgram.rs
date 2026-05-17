use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use language_coach_protocol::deepgram as protocol;
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::StatusCode},
};
use tracing::info;

use crate::config::DeepgramConfig;

const DEFAULT_PROMPT: &str = "Eres un coach de conversacion en ingles para hablantes nativos de espanol de Mexico. Mantiene conversaciones naturales en ingles y da coaching breve en espanol cuando sea necesario.";

/// Opens and initializes a Deepgram Voice Agent websocket session.
pub(crate) async fn connect(
    config: &DeepgramConfig,
    api_key: &str,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let mut request = config
        .url
        .as_str()
        .into_client_request()
        .context("failed to build Deepgram websocket request")?;
    request
        .headers_mut()
        .insert("Authorization", format!("Token {api_key}").parse()?);

    let connect_timeout = Duration::from_millis(config.connect_timeout_ms);
    info!(url = %config.url, timeout_ms = config.connect_timeout_ms, "connecting to Deepgram");
    let (mut stream, response) = timeout(connect_timeout, connect_async(request))
        .await
        .context("timed out connecting to Deepgram")??;
    ensure_handshake_accepted(response.status())?;
    info!(status = %response.status(), "Deepgram websocket handshake accepted");

    let settings = protocol::ClientMessage::Settings(Box::new(build_settings(config)));
    stream
        .send(Message::Text(serde_json::to_string(&settings)?.into()))
        .await
        .context("failed to send Deepgram settings")?;
    info!(
        timeout_ms = config.settings_timeout_ms,
        "waiting for Deepgram settings acknowledgement"
    );

    wait_for_settings_ack(config, &mut stream).await?;
    info!("Deepgram settings acknowledged");
    Ok(stream)
}

fn ensure_handshake_accepted(status: StatusCode) -> Result<()> {
    if status != StatusCode::SWITCHING_PROTOCOLS {
        anyhow::bail!("Deepgram handshake failed with status {status}");
    }
    Ok(())
}

/// Builds an `UpdateSpeak` command for runtime speaking-rate changes.
pub(crate) fn build_update_speak_message(config: &DeepgramConfig, speed: f32) -> Result<String> {
    let update = protocol::UpdateSpeak {
        speak: protocol::SpeakConfig {
            provider: protocol::SpeakProvider {
                r#type: config.speak_provider.clone(),
                model: config.speak_model.clone(),
                speed: Some(speed),
            },
        },
    };

    Ok(serde_json::to_string(
        &protocol::ClientMessage::UpdateSpeak(update),
    )?)
}

/// Builds a function-call response payload to return tool results.
pub(crate) fn build_function_response(call_id: &str, name: &str, content: &str) -> Result<String> {
    let response = protocol::FunctionCallResponse {
        id: call_id.to_owned(),
        name: name.to_owned(),
        content: content.to_owned(),
    };
    Ok(serde_json::to_string(
        &protocol::ClientMessage::FunctionCallResponse(response),
    )?)
}

async fn wait_for_settings_ack(
    config: &DeepgramConfig,
    stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> Result<()> {
    let setup_timeout = Duration::from_millis(config.settings_timeout_ms);
    timeout(setup_timeout, async {
        loop {
            let Some(message_result) = stream.next().await else {
                anyhow::bail!("Deepgram closed before settings acknowledgement");
            };
            let message = message_result.context("Deepgram websocket error during setup")?;
            match message {
                Message::Text(text) => match serde_json::from_str(&text) {
                    Ok(protocol::ServerMessage::SettingsApplied) => return Ok(()),
                    Ok(protocol::ServerMessage::Welcome(welcome)) => {
                        info!(request_id = %welcome.request_id, "Deepgram welcome received");
                    }
                    Ok(protocol::ServerMessage::Error(status)) => anyhow::bail!(
                        "Deepgram rejected settings with {}: {}",
                        status.code,
                        status.description
                    ),
                    Ok(_) => {}
                    Err(error) => {
                        return Err(error)
                            .context("failed to parse Deepgram settings acknowledgement");
                    }
                },
                Message::Ping(payload) => stream
                    .send(Message::Pong(payload))
                    .await
                    .context("failed to send Deepgram setup pong")?,
                Message::Pong(_) => {}
                Message::Binary(_) => {
                    anyhow::bail!("unexpected binary Deepgram message during setup");
                }
                Message::Close(_) => anyhow::bail!("Deepgram closed during setup"),
                Message::Frame(_) => anyhow::bail!("unexpected Deepgram frame during setup"),
            }
        }
    })
    .await
    .context("timed out waiting for Deepgram settings acknowledgement")?
}

fn build_settings(config: &DeepgramConfig) -> protocol::Settings {
    protocol::Settings {
        flags: protocol::SettingsFlags {
            history: config.history_enabled,
        },
        audio: protocol::SettingsAudio {
            input: protocol::AudioInput {
                encoding: protocol::InputAudioEncoding::Mulaw,
                sample_rate: 8000,
            },
            output: protocol::AudioOutput {
                encoding: protocol::OutputAudioEncoding::Mulaw,
                sample_rate: 8000,
                container: protocol::AudioOutputContainer::None,
            },
        },
        agent: protocol::SettingsAgent {
            listen: protocol::ListenConfig {
                provider: protocol::ListenProvider {
                    r#type: protocol::ListenProviderType::Deepgram,
                    version: config.listen_version.clone(),
                    model: config.listen_model.clone(),
                    language: None,
                    language_hints: config.language_hints.clone(),
                },
            },
            think: protocol::ThinkConfig {
                provider: protocol::ThinkProvider {
                    r#type: config.think_provider.clone(),
                    model: config.think_model.clone(),
                },
                prompt: DEFAULT_PROMPT.to_owned(),
                functions: vec![
                    set_speaking_rate_function_schema(),
                    end_call_function_schema(),
                ],
            },
            speak: protocol::SpeakConfig {
                provider: protocol::SpeakProvider {
                    r#type: config.speak_provider.clone(),
                    model: config.speak_model.clone(),
                    speed: Some(config.speak_speed),
                },
            },
            greeting: config.greeting.clone(),
        },
    }
}

fn set_speaking_rate_function_schema() -> protocol::ToolDefinition {
    protocol::ToolDefinition {
        name: "set_speaking_rate".to_owned(),
        description: "Adjust the voice agent speaking speed.".to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "speed": {
                    "type": "number",
                    "minimum": 0.7,
                    "maximum": 1.3,
                },
                "reason": {
                    "type": "string",
                },
            },
            "required": ["speed"],
        }),
    }
}

fn end_call_function_schema() -> protocol::ToolDefinition {
    protocol::ToolDefinition {
        name: "end_call".to_owned(),
        description: "End the session gracefully when the learner is done.".to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                }
            },
            "required": ["reason"],
        }),
    }
}

#[cfg(test)]
mod tests {
    use tokio_tungstenite::tungstenite::http::StatusCode;

    use super::ensure_handshake_accepted;

    #[test]
    fn accepts_successful_websocket_upgrade_status() {
        assert!(ensure_handshake_accepted(StatusCode::SWITCHING_PROTOCOLS).is_ok());
    }

    #[test]
    fn rejects_non_upgrade_handshake_status() {
        let result = ensure_handshake_accepted(StatusCode::OK);
        assert!(result.is_err(), "accepted non-upgrade handshake status");
        let Some(error) = result.err() else {
            return;
        };

        assert_eq!(
            error.to_string(),
            "Deepgram handshake failed with status 200 OK"
        );
    }
}
