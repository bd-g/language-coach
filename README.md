# Language Coach

A phone-based bilingual conversation coach. Each call starts by asking whether the caller wants to practice English (coached in Mexican Spanish) or Spanish (coached in English). It is built as a [Pipecat](https://docs.pipecat.ai) voice bot and deployed to Pipecat Cloud.

- **Pipeline:** Deepgram Flux speech-to-text (`flux-general-multi`, English and Spanish), then an OpenAI LLM, then a configurable TTS provider (Cartesia, Deepgram Aura-2, or ElevenLabs).
- **Phone:** a Daily number through Pipecat Cloud's managed dial-in. Only allowlisted callers get a coaching session.
- **Memory:** for phone callers, the coach remembers phrases it taught, recurring mistakes and progress from earlier calls (stored in Mem0, separately per practice language). It refers back to them and compliments improvement over time.
- **Tools:** the coach records the chosen practice language (which also sets the voice's locale), changes its speaking speed, and ends the call. Sessions are limited to 30 minutes.

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
  practice.py             practice language (English or Spanish) per session
  memory.py               Mem0-backed learner memory: recall on language choice, save at session end
  memory_admin.py         CLI to show or delete a learner's memories
  tools.py                choose_practice_language, set_speaking_rate and end_call tools
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

## Learner memory

Memory is keyed by caller phone number and practice language. When the caller picks a language, one Mem0 lookup loads their notes into the conversation. When the session ends, including when the caller hangs up abruptly, the conversation is sent to Mem0 once, and Mem0 extracts phrases taught, recurring mistakes, progress and preferences. Browser sessions have no caller identity, so they don't use memory.

Review or delete a learner's memories (reads `MEM0_API_KEY` from `.env`):

```bash
python -m language_coach.memory_admin show +15551234567
python -m language_coach.memory_admin delete +15551234567 --language spanish
```

## Configuration

- `config/bot.toml`: session limit, speech-to-text model and language hints, LLM model, and the active TTS provider. See `docs/tts-providers.md` to switch voices or providers.
- Environment: `DEEPGRAM_API_KEY`, `OPENAI_API_KEY`, the selected TTS provider's key, `MEM0_API_KEY` (while `[memory] enabled = true`), and `ALLOWED_CALLERS`. Startup fails and names any missing variable.

## Deployment

Merging to `main` deploys through the Pipecat Cloud GitHub integration. First-time setup, secrets, the phone number, and branch protection are covered in `docs/production-setup.md`.
