# Language Coach Voice Agent: Implementation Plan

## Project Overview

Build a Rust voice-agent service for an English conversation coach for native Spanish speakers from Mexico. The agent is publicly reachable through an allowlisted Telnyx phone number and directly reachable through an authenticated debug WebSocket. The production and debug paths intentionally share the same internal session engine so debugging exercises the same audio bridge, Deepgram Voice Agent settings, tools, metrics, and failure behavior as real calls.

Primary goals:

- Excellent live voice-agent experience with low-latency English role-play, Spanish coaching, barge-in, speed adjustment, and graceful call ending.
- Reliable Telnyx production path with explicit authentication, replay protection, allowlisting, per-call media nonce, fallback audio, and graceful shutdown.
- Easy debugging through an included Rust debug client that can connect locally or to Fly.io without Telnyx.
- Strong observability through structured redacted logs, Fly-managed Prometheus metrics, Grafana dashboard JSON, and low-traffic alert rules.
- High testability through protocol adapters, fake WebSocket peers, deterministic signature fixtures, and metrics assertions.

---

## 1. Stack Summary

| Layer | Choice | Justification |
|---|---|---|
| Phone number | Telnyx US local DID | Low-cost inbound PSTN access, Call Control API, media streaming WebSockets, inbound screening. |
| Telephony bridge | Telnyx Call Control + bidirectional media streaming | Telnyx streams JSON WebSocket events with base64 PCMU media payloads. The server adapts those events to Deepgram raw binary audio. |
| Voice agent | Deepgram Voice Agent API at `wss://agent.deepgram.com/v1/agent/converse` | Handles STT, LLM, TTS, turn-taking, function calls, and streamed audio. |
| Hosting | Fly.io single always-on `shared-cpu-1x`, 256 MB RAM, primary region `iad` | Small Rust async server, low expected traffic, lowest operational complexity. |
| Language/runtime | Rust workspace with Tokio | Strong typing for protocol boundaries, low memory use, async WebSocket bridge, testable crates. |
| Debugging | Included Rust CLI debug client | Direct local/Fly debugging without Telnyx while exercising the same PCMU JSON media protocol boundary. |
| Config | TOML config file plus env/Fly secrets | Non-secret operational settings live in typed TOML; secret API tokens stay in env/Fly secrets. |
| Observability | Structured JSON logs + Fly managed Prometheus + Grafana | Good production visibility without adding a separate metrics database. |
| Fallback audio | Embedded raw PCMU clips | In-band fallback/reconnect audio works even when REST/static asset paths are unavailable. |

---

## 2. Repository And Workspace Layout

Use a Cargo workspace from the beginning.

```text
language-coach/
├── Cargo.toml
├── crates/
│   ├── protocol/
│   │   ├── src/telnyx_media.rs
│   │   ├── src/deepgram_agent.rs
│   │   ├── src/pcmu.rs
│   │   └── src/lib.rs
│   ├── server/
│   │   ├── src/agent_config.rs
│   │   ├── src/config.rs
│   │   ├── src/deepgram.rs
│   │   ├── src/main.rs
│   │   ├── src/metrics.rs
│   │   ├── src/routes.rs
│   │   ├── src/session.rs
│   │   ├── src/shutdown.rs
│   │   ├── src/telnyx.rs
│   │   └── assets/
│   │       ├── fallback.pcmu
│   │       └── reconnecting.pcmu
│   └── debug-client/
│       ├── src/audio.rs
│       ├── src/config.rs
│       ├── src/main.rs
│       └── src/ws.rs
├── config/
│   ├── server.example.toml
│   └── debug-client.example.toml
├── observability/
│   └── grafana-dashboard.json
├── .github/workflows/ci.yml
├── Dockerfile
├── fly.toml
└── README.md
```

Crate responsibilities:

- `crates/protocol`: Telnyx-like JSON media envelopes, Deepgram message structs, PCMU conversion helpers, deterministic protocol tests.
- `crates/server`: Axum HTTP/WebSocket server, Telnyx webhooks, Deepgram sessions, tools, metrics, graceful shutdown.
- `crates/debug-client`: CLI for local and remote direct debugging with microphone/WAV input and speaker output.

---

## 3. Production Versus Debug Access Paths

The server has one runtime binary and one internal session engine. “Production mode” and “debug mode” are usage paths, not separate server binaries.

| Path | Entry Point | Auth | Media Protocol | Purpose |
|---|---|---|---|---|
| Production phone call | `POST /telnyx/webhook` then `GET /telnyx/media?call_token=...` | Telnyx Ed25519 webhook signatures, allowlist, per-call media nonce | Telnyx JSON text frames with base64 PCMU payloads | Real PSTN callers. |
| Direct debug | `GET /dev/connect` | `Authorization: Bearer` or browser-compatible WebSocket subprotocol token | Telnyx-like JSON/base64 PCMU subset | Local/Fly debugging without Telnyx. |

Both paths create a `VoiceSession` with:

- `mode = "telnyx" | "debug"`
- `caller_last4 = last 4 digits | "debug" | "unknown"`
- shared concurrency cap
- shared Deepgram settings
- shared barge-in, tools, reconnect, fallback, metrics, and shutdown behavior

### 3.1 Debug WebSocket Protocol

`/dev/connect` uses a Telnyx-like subset, not a separate custom raw-binary protocol. This keeps debugging representative of production and makes protocol tests reusable.

Supported debug frames:

- `start`: declares `stream_id`, audio format `PCMU`, `8000 Hz`, one channel.
- `media`: JSON text frame with `media.payload` containing base64 PCMU bytes.
- `stop`: ends the debug session.
- `clear`: sent by server to stop queued playback on barge-in.
- `mark`: optional playback marker support for parity with Telnyx semantics.

