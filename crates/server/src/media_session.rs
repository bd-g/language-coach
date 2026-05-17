use std::time::Duration;

use anyhow::Result;
use axum::extract::ws::{Message as ClientMessage, WebSocket};
use futures_util::{
    Sink, StreamExt,
    stream::{SplitSink, SplitStream},
};
use language_coach_protocol::{deepgram, telnyx};
use serde::Deserialize;
use tokio::{
    select,
    time::{Instant, sleep, sleep_until},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Error as DeepgramSinkError, Message as DeepgramMessage},
};
use tracing::{error, info, trace, warn};

use crate::{
    deepgram::{build_function_response, build_update_speak_message, connect},
    state::{AppState, SessionGuard},
    utils::sink_queue::SinkQueue,
};

const FRAME_SAMPLES: usize = 160;
const FRAME_PACE_MS: u64 = 20;
const RECONNECTING_PCMU: &[u8] = include_bytes!("../assets/reconnecting.pcmu");
const FALLBACK_PCMU: &[u8] = include_bytes!("../assets/fallback.pcmu");

type DeepgramStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type CallerSink = SplitSink<WebSocket, ClientMessage>;
type CallerRead = SplitStream<WebSocket>;
type CallerSinkError = <CallerSink as Sink<ClientMessage>>::Error;
type CallerQueue = SinkQueue<ClientMessage>;
type DeepgramQueue = SinkQueue<DeepgramMessage>;

#[derive(Debug)]
struct SessionContext {
    speaking_speed: f32,
}

#[derive(Debug, Deserialize)]
struct SetSpeakingRateArgs {
    speed: f32,
}

#[derive(Debug, Clone, Copy)]
struct BridgeTiming {
    started: Instant,
    max_duration: Duration,
}

#[derive(Debug, Clone, Copy)]
enum BridgeStatus {
    CallerEnded,
    DeepgramEnded,
    SessionTimedOut,
}

enum BridgeEvent {
    Caller(Option<Result<ClientMessage, axum::Error>>),
    Deepgram(Option<Result<DeepgramMessage, DeepgramSinkError>>),
    CallerSink(Result<(), CallerSinkError>),
    DeepgramSink(Result<(), DeepgramSinkError>),
    TimedOut,
}

