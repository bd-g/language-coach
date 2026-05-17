use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};

/// Deepgram Voice Agent `Settings` client message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// Feature flags for the Voice Agent session.
    pub flags: SettingsFlags,
    /// Input and output audio settings.
    pub audio: SettingsAudio,
    /// Listen, think, speak, and greeting settings.
    pub agent: SettingsAgent,
}

/// Deepgram Voice Agent settings flags.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettingsFlags {
    /// Enables Deepgram history messages for reconnect context.
    pub history: bool,
}

/// Deepgram Voice Agent audio settings.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettingsAudio {
    /// Input audio settings accepted by Deepgram.
    pub input: AudioInput,
    /// Output audio settings emitted by Deepgram.
    pub output: AudioOutput,
}

/// Deepgram Voice Agent input audio settings.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioInput {
    /// Input audio encoding.
    pub encoding: InputAudioEncoding,
    /// Input audio sample rate in hertz.
    pub sample_rate: u32,
}

/// Deepgram Voice Agent output audio settings.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioOutput {
    /// Output audio encoding.
    pub encoding: OutputAudioEncoding,
    /// Output audio sample rate in hertz.
    pub sample_rate: u32,
    /// Output audio container.
    pub container: AudioOutputContainer,
}

/// Deepgram Voice Agent input audio encodings.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InputAudioEncoding {
    /// Linear 16-bit PCM.
    #[serde(rename = "linear16")]
    Linear16,
    /// Linear 32-bit PCM.
    #[serde(rename = "linear32")]
    Linear32,
    /// FLAC audio.
    #[serde(rename = "flac")]
    Flac,
    /// G.711 A-law audio.
    #[serde(rename = "alaw")]
    Alaw,
    /// G.711 mu-law audio.
    #[serde(rename = "mulaw")]
    Mulaw,
    /// Adaptive Multi-Rate Narrowband audio.
    #[serde(rename = "amr-nb")]
    AmrNb,
    /// Adaptive Multi-Rate Wideband audio.
    #[serde(rename = "amr-wb")]
    AmrWb,
    /// Opus audio.
    #[serde(rename = "opus")]
    Opus,
    /// Ogg Opus audio.
    #[serde(rename = "ogg-opus")]
    OggOpus,
    /// Speex audio.
    #[serde(rename = "speex")]
    Speex,
    /// G.729 audio.
    #[serde(rename = "g729")]
    G729,
}

/// Deepgram Voice Agent output audio encodings.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OutputAudioEncoding {
    /// Linear 16-bit PCM.
    #[serde(rename = "linear16")]
    Linear16,
    /// G.711 mu-law audio.
    #[serde(rename = "mulaw")]
    Mulaw,
    /// G.711 A-law audio.
    #[serde(rename = "alaw")]
    Alaw,
    /// MP3 audio.
    #[serde(rename = "mp3")]
    Mp3,
    /// Opus audio.
    #[serde(rename = "opus")]
    Opus,
    /// FLAC audio.
    #[serde(rename = "flac")]
    Flac,
    /// AAC audio.
    #[serde(rename = "aac")]
    Aac,
}

/// Deepgram Voice Agent output audio containers.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioOutputContainer {
    /// No audio container; raw encoded frames.
    #[serde(rename = "none")]
    None,
    /// WAV container.
    #[serde(rename = "wav")]
    Wav,
    /// Ogg container.
    #[serde(rename = "ogg")]
    Ogg,
}

/// Deepgram Voice Agent agent settings.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingsAgent {
    /// Speech-to-text provider config.
    pub listen: ListenConfig,
    /// LLM provider config and tools.
    pub think: ThinkConfig,
    /// Text-to-speech provider config.
    pub speak: SpeakConfig,
    /// Initial greeting spoken by the agent.
    pub greeting: String,
}

/// Deepgram Voice Agent listen config.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListenConfig {
    /// Speech-to-text provider settings.
    pub provider: ListenProvider,
}

/// Deepgram Voice Agent think config.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinkConfig {
    /// LLM provider settings.
    pub provider: ThinkProvider,
    /// System prompt for the agent.
    pub prompt: String,
    /// Function tool schemas available to the agent.
    pub functions: Vec<ToolDefinition>,
}

/// Deepgram Voice Agent speak config.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpeakConfig {
    /// Text-to-speech provider settings.
    pub provider: SpeakProvider,
}

/// Deepgram listen provider settings.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListenProvider {
    /// Listen provider discriminator.
    pub r#type: ListenProviderType,
    /// Deepgram listen API version.
    pub version: ListenProviderVersion,
    /// Speech-to-text model name. Deepgram may add model names without a protocol change.
    pub model: String,
    /// Speech-to-text language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Language hints for multilingual Flux models.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub language_hints: Vec<String>,
}

/// Deepgram listen provider discriminator.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ListenProviderType {
    /// Deepgram speech-to-text provider.
    #[serde(rename = "deepgram")]
    Deepgram,
}

