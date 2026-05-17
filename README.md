# Language Coach

## Local Server With Debug Client

Run Rust commands from the project nix shell so the audio and linker dependencies are available:

```bash
nix-shell
```

Create local config files. Files matching `*.local.toml` are ignored by git.

```bash
cp config/server.example.toml server.local.toml
cp config/debug-client.example.toml debug-client.local.toml
```

Edit `server.local.toml` for local debug use:

```toml
public_base_url = "https://language-coach.example.com"
telnyx_public_key = "5866666666666666666666666666666666666666666666666666666666666666"
allowlist_numbers = ["+15551234567"]

[server]
bind_addr = "127.0.0.1:8080"
metrics_bind_addr = "127.0.0.1:9091"
```

The Telnyx values above only satisfy startup validation for direct debug sessions. Use real Telnyx values when testing webhooks or phone calls.

Set server environment variables. `DIRECT_ACCESS_TOKEN` must match in the server and debug-client terminals.

```bash
export DIRECT_ACCESS_TOKEN="local-debug-token"
export DEEPGRAM_API_KEY="<real-deepgram-api-key>"
export TELNYX_API_KEY="unused-for-direct-debug"
```

Start the server:

```bash
RUST_LOG=info cargo run -p language-coach-server -- --config server.local.toml
```

In a second nix shell, start the debug client:

```bash
export DIRECT_ACCESS_TOKEN="local-debug-token"
cargo run -p language-coach-debug-client -- --config debug-client.local.toml
```

For a repeatable WAV input instead of the microphone, set `input` in `debug-client.local.toml`:

```toml
input = "wav:/absolute/path/to/input.wav"
output = "speaker"
```

To target a deployed server, keep using the local client config for audio settings and override only the WebSocket URL:

```bash
export DIRECT_ACCESS_TOKEN="<deployed-direct-access-token>"
cargo run -p language-coach-debug-client -- \
  --config debug-client.local.toml \
  --url wss://language-coach.brentdgeorge.com/dev/connect
```
