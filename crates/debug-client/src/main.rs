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

mod audio;
mod cli;
mod config;
mod ws;

use anyhow::{Context, Result};
use audio::{AudioFrameSink, AudioFrameSource, AudioOutput};
use futures_util::StreamExt;
use language_coach_protocol::telnyx;
use tokio::time::{Duration, Instant, interval};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::{
    audio::AudioInput,
    cli::Cli,
    config::{OutputTarget, WebSocketUrl, load_config},
    ws::OutgoingFrame,
};

#[tokio::main]
async fn main() -> Result<()> {
    init_tls_provider();
    let cli = <Cli as clap::Parser>::parse();
    let mut config = load_config(&cli.config)?;
    if let Some(override_url) = cli.url {
        config.server_url = WebSocketUrl::parse(&override_url)?;
    }

    let token = std::env::var("DIRECT_ACCESS_TOKEN")
        .context("DIRECT_ACCESS_TOKEN must be set in environment")?;
    let request = ws::build_ws_request(&config.server_url, &token, cli.auth_mode)?;
    let mut input: Box<dyn AudioFrameSource> = Box::new(AudioInput::new(&config.input)?);

    let output = if config.output == OutputTarget::Speaker {
        Some(AudioOutput::new()?)
    } else {
        None
    };

    let (stream, response) = connect_async(request).await?;
    eprintln!(
        "connected: {}; input: {:?}; output: {:?}",
        response.status(),
        config.input,
        config.output
    );
    let (write, mut read) = stream.split();
    let outgoing = ws::spawn_writer_task(write);
    let mut ticker = interval(Duration::from_millis(20));
    let mut shutdown_requested = false;
    let mut outbound_audio_frames = 0usize;
    let mut inbound_audio_frames = 0usize;
    let mut last_waiting_for_input_log = Instant::now();

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if !shutdown_requested && let Some(frame) = input.next_frame() {
                    outbound_audio_frames += 1;
                    if outbound_audio_frames == 1 || outbound_audio_frames.is_multiple_of(250) {
                        eprintln!("sent audio frames: {outbound_audio_frames}; latest bytes: {}", frame.len());
                    }
                    outgoing.send(OutgoingFrame::Media(frame)).context("websocket writer task stopped")?;
                } else if !shutdown_requested
                    && outbound_audio_frames == 0
                    && last_waiting_for_input_log.elapsed() >= Duration::from_secs(2)
                {
                    eprintln!("waiting for audio input frames");
                    last_waiting_for_input_log = Instant::now();
                }
            }
            maybe_message = read.next() => {
                let Some(result) = maybe_message else {
                    break;
                };
                let message = match result {
                    Ok(message) => message,
                    Err(tokio_tungstenite::tungstenite::Error::Io(error))
                        if error.kind() == std::io::ErrorKind::ConnectionReset =>
                    {
                        break;
                    }
                    Err(error) => return Err(error.into()),
                };
                match message {
                    Message::Text(text) => {
                        let event: telnyx::Event = serde_json::from_str(&text)?;
                        if let telnyx::Event::Media(media) = &event {
                            inbound_audio_frames += 1;
                            let bytes = media.media.payload.len();
                            if inbound_audio_frames == 1 || inbound_audio_frames.is_multiple_of(250) {
                                eprintln!("received audio frames: {inbound_audio_frames}; latest bytes: {bytes}");
                            }
                        }
                        handle_incoming_event(event, output.as_ref().map(|output| output as &dyn AudioFrameSink));
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            result = tokio::signal::ctrl_c() => {
                result?;
                if !shutdown_requested {
                    shutdown_requested = true;
                    let _ = outgoing.send(OutgoingFrame::Stop);
                }
            }
        }
    }

    Ok(())
}

fn init_tls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn handle_incoming_event(event: telnyx::Event, output: Option<&dyn AudioFrameSink>) {
    match event {
        telnyx::Event::Media(media) => {
            let payload = match telnyx::decode_media_payload(&media) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("failed to decode media payload: {error}");
                    return;
                }
            };
            if let Some(output) = output {
                output.push_mulaw_frame(&payload);
            }
        }
        telnyx::Event::Error(error_event) => {
            eprintln!("server reported error event: {error_event:?}");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::{audio::AudioFrameSink, handle_incoming_event};

    struct FakeAudioSink {
        frames: Mutex<Vec<Vec<u8>>>,
    }

    impl AudioFrameSink for FakeAudioSink {
        fn push_mulaw_frame(&self, payload: &[u8]) {
            if let Ok(mut frames) = self.frames.lock() {
                frames.push(payload.to_vec());
            }
        }
    }

    #[test]
    fn routes_incoming_media_to_audio_sink() {
        let text = language_coach_protocol::telnyx::build_media_message(&[1, 2, 3]);
        assert!(text.is_ok());
        let Ok(text) = text else {
            return;
        };
        let event = serde_json::from_str(&text);
        assert!(event.is_ok());
        let Ok(event) = event else {
            return;
        };
        let sink = FakeAudioSink {
            frames: Mutex::new(Vec::new()),
        };

        handle_incoming_event(event, Some(&sink));

        let frames = sink.frames.lock();
        assert!(frames.is_ok());
        let Ok(frames) = frames else {
            return;
        };
        assert_eq!(frames.as_slice(), &[vec![1, 2, 3]]);
    }
}
