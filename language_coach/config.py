"""Typed bot configuration: tunables from `config/bot.toml`, secrets from the environment."""

import tomllib
from collections.abc import Mapping
from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path
from typing import Annotated, ClassVar, Literal

from pipecat.transcriptions.language import Language
from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    PositiveFloat,
    PositiveInt,
    SecretStr,
    ValidationError,
)

from language_coach.caller import Allowlist, InvalidPhoneNumberError

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CONFIG_PATH = PROJECT_ROOT / "config" / "bot.toml"
PROMPTS_DIR = Path(__file__).resolve().parent / "prompts"


class ConfigError(Exception):
    """Configuration is missing or invalid. Messages name settings, never secret values."""


class Runtime(StrEnum):
    """Where the bot runs. Krisp noise filtering only exists on Pipecat Cloud."""

    LOCAL = "local"
    CLOUD = "cloud"


class _Strict(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)


class SessionConfig(_Strict):
    """Per-session limits."""

    max_duration_seconds: PositiveInt
    time_limit_message: str = Field(min_length=1)
    failure_message: str = Field(min_length=1)


class MemoryConfig(_Strict):
    """Per-learner long-term memory in Mem0 (phone callers only)."""

    enabled: bool
    timeout_seconds: PositiveFloat
    recall_wait_seconds: PositiveFloat


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
    """Deepgram Flux TTS voice on `/v2/speak`, named `flux-{voice}-{language}`.

    Flux Spanish voices such as `flux-ernesto-es` also speak English. `expressivity`
    runs from -2 (calm) to 2 (animated) and is fixed per connection.
    See https://developers.deepgram.com/docs/flux-tts/feature-overview
    """

    api_key_env: ClassVar[str] = "DEEPGRAM_API_KEY"

    provider: Literal["deepgram"]
    voice: str = Field(pattern=r"^flux-[a-z]+-[a-z]+$")
    expressivity: Literal[-2, -1, 0, 1, 2]


type TtsConfig = CartesiaTts | DeepgramTts


class BotConfig(_Strict):
    """Contents of `config/bot.toml`."""

    session: SessionConfig
    memory: MemoryConfig
    stt: SttConfig
    llm: LlmConfig
    tts: Annotated[CartesiaTts | DeepgramTts, Field(discriminator="provider")]


@dataclass(frozen=True, slots=True)
class Secrets:
    """Credentials and the caller allowlist, read from Pipecat Cloud secrets or `.env`."""

    deepgram_api_key: SecretStr
    openai_api_key: SecretStr
    tts_api_key: SecretStr
    mem0_api_key: SecretStr | None
    allowlist: Allowlist


@dataclass(frozen=True, slots=True)
class AppConfig:
    """Everything a session needs, validated once at startup."""

    bot: BotConfig
    secrets: Secrets
    system_prompt: str
    greeting: str
    runtime: Runtime


def load_app_config(
    env: Mapping[str, str],
    config_path: Path = DEFAULT_CONFIG_PATH,
    runtime: Runtime = Runtime.CLOUD,
) -> AppConfig:
    """Load and validate configuration, failing fast with every problem named."""
    try:
        bot = BotConfig.model_validate(tomllib.loads(config_path.read_text(encoding="utf-8")))
    except (OSError, tomllib.TOMLDecodeError, ValidationError) as error:
        raise ConfigError(f"invalid {config_path.name}: {error}") from error

    return AppConfig(
        bot=bot,
        secrets=_load_secrets(env, bot),
        system_prompt=_read_prompt("system_prompt.md"),
        greeting=_read_prompt("greeting.txt"),
        runtime=runtime,
    )


def _load_secrets(env: Mapping[str, str], bot: BotConfig) -> Secrets:
    names = ["DEEPGRAM_API_KEY", "OPENAI_API_KEY", bot.tts.api_key_env, "ALLOWED_CALLERS"]
    if bot.memory.enabled:
        names.append("MEM0_API_KEY")
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
        tts_api_key=SecretStr(values[bot.tts.api_key_env]),
        mem0_api_key=SecretStr(values["MEM0_API_KEY"]) if bot.memory.enabled else None,
        allowlist=allowlist,
    )


def _read_prompt(name: str) -> str:
    text = (PROMPTS_DIR / name).read_text(encoding="utf-8").strip()
    if not text:
        raise ConfigError(f"prompt {name} is empty")
    return text
