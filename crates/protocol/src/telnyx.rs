use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use crate::ProtocolError;

/// Media Streaming WebSocket event.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming>.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    /// Sent after the WebSocket opens.
    Connected(ConnectedEvent),
    /// Sent before media frames with call and audio metadata.
    Start(StartEvent),
    /// Carries base64-encoded audio on the wire.
    Media(MediaEvent),
    /// Sent when the media stream ends.
    Stop(StopEvent),
    /// Correlates queued playback completion.
    Mark(MarkEvent),
    /// Sent when DTMF input is detected.
    Dtmf(DtmfEvent),
    /// Reports malformed frames or stream failures.
    Error(ErrorEvent),
    /// Stops queued outbound media.
    Clear,
}

/// Builds a `media` JSON text frame from raw PCMU bytes.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#sending-rtp-stream>.
///
/// # Errors
/// Returns an error when JSON serialization fails.
pub fn build_media_message(pcmu_bytes: &[u8]) -> Result<String, ProtocolError> {
    let message = Event::Media(MediaEvent {
        sequence_number: None,
        stream_id: None,
        media: MediaPayload {
            payload: Bytes::copy_from_slice(pcmu_bytes),
            track: None,
            chunk: None,
            timestamp: None,
        },
    });
    Ok(serde_json::to_string(&message)?)
}

/// Builds a `stop` JSON text frame for direct debug clients.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
///
/// # Errors
/// Returns an error when JSON serialization fails.
pub fn build_stop_message() -> Result<String, ProtocolError> {
    let message = Event::Stop(StopEvent {
        sequence_number: "0".to_owned(),
        stream_id: "debug".to_owned(),
        stop: None,
    });
    Ok(serde_json::to_string(&message)?)
}

/// Builds a `clear` JSON text frame.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#clear-message>.
#[must_use]
pub fn build_clear_message() -> String {
    "{\"event\":\"clear\"}".to_owned()
}

/// Extracts decoded raw audio bytes from a `media` event.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
///
/// # Errors
/// Returns an error when the decoded media payload is empty.
pub fn decode_media_payload(event: &MediaEvent) -> Result<Vec<u8>, ProtocolError> {
    if event.media.payload.is_empty() {
        return Err(ProtocolError::MissingMediaPayload);
    }
    Ok(event.media.payload.to_vec())
}

/// Call Control webhook payload envelope.
///
/// Spec: <https://developers.telnyx.com/api/call-control/call-control-application-webhooks>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Webhook {
    /// Webhook event metadata and provider payload.
    pub data: WebhookData,
}

/// Call Control webhook event metadata.
///
/// Spec: <https://developers.telnyx.com/api/call-control/call-control-application-webhooks>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WebhookData {
    /// Provider event identifier used for idempotency.
    pub id: String,
    /// Call Control event type.
    pub event_type: WebhookEventType,
    /// Event payload; Telnyx omits fields on some event classes.
    pub payload: Option<WebhookPayload>,
}

/// Call Control webhook event types handled by the server.
///
/// Spec: <https://developers.telnyx.com/api/call-control/call-control-application-webhooks>.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookEventType {
    /// A new inbound or outbound call was initiated.
    CallInitiated,
    /// A call was hung up.
    CallHangup,
    /// Event type not yet modeled by this service.
    Other(String),
}

impl WebhookEventType {
    /// Returns the provider event type string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::CallInitiated => "call.initiated",
            Self::CallHangup => "call.hangup",
            Self::Other(value) => value,
        }
    }
}

impl fmt::Display for WebhookEventType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for WebhookEventType {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "call.initiated" => Self::CallInitiated,
            "call.hangup" => Self::CallHangup,
            other => Self::Other(other.to_owned()),
        })
    }
}

impl Serialize for WebhookEventType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for WebhookEventType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(D::Error::custom)
    }
}

/// Call Control webhook payload fields used for call decisions.
///
/// Spec: <https://developers.telnyx.com/api/call-control/call-control-application-webhooks>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WebhookPayload {
    /// Call Control identifier for REST commands.
    pub call_control_id: Option<String>,
    /// Caller phone number or SIP address.
    pub from: Option<String>,
}

/// Connection metadata sent immediately after the media WebSocket opens.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ConnectedEvent {
    /// Media streaming protocol version, for example `1.0.0`.
    pub version: String,
}

/// Stream-start envelope with call and audio metadata.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StartEvent {
    /// Provider sequence number as a decimal string.
    pub sequence_number: String,
    /// Provider stream identifier for this media WebSocket.
    pub stream_id: String,
    /// Call and media metadata for the stream.
    pub start: StartPayload,
}