/// Runs the media loop for debug and Telnyx sessions.
pub(crate) async fn handle_media_session(
    socket: WebSocket,
    state: AppState,
    mode: &'static str,
    caller_last4: String,
) {
    let mut guard = SessionGuard::new(
        state.active_sessions.clone(),
        state.metrics.clone(),
        mode,
        caller_last4.clone(),
    );
    let max_duration = Duration::from_secs(state.config.server.max_session_duration_seconds);
    let started = Instant::now();
    let (mut caller_write, mut caller_read) = socket.split();

    if mode == "debug" {
        let connected_message =
            ClientMessage::Text("{\"event\":\"connected\",\"version\":\"1.0.0\"}".into());
        if let Err(error) = send_caller_messages(&mut caller_write, [connected_message]).await {
            warn!(%error, "failed to send connected message");
            guard.set_terminal("error", "caller_connect");
            drop(guard);
            return;
        }
    }

    let mut reconnect_attempts = 0usize;
    let mut session = SessionContext {
        speaking_speed: state.config.deepgram.speak_speed,
    };

    loop {
        if started.elapsed() >= max_duration {
            let _ = send_caller_messages(
                &mut caller_write,
                [ClientMessage::Text(telnyx::build_clear_message().into())],
            )
            .await;
            guard.set_terminal("ended", "timeout");
            break;
        }

        let connect_timer = state.metrics.observe_deepgram_connect_timer();
        let deepgram = match connect(&state.config.deepgram, &state.deepgram_api_key).await {
            Ok(stream) => stream,
            Err(error) => {
                warn!(%error, mode, "failed to connect to Deepgram");
                state.metrics.inc_deepgram_error("connect", &caller_last4);
                let _ = play_pcmu_asset(&mut caller_write, FALLBACK_PCMU).await;
                connect_timer.observe_duration();
                guard.set_terminal("error", "deepgram_connect");
                break;
            }
        };
        connect_timer.observe_duration();

        match bridge_round(
            &mut caller_write,
            &mut caller_read,
            deepgram,
            &state,
            &mut session,
            BridgeTiming {
                started,
                max_duration,
            },
            &caller_last4,
        )
        .await
        {
            Ok(BridgeStatus::CallerEnded) => break,
            Ok(BridgeStatus::SessionTimedOut) => {
                guard.set_terminal("ended", "timeout");
                break;
            }
            Ok(BridgeStatus::DeepgramEnded) => {
                if reconnect_attempts >= state.config.deepgram.transient_reconnect_attempts {
                    let _ = play_pcmu_asset(&mut caller_write, FALLBACK_PCMU).await;
                    guard.set_terminal("error", "deepgram_disconnect");
                    break;
                }
                reconnect_attempts += 1;
                info!(
                    reconnect_attempts,
                    "Deepgram disconnected, attempting reconnect"
                );
                let _ = play_pcmu_asset(&mut caller_write, RECONNECTING_PCMU).await;
            }
            Err(error) => {
                error!(%error, "media bridge failed");
                state.metrics.inc_deepgram_error("bridge", &caller_last4);
                let _ = play_pcmu_asset(&mut caller_write, FALLBACK_PCMU).await;
                guard.set_terminal("error", "bridge");
                break;
            }
        }
    }

    let _ = send_caller_messages(&mut caller_write, [ClientMessage::Close(None)]).await;
    drop(guard);
}

async fn bridge_round(
    caller_write: &mut CallerSink,
    caller_read: &mut CallerRead,
    deepgram: DeepgramStream,
    state: &AppState,
    session: &mut SessionContext,
    timing: BridgeTiming,
    caller_last4: &str,
) -> Result<BridgeStatus> {
    let (mut deepgram_write, mut deepgram_read) = deepgram.split();
    let mut caller_queue = CallerQueue::default();
    let mut deepgram_queue = DeepgramQueue::default();
    let mut pending_exit = None;
    let deadline = sleep_until(timing.started + timing.max_duration);
    tokio::pin!(deadline);
    info!("media bridge started");

    loop {
        if let Some(status) = pending_exit
            && terminal_queues_are_idle(status, &caller_queue, &deepgram_queue)
        {
            return Ok(status);
        }

        let event = {
            let mut caller_sink_ready = caller_queue.ready_or_flushed(caller_write);
            let mut deepgram_sink_ready = deepgram_queue.ready_or_flushed(&mut deepgram_write);

            select! {
                caller = caller_read.next(), if pending_exit.is_none() => BridgeEvent::Caller(caller),
                deepgram = deepgram_read.next(), if pending_exit.is_none() => BridgeEvent::Deepgram(deepgram),

                Some(result) = &mut caller_sink_ready => BridgeEvent::CallerSink(result),
                Some(result) = &mut deepgram_sink_ready => BridgeEvent::DeepgramSink(result),

                () = &mut deadline, if pending_exit.is_none() => BridgeEvent::TimedOut,
            }
        };

        match event {
            BridgeEvent::CallerSink(result) => {
                result?;
                caller_queue.send_or_note_flushed(caller_write, |message| message)?;
            }
            BridgeEvent::DeepgramSink(result) => {
                result?;
                deepgram_queue.send_or_note_flushed(&mut deepgram_write, |message| message)?;
            }
            BridgeEvent::Caller(message_result) => {
                let Some(message_result) = message_result else {
                    deepgram_queue.push(DeepgramMessage::Close(None));
                    pending_exit = Some(BridgeStatus::CallerEnded);
                    continue;
                };
                let message = message_result?;
                if !handle_caller_message(message, &mut deepgram_queue) {
                    deepgram_queue.push(DeepgramMessage::Close(None));
                    pending_exit = Some(BridgeStatus::CallerEnded);
                }
            }
            BridgeEvent::Deepgram(message_result) => {
                let Some(message_result) = message_result else {
                    return Ok(BridgeStatus::DeepgramEnded);
                };
                let message = match message_result {
                    Ok(value) => value,
                    Err(error) => {
                        warn!(%error, "deepgram websocket receive error");
                        return Ok(BridgeStatus::DeepgramEnded);
                    }
                };
                if !handle_deepgram_message(
                    message,
                    &mut caller_queue,
                    &mut deepgram_queue,
                    state,
                    session,
                    caller_last4,
                )? {
                    pending_exit = Some(BridgeStatus::CallerEnded);
                }
            }
            BridgeEvent::TimedOut => {
                caller_queue.push(ClientMessage::Text(telnyx::build_clear_message().into()));
                deepgram_queue.push(DeepgramMessage::Close(None));
                pending_exit = Some(BridgeStatus::SessionTimedOut);
            }
        }
    }
}