/// Deepgram listen provider versions.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ListenProviderVersion {
    /// Deepgram V1 speech-to-text API.
    #[serde(rename = "v1")]
    V1,
    /// Deepgram V2 speech-to-text API, including Flux.
    #[serde(rename = "v2")]
    V2,
}

/// Deepgram think provider settings.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThinkProvider {
    /// Think provider discriminator.
    pub r#type: ThinkProviderType,
    /// LLM model name. Deepgram may add model names without a protocol change.
    pub model: String,
}

/// Deepgram think provider discriminator.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ThinkProviderType {
    /// OpenAI-compatible provider through Deepgram.
    #[serde(rename = "open_ai")]
    OpenAi,
    /// AWS Bedrock provider through Deepgram.
    #[serde(rename = "aws_bedrock")]
    AwsBedrock,
    /// Anthropic provider through Deepgram.
    #[serde(rename = "anthropic")]
    Anthropic,
    /// Google provider through Deepgram.
    #[serde(rename = "google")]
    Google,
    /// Groq provider through Deepgram.
    #[serde(rename = "groq")]
    Groq,
}

/// Deepgram speak provider settings.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpeakProvider {
    /// Speak provider discriminator.
    pub r#type: SpeakProviderType,
    /// TTS model name. Deepgram may add model names without a protocol change.
    pub model: String,
    /// Speaking-rate multiplier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
}

/// Deepgram speak provider discriminator.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SpeakProviderType {
    /// Deepgram text-to-speech provider.
    #[serde(rename = "deepgram")]
    Deepgram,
    /// `ElevenLabs` text-to-speech provider.
    #[serde(rename = "eleven_labs")]
    ElevenLabs,
}

/// Deepgram Voice Agent function tool schema.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    /// Function name exposed to the agent.
    pub name: String,
    /// Function description exposed to the agent.
    pub description: String,
    /// JSON Schema object. This is intentionally schema passthrough at the provider boundary.
    pub parameters: serde_json::Value,
}

/// Deepgram Voice Agent `UpdateSpeak` client message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateSpeak {
    /// Replacement speak config.
    pub speak: SpeakConfig,
}

/// Deepgram Voice Agent `FunctionCallResponse` client message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionCallResponse {
    /// Function call identifier from the corresponding request.
    pub id: String,
    /// Function name from the corresponding request.
    pub name: String,
    /// Function result content returned to Deepgram.
    pub content: String,
}

/// Deepgram Voice Agent client JSON text message.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// `Settings` message sent first to configure a Voice Agent session.
    Settings(Box<Settings>),
    /// `UpdateSpeak` message sent to change speaking settings mid-session.
    UpdateSpeak(UpdateSpeak),
    /// `InjectUserMessage` message sent to inject a user utterance.
    InjectUserMessage(InjectMessage),
    /// `InjectAgentMessage` message sent to inject an assistant utterance.
    InjectAgentMessage(InjectMessage),
    /// `FunctionCallResponse` message sent after a client-side tool call.
    FunctionCallResponse(FunctionCallResponse),
    /// `KeepAlive` message sent to keep the session open.
    KeepAlive,
    /// `UpdatePrompt` message sent to replace the prompt mid-session.
    UpdatePrompt(UpdatePromptMessage),
    /// `UpdateThink` message sent to replace think settings mid-session.
    UpdateThink(Box<UpdateThinkMessage>),
}

/// Deepgram injected text message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InjectMessage {
    /// Injected text content.
    pub content: String,
}

/// Deepgram `UpdatePrompt` client message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdatePromptMessage {
    /// Replacement prompt text.
    pub prompt: String,
}

/// Deepgram `UpdateThink` client message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateThinkMessage {
    /// Replacement think config.
    pub think: ThinkConfig,
}

/// Deepgram Voice Agent server JSON text message.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// `FunctionCallResponse` message for server-side function execution.
    FunctionCallResponse(ReceiveFunctionCallResponseMessage),
    /// `PromptUpdated` confirmation message.
    PromptUpdated,
    /// `SpeakUpdated` confirmation message.
    SpeakUpdated,
    /// `ThinkUpdated` confirmation message.
    ThinkUpdated,
    /// `InjectionRefused` message explaining an injection rejection.
    InjectionRefused(InjectionRefusedMessage),
    /// `Welcome` message sent at session setup.
    Welcome(WelcomeMessage),
    /// `SettingsApplied` confirmation message.
    SettingsApplied,
    /// `ConversationText` transcript message.
    ConversationText(ConversationTextMessage),
    /// `UserStartedSpeaking` barge-in signal.
    UserStartedSpeaking,
    /// `AgentThinking` message with thought text.
    AgentThinking(AgentThinkingMessage),
    /// `FunctionCallRequest` message requesting client-side tool execution.
    FunctionCallRequest(FunctionCallRequestMessage),
    /// `AgentStartedSpeaking` latency metric message.
    AgentStartedSpeaking(AgentStartedSpeakingMessage),
    /// `AgentAudioDone` message sent after generated audio completes.
    AgentAudioDone,
    /// `Error` terminal or non-terminal error message.
    Error(StatusMessage),
    /// `Warning` non-fatal warning message.
    Warning(StatusMessage),
    /// `History` conversation or function-call history message.
    History(HistoryMessage),
}

