"""Typed bot configuration: tunables from `config/bot.toml`, secrets from the environment."""

import tomllib
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Annotated, ClassVar, Literal

from pipecat.transcriptions.language import Language
from pydantic import BaseModel, ConfigDict, Field, PositiveInt, SecretStr, ValidationError

from language_coach.caller import Allowlist, InvalidPhoneNumberError

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CONFIG_PATH = PROJECT_ROOT / "config" / "bot.toml"
PROMPTS_DIR = Path(__file__).resolve().parent / "prompts"


class ConfigError(Exception):
    """Configuration is missing or invalid. Messages name settings, never secret values."""


class _Strict(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)


class SessionConfig(_Strict):
    """Per-session limits."""

    max_duration_seconds: PositiveInt
    time_limit_message: str = Field(min_length=1)


class SttConfig(_Strict):
    """Deepgram Flux speech-to-text settings."""

    model: str = Field(min_length=1)
    language_hints: list[Language]


class LlmConfig(_Strict):
    """OpenAI Responses LLM settings."""

    model: str = Field(min_length=1)


class CartesiaTts(_Strict):
    """Cartesia Sonic voice. `language` is a Cartesia language or locale code such as `es-MX`."""

    api_key_env: ClassVar[str] = "CARTESIA_API_KEY"

    provider: Literal["cartesia"]
    model: str = Field(min_length=1)
    voice: str = Field(min_length=1)
    language: str = Field(min_length=1)


class DeepgramTts(_Strict):
    """Deepgram Aura voice. Aura-2 Spanish voices code-switch with English."""

    api_key_env: ClassVar[str] = "DEEPGRAM_API_KEY"

    provider: Literal["deepgram"]
    voice: str = Field(min_length=1)


class ElevenLabsTts(_Strict):
    """ElevenLabs voice. `language` is optional; multilingual models detect it from text."""

    api_key_env: ClassVar[str] = "ELEVENLABS_API_KEY"

    provider: Literal["elevenlabs"]
    model: str = Field(min_length=1)
    voice: str = Field(min_length=1)
    language: str | None = None


type TtsConfig = CartesiaTts | DeepgramTts | ElevenLabsTts


class BotConfig(_Strict):
    """Contents of `config/bot.toml`."""

    session: SessionConfig
    stt: SttConfig
    llm: LlmConfig
    tts: Annotated[CartesiaTts | DeepgramTts | ElevenLabsTts, Field(discriminator="provider")]


@dataclass(frozen=True, slots=True)
class Secrets:
    """Credentials and the caller allowlist, read from Pipecat Cloud secrets or `.env`."""

    deepgram_api_key: SecretStr
    openai_api_key: SecretStr
    tts_api_key: SecretStr
    allowlist: Allowlist


@dataclass(frozen=True, slots=True)
class AppConfig:
    """Everything a session needs, validated once at startup."""

    bot: BotConfig
    secrets: Secrets
    system_prompt: str
    greeting: str


def load_app_config(env: Mapping[str, str], config_path: Path = DEFAULT_CONFIG_PATH) -> AppConfig:
    """Load and validate configuration, failing fast with every problem named."""
    try:
        bot = BotConfig.model_validate(tomllib.loads(config_path.read_text(encoding="utf-8")))
    except (OSError, tomllib.TOMLDecodeError, ValidationError) as error:
        raise ConfigError(f"invalid {config_path.name}: {error}") from error

    return AppConfig(
        bot=bot,
        secrets=_load_secrets(env, bot.tts),
        system_prompt=_read_prompt("system_prompt.md"),
        greeting=_read_prompt("greeting.txt"),
    )


def _load_secrets(env: Mapping[str, str], tts: TtsConfig) -> Secrets:
    names = ("DEEPGRAM_API_KEY", "OPENAI_API_KEY", tts.api_key_env, "ALLOWED_CALLERS")
    values = {name: env.get(name, "").strip() for name in names}
    missing = sorted({name for name, value in values.items() if not value})
    if missing:
        raise ConfigError(f"missing environment variables: {', '.join(missing)}")

    try:
        allowlist = Allowlist.parse(values["ALLOWED_CALLERS"])
    except InvalidPhoneNumberError as error:
        raise ConfigError(f"invalid ALLOWED_CALLERS: {error}") from None

    return Secrets(
        deepgram_api_key=SecretStr(values["DEEPGRAM_API_KEY"]),
        openai_api_key=SecretStr(values["OPENAI_API_KEY"]),
        tts_api_key=SecretStr(values[tts.api_key_env]),
        allowlist=allowlist,
    )


def _read_prompt(name: str) -> str:
    text = (PROMPTS_DIR / name).read_text(encoding="utf-8").strip()
    if not text:
        raise ConfigError(f"prompt {name} is empty")
    return text