fn terminal_queues_are_idle(
    status: BridgeStatus,
    caller_queue: &CallerQueue,
    deepgram_queue: &DeepgramQueue,
) -> bool {
    match status {
        BridgeStatus::CallerEnded => deepgram_queue.is_idle(),
        BridgeStatus::DeepgramEnded => true,
        BridgeStatus::SessionTimedOut => caller_queue.is_idle() && deepgram_queue.is_idle(),
    }
}

fn handle_caller_message(message: ClientMessage, deepgram_queue: &mut DeepgramQueue) -> bool {
    match message {
        ClientMessage::Text(text) => {
            let event = match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(error) => {
                    warn!(%error, "failed to parse caller media message");
                    return true;
                }
            };
            match event {
                telnyx::Event::Media(media) => {
                    if matches!(&media.media.track, Some(telnyx::MediaTrack::Outbound)) {
                        return true;
                    }
                    let payload = match telnyx::decode_media_payload(&media) {
                        Ok(value) => value,
                        Err(error) => {
                            warn!(%error, "caller media payload decode failed");
                            return true;
                        }
                    };
                    deepgram_queue.push(DeepgramMessage::Binary(payload.into()));
                }
                telnyx::Event::Stop(_) => return false,
                telnyx::Event::Error(error_event) => {
                    warn!(?error_event, "caller reported media error");
                }
                _ => {}
            }
            true
        }
        ClientMessage::Close(_) => false,
        ClientMessage::Binary(_) | ClientMessage::Ping(_) | ClientMessage::Pong(_) => true,
    }
}

fn handle_deepgram_message(
    message: DeepgramMessage,
    caller_queue: &mut CallerQueue,
    deepgram_queue: &mut DeepgramQueue,
    state: &AppState,
    session: &mut SessionContext,
    caller_last4: &str,
) -> Result<bool> {
    match message {
        DeepgramMessage::Binary(audio) => {
            let payload = telnyx::build_media_message(&audio)?;
            caller_queue.push(ClientMessage::Text(payload.into()));
            Ok(true)
        }
        DeepgramMessage::Text(text) => {
            match serde_json::from_str(&text) {
                Ok(deepgram::ServerMessage::UserStartedSpeaking) => {
                    caller_queue.push(ClientMessage::Text(telnyx::build_clear_message().into()));
                }
                Ok(deepgram::ServerMessage::Welcome(welcome)) => {
                    info!(request_id = %welcome.request_id, "Deepgram welcome received");
                }
                Ok(deepgram::ServerMessage::AgentStartedSpeaking(latency)) => {
                    info!(
                        total_latency = latency.total,
                        tts_latency = latency.tts,
                        ttt_latency = latency.ttt,
                        "Deepgram agent started speaking"
                    );
                }
                Ok(deepgram::ServerMessage::AgentAudioDone) => {
                    info!("Deepgram agent audio done");
                }
                Ok(deepgram::ServerMessage::FunctionCallRequest(call)) => {
                    if !handle_tool_calls(call, deepgram_queue, session, state)? {
                        return Ok(false);
                    }
                }
                Ok(deepgram::ServerMessage::Error(status)) => {
                    error!(code = %status.code, description = %status.description, "Deepgram returned error");
                    state
                        .metrics
                        .inc_deepgram_error("server_error", caller_last4);
                }
                Ok(deepgram::ServerMessage::Warning(status)) => {
                    warn!(code = %status.code, description = %status.description, "Deepgram returned warning");
                }
                Ok(other) => {
                    trace!(?other, "ignored Deepgram control event");
                }
                Err(error) => {
                    error!(%error, "failed to parse Deepgram control event");
                }
            }
            Ok(true)
        }
        DeepgramMessage::Close(_) => Ok(false),
        DeepgramMessage::Ping(payload) => {
            deepgram_queue.push(DeepgramMessage::Pong(payload));
            Ok(true)
        }
        DeepgramMessage::Pong(_) | DeepgramMessage::Frame(_) => Ok(true),
    }
}