/// Call metadata nested under a `start` event.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StartPayload {
    /// User identifier when present in the stream metadata.
    pub user_id: Option<String>,
    /// Call Control identifier.
    pub call_control_id: String,
    /// Call session identifier.
    pub call_session_id: String,
    /// Caller number or SIP address.
    pub from: String,
    /// Called number or SIP address.
    pub to: String,
    /// Application-provided stream tags.
    pub tags: Option<Vec<String>>,
    /// Base64 client state passed through Call Control.
    pub client_state: Option<String>,
    /// Audio format declared for incoming media frames.
    pub media_format: Option<MediaFormat>,
}

/// Audio format declared for incoming media frames.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MediaFormat {
    /// Media codec negotiated for the stream.
    pub encoding: MediaEncoding,
    /// Sample rate in hertz.
    pub sample_rate: u32,
    /// Channel count for the media stream.
    pub channels: u8,
}

/// Media codec names supported by bidirectional RTP streaming.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#rtp-stream-codec>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum MediaEncoding {
    /// G.711 mu-law at 8 kHz.
    #[serde(rename = "PCMU")]
    Pcmu,
    /// G.711 A-law at 8 kHz.
    #[serde(rename = "PCMA")]
    Pcma,
    /// G.722 audio.
    #[serde(rename = "G722")]
    G722,
    /// Opus audio.
    #[serde(rename = "OPUS")]
    Opus,
    /// Adaptive Multi-Rate Wideband audio.
    #[serde(rename = "AMR-WB")]
    AmrWb,
    /// Linear 16-bit PCM.
    #[serde(rename = "L16")]
    L16,
}

/// Media frame envelope.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MediaEvent {
    /// Sequence number as a decimal string when present.
    pub sequence_number: Option<String>,
    /// Stream identifier when present.
    pub stream_id: Option<String>,
    /// Media frame payload and optional sequencing metadata.
    pub media: MediaPayload,
}

/// Base64-on-the-wire audio payload plus optional sequencing metadata.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MediaPayload {
    /// Raw audio bytes; encoded as base64 in the Telnyx JSON `payload` field.
    #[serde(with = "base64_bytes")]
    pub payload: Bytes,
    /// Media track that produced the frame when present.
    pub track: Option<MediaTrack>,
    /// Chunk number as a decimal string when present.
    pub chunk: Option<String>,
    /// Timestamp as a decimal string when present.
    pub timestamp: Option<String>,
}

/// Media track names.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaTrack {
    /// Audio from the caller into the application.
    Inbound,
    /// Audio from the far side of the call.
    Outbound,
}

/// Stream-stop envelope.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StopEvent {
    /// Provider sequence number as a decimal string.
    pub sequence_number: String,
    /// Provider stream identifier for this media WebSocket.
    pub stream_id: String,
    /// Stream stop metadata.
    pub stop: Option<StopPayload>,
}

/// Optional call metadata nested under a `stop` event.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#streaming-process-flow>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StopPayload {
    /// User identifier when present in the stream metadata.
    pub user_id: Option<String>,
    /// Call Control identifier.
    pub call_control_id: Option<String>,
}

/// Marker envelope used to correlate media playback completion.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#mark-message>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MarkEvent {
    /// Sequence number as a decimal string when present.
    pub sequence_number: Option<String>,
    /// Stream identifier when present.
    pub stream_id: Option<String>,
    /// Marker name payload.
    pub mark: MarkPayload,
}

/// Marker name payload.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#mark-message>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MarkPayload {
    /// Application-defined marker name.
    pub name: String,
}

/// DTMF event envelope.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#dtmf-message>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DtmfEvent {
    /// Provider sequence number as a decimal string.
    pub sequence_number: String,
    /// Provider stream identifier for this media WebSocket.
    pub stream_id: String,
    /// Timestamp captured by the Telnyx telephony engine.
    pub occurred_at: Option<String>,
    /// DTMF digit payload.
    pub dtmf: DtmfPayload,
}

/// DTMF digit payload.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#dtmf-message>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DtmfPayload {
    /// Detected DTMF digit.
    pub digit: String,
}

/// Media stream error envelope.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#error-message>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ErrorEvent {
    /// Provider stream identifier for this media WebSocket.
    pub stream_id: String,
    /// Error payload values.
    pub payload: ErrorPayload,
}

/// Media stream error details.
///
/// Spec: <https://developers.telnyx.com/docs/voice/programmable-voice/media-streaming#error-message>.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ErrorPayload {
    /// Media streaming error code.
    pub code: i64,
    /// Media streaming error title.
    pub title: String,
    /// Human-readable error details.
    pub detail: String,
}

mod base64_bytes {
    use super::{Bytes, STANDARD};
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    pub(super) fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        STANDARD
            .decode(encoded)
            .map(Bytes::from)
            .map_err(D::Error::custom)
    }
}