/// Deepgram `FunctionCallResponse` server message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ReceiveFunctionCallResponseMessage {
    /// Function call identifier, omitted by Deepgram for some server-side executions.
    pub id: Option<String>,
    /// Function name.
    pub name: String,
    /// Function response content.
    pub content: String,
}

/// Deepgram `InjectionRefused` server message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct InjectionRefusedMessage {
    /// Refusal reason.
    pub message: String,
}

/// Deepgram `Welcome` server message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WelcomeMessage {
    /// Deepgram request identifier.
    pub request_id: String,
}

/// Deepgram conversation role.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConversationRole {
    /// User/caller role.
    User,
    /// Assistant/agent role.
    Assistant,
}

/// Deepgram `ConversationText` server message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ConversationTextMessage {
    /// Speaker role.
    pub role: ConversationRole,
    /// Transcript content.
    pub content: String,
    /// Active language hints when present.
    pub languages_hinted: Option<Vec<String>>,
    /// Detected languages when present.
    pub languages: Option<Vec<String>>,
}

/// Deepgram `AgentThinking` server message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AgentThinkingMessage {
    /// Agent thought content.
    pub content: String,
}

/// Deepgram `FunctionCallRequest` server message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FunctionCallRequestMessage {
    /// Batched function calls requested by Deepgram.
    pub functions: Vec<FunctionCallRequest>,
}

/// Deepgram function call request item.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FunctionCallRequest {
    /// Function call identifier.
    pub id: String,
    /// Function name.
    pub name: String,
    /// Function arguments. Deepgram sends this as a JSON string on the wire.
    #[serde(deserialize_with = "deserialize_json_string")]
    pub arguments: FunctionCallArguments,
    /// Whether the function should be executed by this client.
    pub client_side: bool,
    /// Gemini thought signature when present.
    pub thought_signature: Option<String>,
}

/// Parsed Deepgram function-call arguments.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionCallArguments(serde_json::Value);

impl FunctionCallArguments {
    /// Deserializes the function arguments into an application-specific typed request.
    ///
    /// # Errors
    /// Returns an error when the argument object does not match `T`.
    pub fn deserialize<T>(&self) -> Result<T, serde_json::Error>
    where
        T: DeserializeOwned,
    {
        serde_json::from_value(self.0.clone())
    }

    /// Returns the parsed JSON value at the provider boundary for schema-like inspection.
    #[must_use]
    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }
}

/// Deepgram `AgentStartedSpeaking` server message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AgentStartedSpeakingMessage {
    /// Total latency from utterance to reply in seconds.
    #[serde(rename = "total_latency")]
    pub total: f64,
    /// Text-to-speech latency in seconds.
    #[serde(rename = "tts_latency")]
    pub tts: f64,
    /// Text-to-text latency in seconds.
    #[serde(rename = "ttt_latency")]
    pub ttt: f64,
}

/// Deepgram `Error` or `Warning` server message payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StatusMessage {
    /// Human-readable status description.
    pub description: String,
    /// Deepgram status code.
    pub code: String,
}

/// Deepgram `History` server message payload variants.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum HistoryMessage {
    /// Conversation text history item.
    Conversation(HistoryConversationMessage),
    /// Function call history item list.
    FunctionCalls(HistoryFunctionCallsMessage),
}

/// Deepgram conversational `History` payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct HistoryConversationMessage {
    /// Speaker role.
    pub role: ConversationRole,
    /// Transcript content.
    pub content: String,
}

/// Deepgram function-call `History` payload.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct HistoryFunctionCallsMessage {
    /// Function call records.
    pub function_calls: Vec<HistoryFunctionCall>,
}

/// Deepgram historical function-call record.
///
/// Spec: <https://developers.deepgram.com/reference/voice-agent/voice-agent>.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct HistoryFunctionCall {
    /// Function call identifier.
    pub id: String,
    /// Function name.
    pub name: String,
    /// Whether the function was executed by the client.
    pub client_side: bool,
    /// Function arguments. Deepgram sends this as a JSON string on the wire.
    #[serde(deserialize_with = "deserialize_json_string")]
    pub arguments: FunctionCallArguments,
    /// Function response content.
    pub response: String,
    /// Gemini thought signature when present.
    pub thought_signature: Option<String>,
}

fn deserialize_json_string<'de, D>(deserializer: D) -> Result<FunctionCallArguments, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    serde_json::from_str(&value)
        .map(FunctionCallArguments)
        .map_err(serde::de::Error::custom)
}