fn handle_tool_calls(
    request: deepgram::FunctionCallRequestMessage,
    deepgram_queue: &mut DeepgramQueue,
    session: &mut SessionContext,
    state: &AppState,
) -> Result<bool> {
    for call in request.functions {
        if !call.client_side {
            continue;
        }
        if !handle_tool_call(&call, deepgram_queue, session, state)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn handle_tool_call(
    call: &deepgram::FunctionCallRequest,
    deepgram_queue: &mut DeepgramQueue,
    session: &mut SessionContext,
    state: &AppState,
) -> Result<bool> {
    if call.name == "set_speaking_rate" {
        let speed = parse_speaking_rate(call).unwrap_or(session.speaking_speed);
        session.speaking_speed = speed;

        let update = build_update_speak_message(&state.config.deepgram, speed)?;
        deepgram_queue.push(DeepgramMessage::Text(update.into()));
        let response = build_function_response(
            &call.id,
            &call.name,
            "Listo. Ajuste la velocidad y continuo con la conversacion.",
        )?;
        deepgram_queue.push(DeepgramMessage::Text(response.into()));
        return Ok(true);
    }

    if call.name == "end_call" {
        let response = build_function_response(
            &call.id,
            &call.name,
            "Entendido. Cierro la sesion de forma segura.",
        )?;
        deepgram_queue.push(DeepgramMessage::Text(response.into()));
        info!("received end_call tool request");
        return Ok(false);
    }

    let response = build_function_response(&call.id, &call.name, "Unknown function name")?;
    deepgram_queue.push(DeepgramMessage::Text(response.into()));
    Ok(true)
}

fn parse_speaking_rate(call: &deepgram::FunctionCallRequest) -> Option<f32> {
    call.arguments
        .deserialize::<SetSpeakingRateArgs>()
        .ok()
        .map(|args| args.speed.clamp(0.7, 1.3))
}

async fn play_pcmu_asset(caller_write: &mut CallerSink, asset: &[u8]) -> Result<()> {
    for chunk in asset.chunks(FRAME_SAMPLES) {
        let text = telnyx::build_media_message(chunk)?;
        send_caller_messages(caller_write, [ClientMessage::Text(text.into())]).await?;
        sleep(Duration::from_millis(FRAME_PACE_MS)).await;
    }
    Ok(())
}

async fn send_caller_messages<I>(caller_write: &mut CallerSink, messages: I) -> Result<()>
where
    I: IntoIterator<Item = ClientMessage>,
{
    let mut queue = CallerQueue::default();
    queue.extend(messages);
    while !queue.is_idle() {
        let Some(result) = queue.ready_or_flushed(caller_write).await else {
            continue;
        };
        result?;
        queue.send_or_note_flushed(caller_write, |message| message)?;
    }
    Ok(())
}
