# Language Coach

A phone-based English conversation coach for native Spanish speakers from Mexico. It is built as a [Pipecat](https://docs.pipecat.ai) voice bot and deployed to Pipecat Cloud.

- **Pipeline:** Deepgram Flux speech-to-text (`flux-general-multi`, English and Spanish), then an OpenAI LLM, then a configurable TTS provider (Cartesia, Deepgram Aura-2, or ElevenLabs).
- **Phone:** a Daily number through Pipecat Cloud's managed dial-in. Only allowlisted callers get a coaching session.
- **Tools:** the coach can change its speaking speed and end the call. Sessions are limited to 30 minutes.

## Layout

```text
bot.py                    Pipecat Cloud entry point (loads config, calls run_session)
language_coach/
  config.py               config/bot.toml + environment → typed AppConfig
  caller.py               E.164 caller numbers and the allowlist
  origin.py               runner arguments → PhoneCall | DailyRoomSession | WebRtcSession
  admission.py            allowlist decision for phone calls
  session.py              reject unknown callers, or run the coaching pipeline
  failures.py             when a failing provider should end the session
  tools.py                set_speaking_rate and end_call tools
  tts.py                  TTS provider construction and speed mapping
  usage.py                per-session TTS character and LLM token totals
  prompts/                system prompt and greeting
config/bot.toml           non-secret tunables (models, voice, session limit)
docs/                     production setup and TTS provider notes
```

## Development

Everything runs in the Nix shell. Python dependencies come from `uv.lock` through uv2nix.

```bash
nix-shell
cp .env.example .env      # fill in API keys
python bot.py             # local playground at http://localhost:7860
```

Open `http://localhost:7860`, connect, and talk to the coach in the browser. Local browser sessions skip the phone allowlist.

Checks (the same ones CI runs):

```bash
uv lock --check
ruff format --check .
ruff check .
pyright
pytest
```

To change dependencies, run `uv add <package>` (or edit `pyproject.toml` and run `uv lock`), then exit and re-enter `nix-shell`.

## Configuration

- `config/bot.toml`: session limit, speech-to-text model and language hints, LLM model, and the active TTS provider. See `docs/tts-providers.md` to switch voices or providers.
- Environment: `DEEPGRAM_API_KEY`, `OPENAI_API_KEY`, the selected TTS provider's key, and `ALLOWED_CALLERS`. Startup fails and names any missing variable.

## Deployment

Merging to `main` deploys through the Pipecat Cloud GitHub integration. First-time setup, secrets, the phone number, and branch protection are covered in `docs/production-setup.md`.