Debug sessions bypass the phone allowlist and are authorized only by the direct-access token. They still count against the same `max_active_sessions = 3` cap as phone calls.

### 3.2 Debug Authentication

Support both CLI-friendly and browser-compatible auth. Never support `?token=` query parameters because URLs are too easily logged.

CLI/tools:

```http
Authorization: Bearer <DIRECT_ACCESS_TOKEN>
```

Browser-compatible WebSocket clients:

```http
Sec-WebSocket-Protocol: language-coach-debug, bearer.<base64url-token>
```

Server behavior:

- Accept either auth mechanism.
- Compare tokens in constant time.
- Select only the `language-coach-debug` subprotocol on success.
- Reject missing or invalid tokens with HTTP 401 before upgrade.
- Redact auth headers and subprotocol token values from logs.

---

## 4. DNS, Domain, And Fly.io Infrastructure

### 4.1 DNS

Add a subdomain CNAME without changing the GitHub Pages apex record.

```text
Type:  CNAME
Name:  language-coach
Value: <your-fly-app-name>.fly.dev.
TTL:   300
```

Provision the certificate after DNS propagates:

```bash
fly certs add language-coach.brentdgeorge.com -a <your-fly-app-name>
```

### 4.2 Fly Machine

Use one always-on machine in `iad`.

```toml
app = "<your-fly-app-name>"
primary_region = "iad"

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory_mb = 256
```

### 4.3 Public HTTP Service

Expose one public app port for webhooks, media WebSockets, debug WebSockets, and health endpoints.

```toml
[[services]]
  internal_port = 8080
  protocol = "tcp"

  [[services.ports]]
    port = 443
    handlers = ["tls", "http"]

  [[services.ports]]
    port = 80
    handlers = ["http"]
    [services.ports.force_https]
      enabled = true
```

### 4.4 Internal Metrics Listener

Expose Prometheus metrics only through Fly's internal custom metrics scraping. Do not map `/metrics` onto the public HTTPS service.

```toml
[metrics]
  port = 9091
  path = "/metrics"
```

The server runs a separate metrics listener on `0.0.0.0:9091`. Fly scrapes it every 15 seconds into Fly managed Prometheus/VictoriaMetrics, queryable through Fly's Prometheus API and `fly-metrics.net` Grafana.

### 4.5 Runtime Config File From Fly Secret

Production config must not be baked into the Docker image because it contains allowlisted phone numbers. Mount it from a base64 Fly secret as a file.

Set the config secret:

```bash
fly secrets set SERVER_CONFIG_TOML="$(base64 -w0 config/production.toml)" -a <your-fly-app-name>
```

Mount the file in `fly.toml`:

```toml
[[files]]
  guest_path = "/app/config/production.toml"
  secret_name = "SERVER_CONFIG_TOML"
```

Run the server with:

```dockerfile
CMD ["/usr/local/bin/language-coach-server", "--config", "/app/config/production.toml"]
```

### 4.6 Required Secrets

Normal runtime environment secrets are limited to API credentials and the direct debug token:

```text
TELNYX_API_KEY
DEEPGRAM_API_KEY
DIRECT_ACCESS_TOKEN
```

`TELNYX_PUBLIC_KEY` is not secret and belongs in the TOML config as `telnyx_public_key`.

`SERVER_CONFIG_TOML` is the one deliberate exception: it is a Fly secret only so Fly can mount the production TOML file without baking allowlisted phone numbers into the image or committing them to git.

---

## 5. Server Configuration

The server reads TOML from `--config <path>`. Local development uses normal files such as `config/server.local.toml`. Production uses `/app/config/production.toml` mounted from a Fly secret.

### 5.1 Required Production Config

Only three fields are required in production. Everything else has typed, validated defaults and optional TOML overrides.

```toml
public_base_url = "https://language-coach.brentdgeorge.com"
telnyx_public_key = "<Telnyx Ed25519 public key>"
allowlist_numbers = ["+15551234567"]
```

### 5.2 Important Defaults

```toml
[server]
bind_addr = "0.0.0.0:8080"
metrics_bind_addr = "0.0.0.0:9091"
max_active_sessions = 3
max_session_duration_seconds = 1800
shutdown_drain_seconds = 10

[deepgram]
url = "wss://agent.deepgram.com/v1/agent/converse"
connect_timeout_ms = 3000
settings_timeout_ms = 3000
transient_reconnect_attempts = 1
listen_model = "flux-general-multi"
listen_version = "v2"
language_hints = ["en", "es"]
think_provider = "open_ai"
think_model = "gpt-5-mini"
speak_provider = "deepgram"
speak_model = "aura-2-selena-es"
speak_speed = 1.0
history_enabled = true
keyterms = []
greeting = "Hola, soy tu coach de conversación en inglés. Vamos a practicar con situaciones reales, como trabajo, viajes, comida, compras o conversaciones casuales. Puedes interrumpirme cuando quieras, preguntarme en español qué significa una palabra, cómo decir algo o pedirme que hable más despacio. Cuando me preguntes algo en español, te contesto en español y luego regresamos a la conversación en inglés. ¿Qué situación te gustaría practicar hoy? Si quieres, también puedo escoger una para ti."

[telnyx]
rest_timeout_ms = 2000
rest_retry_attempts = 2
webhook_replay_window_seconds = 300
dedupe_ttl_seconds = 900
stream_track = "both_tracks"
stream_bidirectional_mode = "rtp"
stream_bidirectional_codec = "PCMU"

[metrics]
enabled = true
```

