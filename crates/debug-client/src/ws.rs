use anyhow::{Context, Result};
use futures_util::{Sink, SinkExt};
use language_coach_protocol::telnyx;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::cli::AuthMode;
use crate::config::WebSocketUrl;

/// Outbound WebSocket frame requested by the debug session loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutgoingFrame {
    Media(Vec<u8>),
    Stop,
}

impl OutgoingFrame {
    fn into_message(self) -> Result<Message> {
        let text = match self {
            Self::Media(frame) => telnyx::build_media_message(&frame)?,
            Self::Stop => telnyx::build_stop_message()?,
        };
        Ok(Message::Text(text.into()))
    }

    const fn closes_session(&self) -> bool {
        matches!(self, Self::Stop)
    }
}

/// Spawns a single writer task for all outbound WebSocket frames.
pub(crate) fn spawn_writer_task<W>(mut write: W) -> mpsc::UnboundedSender<OutgoingFrame>
where
    W: Sink<Message> + Unpin + Send + 'static,
    W::Error: std::fmt::Display,
{
    let (sender, mut receiver) = mpsc::unbounded_channel::<OutgoingFrame>();
    tokio::spawn(async move {
        while let Some(frame) = receiver.recv().await {
            let closes_session = frame.closes_session();
            let message = match frame.into_message() {
                Ok(message) => message,
                Err(error) => {
                    eprintln!("failed to build websocket message: {error}");
                    continue;
                }
            };
            if let Err(error) = write.send(message).await {
                eprintln!("websocket send failed: {error}");
                break;
            }
            if closes_session {
                let _ = write.close().await;
                break;
            }
        }
    });
    sender
}

/// Builds a websocket request with debug auth headers.
pub(crate) fn build_ws_request(
    url: &WebSocketUrl,
    token: &str,
    auth_mode: AuthMode,
) -> Result<http::Request<()>> {
    let mut request = url
        .as_str()
        .into_client_request()
        .context("failed to build websocket request")?;

    match auth_mode {
        AuthMode::Subprotocol => {
            let bearer = format!("language-coach-debug, bearer.{token}");
            request
                .headers_mut()
                .insert("Sec-WebSocket-Protocol", bearer.parse()?);
        }
        AuthMode::Authorization => {
            request
                .headers_mut()
                .insert("Sec-WebSocket-Protocol", "language-coach-debug".parse()?);
            request
                .headers_mut()
                .insert("Authorization", format!("Bearer {token}").parse()?);
        }
    }

    Ok(request)
}

#[cfg(test)]
mod tests {
    use language_coach_protocol::telnyx;

    use crate::{
        cli::AuthMode,
        config::WebSocketUrl,
        ws::{OutgoingFrame, build_ws_request},
    };

    #[test]
    fn builds_subprotocol_request() {
        let url = WebSocketUrl::parse("ws://localhost:8080/dev/connect");
        assert!(url.is_ok());
        let Ok(url) = url else {
            return;
        };
        let request = build_ws_request(&url, "token", AuthMode::Subprotocol);
        assert!(request.is_ok());
        let Ok(request) = request else {
            return;
        };
        assert_eq!(request.uri(), "ws://localhost:8080/dev/connect");
        let protocol = request
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|value| value.to_str().ok());
        assert_eq!(protocol, Some("language-coach-debug, bearer.token"));
    }

    #[test]
    fn outgoing_media_frame_serializes_to_telnyx_event() {
        let message = OutgoingFrame::Media(vec![1, 2, 3]).into_message();
        assert!(message.is_ok());
        let Ok(tokio_tungstenite::tungstenite::protocol::Message::Text(text)) = message else {
            return;
        };
        let event: Result<telnyx::Event, _> = serde_json::from_str(&text);
        assert!(matches!(event, Ok(telnyx::Event::Media(_))));
    }

    #[test]
    fn outgoing_stop_frame_serializes_to_telnyx_event() {
        let frame = OutgoingFrame::Stop;
        assert!(frame.closes_session());
        let message = frame.into_message();
        assert!(message.is_ok());
        let Ok(tokio_tungstenite::tungstenite::protocol::Message::Text(text)) = message else {
            return;
        };
        let event: Result<telnyx::Event, _> = serde_json::from_str(&text);
        assert!(matches!(event, Ok(telnyx::Event::Stop(_))));
    }
}
