# AGENTS.md

## Engineering Style

- Write small, direct Python. Prefer the simplest correct design over framework, abstraction, or infrastructure growth.
- Lean on the type system. Parse into domain types at boundaries; do not pass loosely validated strings, dicts, or primitive aliases through core logic.
- Avoid duplication aggressively. If two paths exist (phone and browser sessions), share the session boundary rather than copy behavior.
- Keep infrastructure boring: one Pipecat bot on Pipecat Cloud, explicit config, Daily for telephony, and no extra services (webhook servers, databases, metrics stacks) unless the user asks for them.
- Use concise docstrings on public classes and functions that model data flow or provider behavior. Avoid obvious comments in function bodies.
- Keep an explicit running list of unresolved user questions in `QUESTIONS.md` during implementation. Update it as you discover uncertainties, then present that list at the end of a working phase.
- Incorporate user code review feedback into both code changes and this file when the feedback describes a durable engineering preference.
- Keep entrypoints small: `bot.py` only loads config and delegates to `language_coach/`. Group concerns into named modules (`config.py`, `origin.py`, `session.py`, `tools.py`, `tts.py`, `memory.py`) rather than growing one file.
- Minimize per-call cost. Prefer providers and settings that keep TTS characters, LLM tokens, and agent minutes low, and never start AI services for sessions that will be rejected.

## Typing

- `pyright` runs in `strict` mode on `bot.py`, `language_coach/`, and `tests/`. Do not loosen it globally.
- Every `# pyright: ignore[rule]` names the rule and has a short comment explaining the upstream gap. Unnecessary ignores are errors.
- Model domain values as frozen dataclasses, frozen pydantic models, `StrEnum`s, and `Literal`s instead of bare `str`/`float`. Examples: `CallerNumber`, `SpeakingRate`, `EndCallReason`, the `TtsConfig` union.
- Use discriminated unions plus `match` for variants (TTS providers, session origins). Keep `match` exhaustive so pyright flags new variants.
- Do not use `Any` or `dict[str, Any]` in core logic. `Any` is acceptable only where Pipecat's own API is untyped (for example a SmallWebRTC connection handle), and it must not spread further.

## Boundaries And Parsing

- Parse at the edges, trust inside: `config/bot.toml` and the environment go through `config.py`; runner arguments and Daily dial-in bodies go through `origin.py`; LLM tool arguments go through pydantic models in `tools.py`; Mem0 responses and LLM context messages go through pydantic models in `memory.py`.
- External services used inside a call (such as Mem0) sit behind a small `Protocol`, have a timeout, and degrade to "no memory" instead of failing the call.
- Our own config models use `extra="forbid"`. Provider payloads tolerate unknown additive fields but must validate required fields.
- Startup fails fast on invalid config or missing secrets. Error messages name settings and fields, never secret values or phone numbers.
- Tool schemas are explicit `FunctionSchema`s with JSON-schema constraints (`minimum`, `maximum`, `enum`). Handlers validate `params.arguments` into a pydantic model and return a safe error result on invalid input. Do not rely on direct-function signature inference for constrained arguments.
- Document provider quirks and spec links in docstrings on the types that model them. When provider docs change or ambiguity is discovered, update the docstring and `QUESTIONS.md` before broadening types.

## Pipecat Practices

- Pipecat moves fast. Verify every class, import, and setting against the installed package source (`python -c "import pipecat, os; print(os.path.dirname(pipecat.__file__))"`) or current docs before using it. Do not write Pipecat APIs from memory.
- Use current names: `PipelineWorker` with `WorkerRunner`, `EndWorkerFrame` to end from a tool, and service `settings=Service.Settings(...)`. Do not use deprecated `PipelineTask`, `EndTaskFrame`, `params=`/`InputParams`, or init-arg voices.
- Change a running pipeline by pushing frames (`TTSUpdateSettingsFrame`, `TTSSpeakFrame`, `EndFrame`), not by mutating services.
- Use Pipecat's built-in services, transports, and turn handling rather than custom WebSocket or audio code. Deepgram Flux owns turn detection; do not add a VAD unless it is needed.
- Keep the canonical cascade order: transport input, STT, user aggregator, LLM, TTS, transport output, assistant aggregator.
- The base image imports `bot.bot()` from `/app`. Keep that contract, and copy any new top-level directories in the `Dockerfile`.

## Privacy And Logging

- Log callers only as `CallerNumber` or `LearnerId` (both render as the last four digits). Never log full phone numbers, API keys, transcripts, or audio.
- Learner memory is the one place full numbers and conversation content leave the bot: Mem0 user ids contain the E.164 number, and the session's spoken turns are sent to Mem0 once at session end. This is a deliberate user choice; do not add other stores of transcripts or numbers.
- Memory must never add latency to the conversation: look memories up in a background prefetch at session start (one lookup per practice language), wait at most `recall_wait_seconds` for it when the learner picks a language, and save once after the session ends. Do not use Pipecat's per-turn `Mem0MemoryService`; it adds latency and per-turn extraction cost.
- Failures in optional services such as Mem0 are logged and otherwise silent; do not add alerting integrations.
- Log tool outcomes as categories (`EndCallReason`, numeric speed), not free-text arguments.
- Log per-session usage (TTS characters, LLM tokens, duration, TTS provider) at session end so provider costs can be compared.

## Testing

- Use TDD for new behavior: write the smallest failing test first, then implement.
- Prefer deterministic tests. Do not require Deepgram, OpenAI, TTS, Daily, or Pipecat Cloud credentials in CI.
- Test boundaries as first-class behavior: config parsing and failure messages, dial-in body fixtures (valid, malformed, withheld caller ID), tool argument validation, provider speed mapping, and usage accounting.
- Use `pipecat.tests.utils.run_test` for frame processors and observers instead of hand-rolled pipelines.
- Provider fixtures follow documented payloads and cite the doc URL in the test module.

## Verification

- Run all checks inside `nix-shell`: `uv lock --check`, `ruff format --check .`, `ruff check .`, `pyright`, and `pytest`. CI runs the same commands and is a required check for pull requests into `main`.
- Smoke-test startup with `python bot.py` (local playground at `http://localhost:7860`) after changing session wiring. Rebuild the Docker image when changing the `Dockerfile` or dependencies.
- `main` deploys automatically through the Pipecat Cloud GitHub integration, so changes reach `main` only through pull requests.

## Nix And Dependencies

- Use the nix shell for all development. Python dependencies are locked in `uv.lock` and materialized by uv2nix; do not `pip install` or create ad-hoc virtualenvs.
- Add or change Python dependencies with `uv add`/`uv lock` inside the nix shell, then re-enter the shell. Packages that need build fixes go in `nix/overrides.nix`.
- New system dependencies go into `shell.nix` first. Nix inputs are pinned in `nix/sources.nix`.
- Keep the Python minor version in `pyproject.toml`, `shell.nix`, and the `Dockerfile` base image tag in sync.