### 5.3 Startup Validation

The server must fail fast before binding routes if:

- Required config fields are missing.
- `public_base_url` is not HTTPS in production.
- Any allowlist number is not E.164.
- `telnyx_public_key` cannot be decoded/parsed as an Ed25519 public key.
- Required secrets are missing.
- Timeouts, caps, or durations are zero/invalid.
- Deepgram settings are internally inconsistent.
- Embedded PCMU audio assets are empty or invalid length.
- Metrics registry initialization fails.

---

## 6. Telnyx Configuration

### 6.1 Phone Number And Call Control Application

Purchase one US local DID using pay-per-minute billing.

Mission Control settings:

- Webhook URL: `https://language-coach.brentdgeorge.com/telnyx/webhook`
- Webhook API Version: V2 EdDSA signatures
- Assign purchased DID to the Call Control application
- Enable inbound call screening for spam/robocall rejection

### 6.2 Webhook Events

Handle these events at minimum:

| Event | Action |
|---|---|
| `call.initiated` | Verify signature, dedupe event, check allowlist/capacity, generate media nonce, answer with streaming. |
| `call.answered` | Record answered state for metrics/logging. |
| `streaming.started` | Record stream state for metrics/logging. |
| `streaming.stopped` | Mark media stream stopped and clean up if call ended. |
| `call.hangup` | Clean up session state and emit final metrics. |
| `call.playback.ended` | Not used for primary fallback path, but safe to ignore/log. |

Always return 2xx to valid, processed Telnyx webhooks after internal action attempts, so Telnyx does not retry successfully handled events. Return 403 for failed signature/replay validation.

### 6.3 Webhook Signature Verification

Every webhook must pass:

1. Required headers `telnyx-signature-ed25519` and `telnyx-timestamp` are present.
2. Timestamp is within 300 seconds of server time.
3. Signed message is `{timestamp}|{raw_body_bytes}`.
4. Ed25519 signature verifies against `telnyx_public_key` from TOML config.
5. Event ID has not already been processed in the in-memory TTL dedupe cache.

### 6.4 Allowlist And Capacity

On `call.initiated`:

1. Extract caller `from` number.
2. Reject before answer if `from` is not in `allowlist_numbers`.
3. Reject before answer if active sessions are already at `max_active_sessions = 3`.
4. Generate a cryptographically random per-call media nonce.
5. Store pending session state keyed by `call_control_id`.
6. Answer the call with media streaming enabled.

Over-capacity rejects are tracked as metrics but do not alert by default.

### 6.5 Answer Command

Answer allowed calls with a nonce-protected stream URL:

```json
{
  "stream_url": "wss://language-coach.brentdgeorge.com/telnyx/media?call_token=<random_nonce>",
  "stream_track": "both_tracks",
  "stream_bidirectional_mode": "rtp",
  "stream_bidirectional_codec": "PCMU"
}
```

Retry transient Telnyx REST failures up to 2 attempts with short jittered backoff. Do not retry non-retryable auth/config errors.

---

## 7. Telnyx Media WebSocket Protocol

Telnyx media WebSocket frames are JSON text messages, not raw binary audio frames.

Incoming Telnyx media event:

```json
{
  "event": "media",
  "sequence_number": "4",
  "media": {
    "track": "inbound",
    "chunk": "2",
    "timestamp": "5",
    "payload": "<base64 PCMU RTP payload without RTP headers>"
  },
  "stream_id": "..."
}
```

Outbound audio to Telnyx is also JSON:

```json
{
  "event": "media",
  "media": {
    "payload": "<base64 PCMU bytes>"
  }
}
```

The protocol crate must model:

- `connected`
- `start`
- `media`
- `stop`
- `mark`
- `clear`
- `dtmf`
- `error`

Bridge behavior:

- Telnyx `media.payload` base64-decodes to raw PCMU bytes before forwarding to Deepgram as a binary WebSocket frame.
- Deepgram binary audio frames are base64-encoded into Telnyx JSON `media` events before sending to Telnyx.
- Telnyx `error` events trigger session error handling.
- Telnyx media WebSocket must reject missing, unknown, expired, or already-consumed `call_token` values.
- Media chunks may arrive out of order. Initial implementation should track sequence/chunk gaps for metrics and logs; it does not need to reorder unless real calls show this is necessary.

---

## 8. Deepgram Voice Agent Configuration

### 8.1 Audio Settings

Use PCMU end-to-end to avoid server-side transcoding in production.

```json
{
  "type": "Settings",
  "flags": {
    "history": true
  },
  "audio": {
    "input": {
      "encoding": "mulaw",
      "sample_rate": 8000
    },
    "output": {
      "encoding": "mulaw",
      "sample_rate": 8000,
      "container": "none"
    }
  }
}
```

Do not set `mip_opt_out` initially; use Deepgram's default behavior.

The actual first Deepgram client message is one `Settings` object with this top-level shape. The smaller examples below document each decision, but implementation should serialize one combined payload:

```json
{
  "type": "Settings",
  "flags": {
    "history": true
  },
  "audio": {
    "input": {
      "encoding": "mulaw",
      "sample_rate": 8000
    },
    "output": {
      "encoding": "mulaw",
      "sample_rate": 8000,
      "container": "none"
    }
  },
  "agent": {
    "listen": {
      "provider": {
        "type": "deepgram",
        "version": "v2",
        "model": "flux-general-multi",
        "language_hint": ["en", "es"]
      }
    },
    "think": {
      "provider": {
        "type": "open_ai",
        "model": "gpt-5-mini"
      },
      "prompt": "<Appendix A prompt>",
      "functions": ["<set_speaking_rate schema>", "<end_call schema>"]
    },
    "speak": {
      "provider": {
        "type": "deepgram",
        "model": "aura-2-selena-es",
        "speed": 1.0
      }
    },
    "greeting": "<Spanish greeting>"
  }
}
```

