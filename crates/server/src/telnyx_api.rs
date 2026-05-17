use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::config::{TelnyxBidirectionalMode, TelnyxConfig, TelnyxStreamTrack};
use language_coach_protocol::telnyx::MediaEncoding;

/// Request body for the Telnyx answer command.
#[derive(Debug, Serialize)]
struct AnswerRequest {
    #[serde(rename = "stream_url")]
    url: String,
    #[serde(rename = "stream_track")]
    track: TelnyxStreamTrack,
    #[serde(rename = "stream_bidirectional_mode")]
    bidirectional_mode: TelnyxBidirectionalMode,
    #[serde(rename = "stream_bidirectional_codec")]
    bidirectional_codec: MediaEncoding,
}

/// Sends `answer` for a call and enables bidirectional media streaming.
pub(crate) async fn answer_call(
    client: &reqwest::Client,
    config: &TelnyxConfig,
    api_key: &str,
    call_control_id: &str,
    stream_url: &str,
) -> Result<()> {
    let payload = AnswerRequest {
        url: stream_url.to_owned(),
        track: config.stream_track.clone(),
        bidirectional_mode: config.stream_bidirectional_mode.clone(),
        bidirectional_codec: config.stream_bidirectional_codec.clone(),
    };
    send_command(
        client,
        config,
        api_key,
        call_control_id,
        "answer",
        Some(&payload),
    )
    .await
}

/// Sends `hangup` for a call control session.
pub(crate) async fn hangup_call(
    client: &reqwest::Client,
    config: &TelnyxConfig,
    api_key: &str,
    call_control_id: &str,
) -> Result<()> {
    send_command::<AnswerRequest>(client, config, api_key, call_control_id, "hangup", None).await
}

async fn send_command<T: Serialize>(
    client: &reqwest::Client,
    config: &TelnyxConfig,
    api_key: &str,
    call_control_id: &str,
    command: &str,
    payload: Option<&T>,
) -> Result<()> {
    let endpoint = format!(
        "{}/calls/{call_control_id}/actions/{command}",
        config.api_base_url.trim_end_matches('/')
    );
    let timeout = Duration::from_millis(config.rest_timeout_ms);
    let attempts = config.rest_retry_attempts.saturating_add(1);

    for attempt in 0..attempts {
        let mut request = client
            .post(&endpoint)
            .timeout(timeout)
            .bearer_auth(api_key)
            .header("content-type", "application/json");
        if let Some(value) = payload {
            request = request.json(value);
        }

        match request.send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) if should_retry_status(response.status()) && attempt + 1 < attempts => {
                tokio::time::sleep(retry_backoff(attempt)).await;
            }
            Ok(response) => {
                anyhow::bail!("Telnyx {command} failed with status {}", response.status());
            }
            Err(error) if error.is_timeout() && attempt + 1 < attempts => {
                tokio::time::sleep(retry_backoff(attempt)).await;
            }
            Err(error) if error.is_connect() && attempt + 1 < attempts => {
                tokio::time::sleep(retry_backoff(attempt)).await;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Telnyx {command} request failed for {call_control_id}")
                });
            }
        }
    }

    anyhow::bail!("Telnyx {command} failed without a terminal response")
}

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

fn retry_backoff(attempt: usize) -> Duration {
    let capped_attempt = attempt.min(5);
    let factor = 1_u64 << capped_attempt;
    Duration::from_millis(75_u64.saturating_mul(factor))
}

#[cfg(test)]
mod tests {
    use super::{retry_backoff, should_retry_status};

    #[test]
    fn retries_only_transient_status_codes() {
        assert!(should_retry_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(should_retry_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(!should_retry_status(reqwest::StatusCode::UNAUTHORIZED));
        assert!(!should_retry_status(reqwest::StatusCode::BAD_REQUEST));
    }

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_backoff(0), std::time::Duration::from_millis(75));
        assert_eq!(retry_backoff(1), std::time::Duration::from_millis(150));
        assert_eq!(retry_backoff(2), std::time::Duration::from_millis(300));
        assert_eq!(retry_backoff(6), std::time::Duration::from_millis(2400));
    }
}
