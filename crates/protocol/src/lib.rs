#![deny(
    clippy::panic,
    clippy::todo,
    clippy::unreachable,
    clippy::unimplemented,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic_in_result_fn,
    clippy::exit
)]
#![cfg_attr(not(test), deny(clippy::assertions_on_constants))]
#![deny(clippy::pedantic)]

mod codec;
pub mod deepgram;
mod error;
pub mod telnyx;

pub use codec::{mulaw_to_pcm16, pcm16_to_mulaw};
pub use error::ProtocolError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_media_frame() {
        let input = r#"{"event":"media","media":{"payload":"AQID","track":"inbound"}}"#;
        let event: Result<telnyx::Event, _> = serde_json::from_str(input);
        assert!(event.is_ok());
        let Ok(event) = event else {
            return;
        };
        let telnyx::Event::Media(media) = event else {
            return;
        };
        let bytes = telnyx::decode_media_payload(&media);
        assert!(bytes.is_ok());
        assert_eq!(bytes.unwrap_or_default(), vec![1, 2, 3]);
    }

    #[test]
    fn parses_documented_telnyx_messages() {
        let messages = [
            r#"{"event":"connected","version":"1.0.0"}"#,
            r#"{"event":"start","sequence_number":"1","stream_id":"stream-1","start":{"user_id":"user-1","call_control_id":"call-1","call_session_id":"session-1","from":"+13122010094","to":"+13122123456","tags":["TAG1"],"client_state":"aGF2ZQ==","media_format":{"encoding":"PCMU","sample_rate":8000,"channels":1}}}"#,
            r#"{"event":"media","sequence_number":"4","stream_id":"stream-1","media":{"track":"inbound","chunk":"2","timestamp":"5","payload":"AQID"}}"#,
            r#"{"event":"stop","sequence_number":"5","stream_id":"stream-1","stop":{"user_id":"user-1","call_control_id":"call-1"}}"#,
            r#"{"event":"mark","stream_id":"stream-1","sequence_number":"6","mark":{"name":"done"}}"#,
            r#"{"event":"dtmf","stream_id":"stream-1","occurred_at":"2025-06-05T08:54:19.698408Z","sequence_number":"7","dtmf":{"digit":"1"}}"#,
            r#"{"event":"error","stream_id":"stream-1","payload":{"code":100003,"title":"malformed_frame","detail":"bad frame"}}"#,
            r#"{"event":"clear"}"#,
        ];

        for message in messages {
            let parsed: Result<telnyx::Event, _> = serde_json::from_str(message);
            assert!(parsed.is_ok(), "failed to parse {message}: {parsed:?}");
        }
    }

    #[test]
    fn parses_telnyx_call_control_webhook() {
        let input = r#"{"data":{"id":"event-1","event_type":"call.initiated","payload":{"call_control_id":"call-1","from":"+15551234567"}}}"#;
        let parsed: Result<telnyx::Webhook, _> = serde_json::from_str(input);
        assert!(parsed.is_ok());
        let Ok(parsed) = parsed else {
            return;
        };
        assert_eq!(
            parsed.data.event_type,
            telnyx::WebhookEventType::CallInitiated
        );
    }

    #[test]
    fn parses_telnyx_answered_webhook() {
        let input = r#"{"data":{"id":"event-2","event_type":"call.answered","payload":{"call_control_id":"call-1","from":"+15551234567"}}}"#;
        let parsed: Result<telnyx::Webhook, _> = serde_json::from_str(input);
        assert!(parsed.is_ok());
        let Ok(parsed) = parsed else {
            return;
        };
        assert_eq!(
            parsed.data.event_type,
            telnyx::WebhookEventType::CallAnswered
        );
    }

    #[test]
    fn builds_media_frame_roundtrip() {
        let source = vec![0u8, 1, 2, 3, 4, 5, 6, 7];
        let text = telnyx::build_media_message(&source);
        assert!(text.is_ok());
        let Ok(text) = text else {
            return;
        };
        let event: Result<telnyx::Event, _> = serde_json::from_str(&text);
        assert!(event.is_ok());
        let Ok(event) = event else {
            return;
        };
        let telnyx::Event::Media(media) = event else {
            return;
        };
        let decoded = telnyx::decode_media_payload(&media);
        assert!(decoded.is_ok());
        assert_eq!(decoded.unwrap_or_default(), source);
    }

    #[test]
    fn builds_stop_frame_roundtrip() {
        let text = telnyx::build_stop_message();
        assert!(text.is_ok());
        let Ok(text) = text else {
            return;
        };
        let event: Result<telnyx::Event, _> = serde_json::from_str(&text);
        assert!(event.is_ok());
        let Ok(event) = event else {
            return;
        };
        let telnyx::Event::Stop(stop) = event else {
            return;
        };
        assert_eq!(stop.stream_id, "debug");
    }

    #[test]
    fn pcmu_codec_decodes_silence_values_to_zero() {
        assert_eq!(mulaw_to_pcm16([0xff, 0x7f]), vec![0, 0]);
    }

    #[test]
    fn pcmu_codec_roundtrip_is_close() {
        let source = vec![-30000i16, -1000, -100, 0, 100, 1000, 30000];
        let ulaw = pcm16_to_mulaw(source.clone());
        let back = mulaw_to_pcm16(ulaw);
        assert_eq!(source.len(), back.len());
        assert!(
            source
                .iter()
                .zip(back.iter())
                .all(|(a, b)| (i32::from(*a) - i32::from(*b)).abs() < 1200)
        );
    }

    #[test]
    fn pcmu_codec_handles_i16_min_without_panicking() {
        let encoded = pcm16_to_mulaw([i16::MIN]);
        assert_eq!(encoded.len(), 1);
        let decoded = mulaw_to_pcm16(encoded);
        assert_eq!(decoded.len(), 1);
    }

    #[test]
    fn parses_all_documented_deepgram_server_text_messages() {
        let messages = [
            r#"{"type":"FunctionCallResponse","id":"f-1","name":"end_call","content":"ok"}"#,
            r#"{"type":"PromptUpdated"}"#,
            r#"{"type":"SpeakUpdated"}"#,
            r#"{"type":"ThinkUpdated"}"#,
            r#"{"type":"InjectionRefused","message":"policy"}"#,
            r#"{"type":"Welcome","request_id":"req-1"}"#,
            r#"{"type":"SettingsApplied"}"#,
            r#"{"type":"ConversationText","role":"user","content":"hello"}"#,
            r#"{"type":"UserStartedSpeaking"}"#,
            r#"{"type":"AgentThinking","content":"thinking"}"#,
            r#"{"type":"FunctionCallRequest","functions":[{"id":"f-2","name":"set_speaking_rate","arguments":"{\"speed\":1.1}","client_side":true}]}"#,
            r#"{"type":"AgentStartedSpeaking","total_latency":0.7,"tts_latency":0.2,"ttt_latency":0.5}"#,
            r#"{"type":"AgentAudioDone"}"#,
            r#"{"type":"Error","description":"bad","code":"E123"}"#,
            r#"{"type":"Warning","description":"warn","code":"W123"}"#,
            r#"{"type":"History","role":"assistant","content":"hola"}"#,
            r#"{"type":"History","function_calls":[{"id":"f-3","name":"end_call","client_side":true,"arguments":"{}","response":"ok"}]}"#,
        ];
        for message in messages {
            let parsed: Result<deepgram::ServerMessage, _> = serde_json::from_str(message);
            assert!(parsed.is_ok(), "failed to parse {message}: {parsed:?}");
        }
    }

    #[test]
    fn parses_deepgram_function_arguments_from_json_string() {
        let message = r#"{"type":"FunctionCallRequest","functions":[{"id":"f-2","name":"set_speaking_rate","arguments":"{\"speed\":1.1}","client_side":true}]}"#;
        let parsed: Result<deepgram::ServerMessage, _> = serde_json::from_str(message);
        assert!(parsed.is_ok());
        let Ok(deepgram::ServerMessage::FunctionCallRequest(request)) = parsed else {
            return;
        };
        let Some(call) = request.functions.first() else {
            return;
        };
        assert_eq!(
            call.arguments
                .as_value()
                .get("speed")
                .and_then(serde_json::Value::as_f64),
            Some(1.1)
        );
    }

    #[test]
    fn parses_deepgram_history_function_arguments_from_json_string() {
        let message = r#"{"type":"History","function_calls":[{"id":"f-3","name":"end_call","client_side":true,"arguments":"{\"reason\":\"done\"}","response":"ok"}]}"#;
        let parsed: Result<deepgram::ServerMessage, _> = serde_json::from_str(message);
        assert!(parsed.is_ok());
        let Ok(deepgram::ServerMessage::History(deepgram::HistoryMessage::FunctionCalls(request))) =
            parsed
        else {
            return;
        };
        let Some(call) = request.function_calls.first() else {
            return;
        };
        assert_eq!(
            call.arguments
                .as_value()
                .get("reason")
                .and_then(serde_json::Value::as_str),
            Some("done")
        );
    }

    #[test]
    fn rejects_malformed_telnyx_payload_fixtures() {
        let messages = [
            r#"{"event":"connected"}"#,
            r#"{"event":"start","sequence_number":"1","stream_id":"s","start":{"call_control_id":"c"}}"#,
            r#"{"event":"media","media":{"payload":"not base64!"}}"#,
            r#"{"event":"stop","sequence_number":"5"}"#,
            r#"{"event":"mark","mark":{}}"#,
            r#"{"event":"dtmf","stream_id":"s","sequence_number":"7","dtmf":{}}"#,
            r#"{"event":"error","stream_id":"s","payload":{"code":"bad","title":"t","detail":"d"}}"#,
        ];

        for message in messages {
            let parsed: Result<telnyx::Event, _> = serde_json::from_str(message);
            assert!(parsed.is_err(), "parsed malformed fixture: {message}");
        }
    }

    #[test]
    fn rejects_malformed_deepgram_payload_fixtures() {
        let messages = [
            r#"{"type":"FunctionCallResponse","id":"f-1","content":"ok"}"#,
            r#"{"type":"InjectionRefused"}"#,
            r#"{"type":"Welcome"}"#,
            r#"{"type":"ConversationText","role":"learner","content":"hello"}"#,
            r#"{"type":"AgentThinking"}"#,
            r#"{"type":"FunctionCallRequest","functions":[{"id":"f-2","name":"set_speaking_rate","arguments":"not-json","client_side":true}]}"#,
            r#"{"type":"AgentStartedSpeaking","total_latency":"slow","tts_latency":0.2,"ttt_latency":0.5}"#,
            r#"{"type":"Error","description":"bad"}"#,
            r#"{"type":"Warning","code":"W123"}"#,
            r#"{"type":"History","function_calls":[{"id":"f-3","name":"end_call","client_side":true,"arguments":"not-json","response":"ok"}]}"#,
        ];

        for message in messages {
            let parsed: Result<deepgram::ServerMessage, _> = serde_json::from_str(message);
            assert!(parsed.is_err(), "parsed malformed fixture: {message}");
        }
    }

    #[test]
    fn allows_unknown_additive_fields() {
        let telnyx = r#"{"event":"connected","version":"1.0.0","new_field":true}"#;
        let deepgram = r#"{"type":"UserStartedSpeaking","new_field":true}"#;

        assert!(serde_json::from_str::<telnyx::Event>(telnyx).is_ok());
        assert!(serde_json::from_str::<deepgram::ServerMessage>(deepgram).is_ok());
    }

    #[test]
    fn rejects_unknown_type_discriminants() {
        let telnyx = r#"{"event":"mystery","payload":{}}"#;
        let deepgram = r#"{"type":"MysteryMessage"}"#;

        assert!(serde_json::from_str::<telnyx::Event>(telnyx).is_err());
        assert!(serde_json::from_str::<deepgram::ServerMessage>(deepgram).is_err());
    }
}