### 8.2 STT Settings

Use Flux multilingual for low-latency conversational turn-taking and English/Spanish code-switching.

```json
{
  "listen": {
    "provider": {
      "type": "deepgram",
      "version": "v2",
      "model": "flux-general-multi",
      "language_hint": ["en", "es"]
    }
  }
}
```

No keyterms initially. Add keyterms only after observing real failure patterns.

Start with Deepgram's default Flux turn-taking behavior. Do not set `eot_threshold` or `eager_eot_threshold` in v1. Tune these only after observing real call metrics, debug-client sessions, and user experience notes.

### 8.3 LLM Settings

Default to OpenAI 5-family via Deepgram:

```json
{
  "think": {
    "provider": {
      "type": "open_ai",
      "model": "gpt-5-mini"
    },
    "prompt": "<system prompt>"
  }
}
```

`gpt-5-mini` is the documented default. The model must be optionally overrideable in TOML so a newer or account-specific 5-family model can be tested without code changes.

### 8.4 TTS Settings

Use a Deepgram Aura English/Spanish code-switching voice:

```json
{
  "speak": {
    "provider": {
      "type": "deepgram",
      "model": "aura-2-selena-es",
      "speed": 1.0
    }
  }
}
```

The speed value is adjustable at runtime through the `set_speaking_rate` tool. Valid speed range is `0.7..=1.3` for this app, even if the provider supports a wider range, to avoid unnatural coaching audio.

### 8.5 Function Tools

Implement two client-side functions.

`set_speaking_rate`:

```json
{
  "name": "set_speaking_rate",
  "description": "Adjust the voice agent speaking speed when the learner asks the coach to speak slower, faster, or normally.",
  "parameters": {
    "type": "object",
    "properties": {
      "speed": {
        "type": "number",
        "minimum": 0.7,
        "maximum": 1.3,
        "description": "Speaking speed multiplier. Use 1.0 for normal speed."
      },
      "reason": {
        "type": "string",
        "description": "Short reason for the adjustment."
      }
    },
    "required": ["speed"]
  }
}
```

On function call:

- Clamp/validate speed to `0.7..=1.3`.
- Send Deepgram `UpdateSpeak` with the new speed:

  ```json
  {
    "type": "UpdateSpeak",
    "speak": {
      "provider": {
        "type": "deepgram",
        "model": "aura-2-selena-es",
        "speed": 0.85
      }
    }
  }
  ```

- Store current speed in session state.
- Return a Spanish function result instructing the agent to briefly confirm naturally in Spanish, then resume.

`end_call`:

```json
{
  "name": "end_call",
  "description": "End the session when the learner is done, asks to stop, or the coaching scenario has clearly concluded.",
  "parameters": {
    "type": "object",
    "properties": {
      "reason": {
        "type": "string",
        "description": "Short reason for ending the call."
      }
    },
    "required": ["reason"]
  }
}
```

On function call:

- Record `end_call` metric with reason class.
- Allow a brief final response if already in progress.
- Hang up Telnyx sessions or close debug sessions gracefully.

Unknown function calls return a safe error to Deepgram and increment metrics.

### 8.6 Default System Prompt

The initial system prompt is the English conversation coach prompt for native Spanish speakers from Mexico. Store it as a readable Rust string or external prompt file included at compile time. Use Appendix A as the default prompt text. It must preserve these behavior rules:

- The opening, corrections, pronunciation coaching, explanations, meta-commentary, check-ins, and answers to Spanish questions are always in Mexican Spanish.
- English is reserved for in-character role-play conversation.
- Do not over-correct minor harmless slips.
- Interrupt role-play only for meaningful grammar, word choice, recurring, or pronunciation issues.
- For significant pronunciation/mistranscription issues, coach in Spanish, model the word, ask for a retry, then rewind the role-play.
- Keep turns short, natural, and voice-friendly.
- Output only directly spoken words with no markdown, bullets, symbols, stage directions, or formatting.
- If the learner asks to slow down, speed up, or return to normal, use `set_speaking_rate`.
- If the learner says they are done, wants to stop, or says goodbye in a way that implies ending, use `end_call` after a short natural closing.

The prompt should be reviewed after real call logs/metrics, but transcripts are not persisted by default.

### 8.7 Initial Greeting

Use Deepgram `agent.greeting` so the coach speaks first. The default greeting is in Mexican Spanish and should match the prompt's opening rules:

```text
Hola, soy tu coach de conversación en inglés. Vamos a practicar con situaciones reales, como trabajo, viajes, comida, compras o conversaciones casuales. Puedes interrumpirme cuando quieras, preguntarme en español qué significa una palabra, cómo decir algo o pedirme que hable más despacio. Cuando me preguntes algo en español, te contesto en español y luego regresamos a la conversación en inglés. ¿Qué situación te gustaría practicar hoy? Si quieres, también puedo escoger una para ti.
```

The greeting is configurable in TOML. Keep it concise enough to feel natural over the phone; long onboarding belongs in the system prompt, not in every call opening.

### 8.8 Settings Tuning Workflow

Deepgram settings should be easy to tune without code changes. Keep model, voice, speed, greeting, and selected provider parameters as optional TOML overrides with safe defaults.

Initial defaults:

- STT: `flux-general-multi`, `version = "v2"`, `language_hint = ["en", "es"]`.
- Turn-taking: Deepgram defaults, no explicit EOT thresholds.
- LLM: `open_ai` with `gpt-5-mini`.
- TTS: `deepgram` with `aura-2-selena-es`, `speed = 1.0`.
- Keyterms: none.
- History: enabled for in-memory reconnect only.

Tuning loop after production/debug sessions:

1. Use debug-client sessions to test prompt or voice changes before deploying.
2. Check Grafana panels for Deepgram setup latency, first-audio latency, reconnects, fallbacks, barge-ins, and per-last4 failure patterns.
3. If turn-taking feels too eager or too slow, add explicit Flux EOT settings as TOML overrides and test with the debug client first.
4. If Spanish coaching or code-switching sounds unnatural, test alternate Aura Spanish code-switching voices before changing providers.
5. If recognition errors cluster around specific lesson vocabulary, add a small keyterm list in TOML and verify metrics/logs improve.
6. Keep all prompt/settings changes deployable through config or prompt asset changes; avoid hard-coded tuning constants unless they are true safety bounds.

---

## 9. Session Lifecycle And Reliability

### 9.1 Session State

Use typed state that keeps protocol and provider details explicit.

```rust
struct SessionState {
    session_id: SessionId,
    mode: SessionMode,
    call_control_id: Option<CallControlId>,
    stream_id: Option<StreamId>,
    caller_last4: CallerLast4,
    started_at: Instant,
    current_speaking_rate: SpeakingRate,
    deepgram_request_id: Option<String>,
    deepgram_history: Vec<DeepgramHistoryMessage>,
    reconnect_attempted: bool,
    fallback_triggered: bool,
    draining: bool,
}
```

Keep Deepgram history only in memory for the active session. On hangup/session end, drop it. No transcript persistence or session-summary file is written.

### 9.2 Concurrency And Duration Caps

- Shared active session cap: 3 total sessions across Telnyx and debug.
- Over-cap Telnyx calls are rejected before answer.
- Over-cap debug connections are rejected before upgrade.
- Maximum session duration: 30 minutes.
- At max duration, tell the user briefly that the session is ending, then hang up/close.

### 9.3 Deepgram Connect And Reconnect

Initial setup:

- Open Deepgram WebSocket.
- Send `Settings`.
- Require setup/settings application within 3 seconds.
- If setup fails, play bilingual fallback and end the session.

Transient mid-call failure policy:

- Retry once for network drops, timeouts, 429, 5xx, and abnormal WebSocket close.
- Do not retry 400, 401, 403, settings validation errors, or other terminal configuration/auth errors.
- During reconnect, play embedded `reconnecting.pcmu` in-band: a short Spanish/English hold message such as “Un momento, estoy reconectando. One moment, I’m reconnecting.”
- Reconnect within 3 seconds using the full active-session in-memory Deepgram history.
- If reconnect fails, play bilingual fallback and end the session.

### 9.4 Fallback Audio

Commit small raw PCMU assets:

- `crates/server/assets/reconnecting.pcmu`
- `crates/server/assets/fallback.pcmu`

Embed with `include_bytes!` so fallback works without filesystem reads after startup.

Bilingual fallback text:

```text
Something went wrong. Algo salió mal. Please text Brent to let him know.
```

Send fallback in-band over the active Telnyx/debug WebSocket as JSON `media` frames with base64 PCMU payloads, paced at roughly real-time 8 kHz PCMU. Then hang up or close the debug session.

### 9.5 Barge-In

When Deepgram sends `UserStartedSpeaking`:

- Send Telnyx/debug `clear` to stop queued outbound audio.
- Increment a `barge_in_total` metric.
- Keep forwarding caller audio to Deepgram.

### 9.6 Graceful Shutdown

On SIGTERM/SIGINT:

1. Mark server as draining.
2. `/readyz` returns non-ready.
3. Stop accepting new calls/debug sessions.
4. Allow active sessions up to 10 seconds to finish naturally.
5. For remaining active sessions, attempt fallback or polite hangup/close.
6. Exit before Fly forcibly kills the process.

This is required because GitHub Actions auto-deploys `main` to a single always-on Fly machine.

---

## 10. HTTP And WebSocket Routes

```text
POST /telnyx/webhook    Telnyx Call Control events
GET  /telnyx/media      Telnyx media WebSocket; requires valid one-time call_token
GET  /dev/connect       Authenticated direct debug WebSocket
GET  /healthz           Public minimal liveness check
GET  /readyz            Public minimal readiness check
```

No public `/metrics` route on the main app service.

Health behavior:

- `/healthz`: returns 200 if process is running.
- `/readyz`: returns 200 only if config/secrets/assets initialized, metrics ready, and server is not draining.
- Readiness is local-only. Do not call Telnyx or Deepgram from health checks.
- Health responses must not expose secrets, config details, phone numbers, or provider diagnostics.

---

## 11. Debug Client

The debug client is a Rust CLI in `crates/debug-client`.

The first version is terminal-only. Do not build a browser debug page in the initial implementation. Browser-compatible WebSocket auth remains supported at the server protocol level for future tooling, but the included client is a CLI with live local audio.

### 11.1 Config

Read non-secret settings from `--config <path>`. Read token from env.

Example `config/debug-client.example.toml`:

```toml
server_url = "ws://localhost:8080/dev/connect"
input = "microphone"
output = "speaker"

[audio]
sample_rate = 8000
encoding = "pcmu"
```

Secrets:

```text
DIRECT_ACCESS_TOKEN
```

### 11.2 Targets

Support both local and deployed servers:

```bash
cargo run -p language-coach-debug-client -- --config config/debug-client.local.toml
cargo run -p language-coach-debug-client -- --url wss://language-coach.brentdgeorge.com/dev/connect
```

The client sends token auth through WebSocket subprotocol by default and can optionally use `Authorization: Bearer` for non-browser-compatible clients/tools.

### 11.3 Audio

Initial client capabilities:

- Microphone input using `cpal`.
- WAV input using `hound` for repeatable tests/manual demos.
- Speaker output using `cpal`.
- PCMU boundary: client converts mic/WAV PCM to PCMU 8 kHz for the WebSocket protocol and converts received PCMU back to speaker PCM.

Testing approach:

- Put audio capture/playback behind traits.
- Unit-test conversion and protocol behavior with fake audio sources/sinks.
- Manual-test real devices.

---

## 12. Observability

### 12.1 Structured Logs

Use `tracing` with JSON formatting in production.

Log fields:

- `session_id` only in logs, never metrics labels.
- `mode`
- `caller_last4`
- `call_control_id` only when needed for Telnyx debugging; redact/avoid in routine info logs.
- `event_type`
- `provider`
- `duration_ms`
- `error_class`
- `close_code`
- `retry_attempt`

Do not log:

- Full phone numbers.
- Debug tokens or auth headers.
- Deepgram API keys or Telnyx API keys.
- Conversation transcripts by default.
- Raw audio payloads or base64 media.

### 12.2 Prometheus Metrics

Use the `prometheus` crate directly. Serve metrics on internal `0.0.0.0:9091/metrics`; Fly scrapes via `[metrics]`.

Metric labels must stay low-cardinality. Allowed labels:

- `mode = "telnyx" | "debug"`
- `status`
- `reason`
- `provider`
- `event_type`
- `caller_last4`
- `direction = "inbound" | "outbound"`

Never use session IDs, call IDs, full phone numbers, transcripts, raw error strings, or token values as metric labels.

Using `caller_last4` is acceptable for this low-traffic allowlisted personal system. Use `caller_last4="debug"` for debug sessions and `caller_last4="unknown"` when unavailable.

Required custom metrics:

- `language_coach_active_sessions{mode,caller_last4}` gauge
- `language_coach_sessions_total{mode,status,reason,caller_last4}` counter
- `language_coach_session_duration_seconds{mode,status,caller_last4}` histogram
- `language_coach_telnyx_webhooks_total{event_type,status}` counter
- `language_coach_telnyx_webhook_duration_seconds{event_type,status}` histogram
- `language_coach_telnyx_commands_total{command,status,reason}` counter
- `language_coach_deepgram_connect_duration_seconds{status}` histogram
- `language_coach_deepgram_settings_duration_seconds{status}` histogram
- `language_coach_deepgram_reconnects_total{status,reason,caller_last4}` counter
- `language_coach_deepgram_errors_total{reason,caller_last4}` counter
- `language_coach_agent_first_audio_seconds{mode,caller_last4}` histogram
- `language_coach_audio_frames_total{mode,direction,caller_last4}` counter
- `language_coach_audio_bytes_total{mode,direction,caller_last4}` counter
- `language_coach_audio_gap_total{mode,direction,caller_last4}` counter
- `language_coach_barge_in_total{mode,caller_last4}` counter
- `language_coach_fallback_total{mode,reason,caller_last4}` counter
- `language_coach_function_calls_total{name,status,caller_last4}` counter
- `language_coach_over_capacity_total{mode}` counter
- `language_coach_malformed_media_total{mode,reason}` counter
- `language_coach_duplicate_webhooks_total{event_type}` counter

Measure high latency but do not alert on it initially.

### 12.3 Grafana Dashboard

Include `observability/grafana-dashboard.json` with panels for:

- Active sessions by mode.
- Sessions started/ended by status.
- Fallbacks and reconnects.
- Telnyx webhook/command outcomes.
- Deepgram setup latency.
- Agent first-audio latency.
- Audio frames/bytes bridged.
- Audio gaps/malformed media.
- Function calls and speaking-rate changes.
- Limited per-`caller_last4` breakdowns for failures and latency.

### 12.4 Alerts

Use Fly managed Grafana/Prometheus alerting.

Initial alerts:

- App down or `/readyz` unavailable.
- `mode="telnyx"` production errors >= 2 within 1 hour across fallback, reconnect failure, Telnyx command failure, Deepgram terminal error, or unhandled session error.
- Debug failures are shown separately and should not page production alerts.

Alert notifications should go to email only for v1. Do not add Slack, Discord, SMS, PagerDuty, or other notification channels until real usage shows email is insufficient.

Over-capacity rejects are dashboard metrics only at first.

---

## 13. Testing Strategy

### 13.1 Unit Tests

Required unit coverage:

- Telnyx media JSON parse/render.
- Base64 PCMU payload decode/encode.
- PCMU conversion helpers.
- Deepgram `Settings`, `UpdateSpeak`, function-call, history, error, and audio message modeling.
- Config defaults, optional overrides, fail-fast validation.
- E.164 allowlist parsing.
- Speaking-rate validation/clamping.
- Max-duration and capacity policy.
- Metrics registration/rendering.

### 13.2 Telnyx Signature Fixtures

Generate deterministic Ed25519 fixtures and test:

- Valid signature.
- Invalid signature.
- Missing signature header.
- Missing timestamp header.
- Stale timestamp.
- Tampered body.

### 13.3 Fake WebSocket Integration Tests

Use fake Telnyx/debug and fake Deepgram WebSocket peers. Do not call live Telnyx or Deepgram in CI.

Required scenarios:

- Accepted Telnyx call bridges caller audio to Deepgram and agent audio back to Telnyx.
- Allowlist reject before answer.
- Over-capacity reject before answer.
- Debug session bridges through the same internal session engine.
- Deepgram transient disconnect reconnects once with in-memory history.
- Deepgram terminal error triggers fallback and hangup.
- Malformed Telnyx/debug media increments metrics and fails safely.
- Duplicate webhook dedupe prevents duplicate answer/reject commands.
- Barge-in sends `clear`.
- `set_speaking_rate` emits Deepgram `UpdateSpeak` and updates state.
- `end_call` closes Telnyx/debug sessions.
- Metrics assertions for accepted, rejected, fallback, reconnect, malformed media, and debug sessions.

### 13.4 Debug Client Tests

- Abstract audio input/output behind traits.
- Test WAV/PCM to PCMU conversion using fixtures.
- Test received PCMU to output PCM path with fake speaker sink.
- Test auth header/subprotocol construction.
- Manual-test real microphone and speaker devices.

### 13.5 Manual Smoke Tests

Document both modes:

- Local debug-client against local server.
- Deployed debug-client against Fly `/dev/connect`.
- Real Telnyx phone call to the public number.

Each smoke test should state expected logs and metrics to check.

No local tunnel/ngrok/zrok mode is included in the initial plan. Local development uses the debug client; production validation uses Telnyx.

---

## 14. CI/CD

### 14.1 GitHub Actions CI

On pull requests and pushes:

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test --workspace
```

CI must not require Telnyx or Deepgram credentials.

### 14.2 Auto Deploy

On successful push to `main`, deploy to Fly automatically.

GitHub Actions secret:

```text
FLY_API_TOKEN
```

Fly app secrets and mounted production config are pre-provisioned outside GitHub Actions:

```bash
fly secrets set TELNYX_API_KEY="..." DEEPGRAM_API_KEY="..." DIRECT_ACCESS_TOKEN="..."
fly secrets set SERVER_CONFIG_TOML="$(base64 -w0 config/production.toml)"
```

Post-deploy checks:

- `GET https://language-coach.brentdgeorge.com/healthz`
- `GET https://language-coach.brentdgeorge.com/readyz`

Add an optional `workflow_dispatch` job for a deployed debug-client smoke test. Do not run live Deepgram smoke tests after every deploy by default.

---

## 15. Security And Privacy

Authentication summary:

| Vector | Auth |
|---|---|
| Telnyx webhook | Ed25519 signature, timestamp replay window, event dedupe. |
| Telnyx media WS | One-time random per-call `call_token` generated only after verified webhook and allowlist pass. |
| Debug WS | `DIRECT_ACCESS_TOKEN` via Authorization header or WebSocket subprotocol token. |
| Metrics | Internal Fly scrape only; no public route. |
| Health | Public minimal responses only. |

Privacy rules:

- No call recording.
- No transcript persistence.
- No session summary persistence after hangup.
- No full phone numbers in logs or metrics.
- No raw audio in logs.
- No query-token auth.
- Allowlist numbers live in the production TOML config mounted from a Fly secret file, not in git and not baked into the Docker image.

Manual rotation should be documented for:

- `TELNYX_API_KEY`
- `DEEPGRAM_API_KEY`
- `DIRECT_ACCESS_TOKEN`
- `SERVER_CONFIG_TOML`

---

## 16. Deployment Commands

Initial setup:

```bash
fly launch --name language-coach --region iad --vm-size shared-cpu-1x --vm-memory 256
fly certs add language-coach.brentdgeorge.com -a language-coach
```

Set API-token secrets:

```bash
fly secrets set \
  TELNYX_API_KEY="..." \
  DEEPGRAM_API_KEY="..." \
  DIRECT_ACCESS_TOKEN="$(openssl rand -hex 32)" \
  -a language-coach
```

Set mounted production config:

```bash
fly secrets set SERVER_CONFIG_TOML="$(base64 -w0 config/production.toml)" -a language-coach
```

Deploy manually if needed:

```bash
fly deploy -a language-coach
```

Normal production deploys happen automatically from GitHub Actions on successful pushes to `main`.

---

## 17. Cost Summary

Expected fixed infrastructure cost remains low:

| Item | Estimate |
|---|---|
| Fly.io `shared-cpu-1x`, 256 MB, always-on | About $2/month, region dependent. |
| Telnyx local DID | About $1/month. |
| Telnyx inbound calls | Per-minute usage charge. |
| Deepgram Voice Agent | Per-minute usage charge after any credits. |
| Fly managed Prometheus/Grafana | No separate published charge found in Fly docs; custom metrics are part of Fly monitoring. |

Cost controls:

- Allowlist rejects before answer.
- Over-capacity rejects before answer.
- 30-minute max session duration.
- No live Deepgram smoke test on every deploy.
- Low-cardinality Prometheus labels.

---

## 18. Initial Implementation Order

1. Create workspace, config loading, typed defaults, and startup validation.
2. Implement protocol crate for Telnyx-like media frames and PCMU helpers.
3. Implement Deepgram settings/message models and fake-peer test harness.
4. Implement server routes, Telnyx signature verification, allowlist, nonce, and session state.
5. Implement Deepgram bridge, tools, barge-in, reconnect, fallback, and graceful shutdown.
6. Implement Prometheus metrics, structured logs, dashboard JSON, and alert queries.
7. Implement debug client with fake audio tests, WAV input, microphone input, and speaker output.
8. Add integration test matrix.
9. Add Dockerfile, Fly config, mounted config secret docs, and GitHub Actions CI/CD.
10. Run local debug smoke, deployed debug smoke, then real Telnyx phone smoke.

---

## 19. Remaining Pre-Launch Assets

Before first production call:

- Record or generate `fallback.pcmu` and `reconnecting.pcmu` as raw 8 kHz mono PCMU.
- Create `config/production.toml` locally with `public_base_url`, `telnyx_public_key`, and allowlisted numbers.
- Set Fly secrets for API keys, debug token, and mounted config file.
- Import or create the Grafana dashboard.
- Configure Grafana alerts.
- Purchase/configure Telnyx DID and Call Control application.
- Run deployed debug-client smoke test.
- Run real phone-call smoke test.

---

## Appendix A. Default System Prompt

```text
You are a warm, patient, and encouraging English conversation coach for native Spanish speakers from Mexico. Your goal is to help the learner improve spoken English through realistic, interactive role-play conversations. You are a live voice agent, so every response must be directly speakable. Never output markdown, bullet points, numbered lists, asterisks, headers, emojis, brackets, stage directions, code formatting, or any visual formatting. Speak like a real tutor talking out loud.

Core language rule: English is for the in-character role-play conversation. Spanish is for coaching. Whenever you correct, explain, give feedback, discuss pronunciation, answer a Spanish question, check in, set expectations, or talk about the learning process, use Spanish. Do not explain English mistakes in English. Use natural Mexican Spanish with tú forms. Avoid vosotros and expressions that sound foreign in Mexico.

The session starts with a Spanish greeting configured outside this prompt. After the greeting, listen to the learner's answer. If they choose a scenario, begin that scenario. If they are unsure, choose a practical everyday scenario for them and briefly say in Spanish what you chose before starting. Good starter scenarios include ordering food, small talk with a coworker, checking into a hotel, shopping, talking about family news, discussing hobbies, travel, weather, or a simple work conversation.

Before role-play begins, keep any setup in Spanish and keep it short. Once the scenario begins, switch to English and stay in character as the conversation partner. Keep the conversation natural, useful, and interactive. Ask one question at a time. Keep most turns to one to three short sentences so the learner has plenty of chances to speak. Use vocabulary and grammar that fit the learner's apparent level. Start at a moderate beginner-to-intermediate level and adjust based on how they respond.

Default behavior: keep the role-play flowing in English. If the learner says something correct, understandable, or only slightly imperfect, do not stop to praise, grade, or correct. Just respond naturally in character. Over-coaching makes the conversation feel like a classroom drill; avoid it.

Let small mistakes pass when meaning is clear. Examples include a missing article, a small preposition issue, a slight verb ending problem, a minor pronunciation issue that does not block understanding, or a dropped third-person s. Do not mention these unless the same pattern repeats several times or the learner asks.

Interrupt role-play only for meaningful issues. Meaningful issues include wrong word choice that changes the meaning, a tense error that matters for the conversation, a false friend such as embarrassed versus embarazada, a sentence that is hard to understand, a recurring pattern, or a pronunciation problem that blocks communication. When you interrupt, switch to Spanish. Briefly say what the issue was, give the natural English version, explain the key point in simple Spanish, then rewind the moment. After the correction, switch back to English, repeat your previous role-play line, and let the learner try again. When they improve, continue naturally instead of over-praising.

For pronunciation coaching, remember that the transcript may be wrong because of automatic speech recognition. Do not assume every strange transcript is the learner's fault. Use context. If you can reasonably infer the intended word, coach in Spanish: say what word you think they meant, model the pronunciation slowly, point out the one most important sound or stress pattern, and ask them to try the word once. Then return to the role-play. Pay special attention to common challenges for Mexican Spanish speakers: ship versus sheep, live versus leave, schwa, th in think and this, v versus b, initial h in house, y versus j, final consonant clusters, word stress, and adding e before s plus consonant as in espeak or estudent. If you cannot infer what they meant, ask in Spanish for clarification or ask them to say in Spanish what they wanted to express.

If the learner asks something in Spanish during role-play, answer in Spanish. Help with the word, phrase, grammar point, or pronunciation they asked about. Then offer a short bridge back to English, such as: Ahora regresamos a la escena. Then resume the role-play from the previous moment.

Check in occasionally in Spanish after a natural pause or after several minutes of role-play. Ask whether they have questions, want a new scenario, want more corrections, fewer corrections, slower speech, or a different difficulty level. Keep check-ins brief and conversational.

Adapt difficulty continuously. If the learner struggles, simplify your English, slow down through the speaking-rate tool if requested, and use more common vocabulary. If the learner is doing well, introduce slightly richer phrases, idioms, or more realistic pressure, but still keep turns short and clear.

Use the set_speaking_rate tool whenever the learner asks you to speak slower, faster, more clearly, or normally. Use about 0.85 for slower, 1.0 for normal, and 1.15 for faster unless they ask for a stronger change. After the tool succeeds, briefly confirm in Spanish and continue. Do not claim you changed speed unless you used the tool.

Use the end_call tool when the learner clearly wants to stop, says goodbye, says they are done, or the session has naturally concluded and they confirms they do not need anything else. Before ending, say a short warm closing in Spanish. Then call end_call with a concise reason.

If conversation history appears after a reconnect, continue seamlessly from the latest relevant point. Do not mention technical reconnection, WebSockets, Deepgram, Telnyx, logs, metrics, deployment, or system internals to the learner.

Be friendly, human, and calm. Make the learner feel safe making mistakes. You are a coach and conversation partner, not a textbook and not an examiner. Output only spoken words.
```
