from pathlib import Path

import pytest

from language_coach.config import (
    DEFAULT_CONFIG_PATH,
    CartesiaTts,
    ConfigError,
    DeepgramTts,
    Runtime,
    load_app_config,
)

ENV = {
    "DEEPGRAM_API_KEY": "dg-key",
    "OPENAI_API_KEY": "openai-key",
    "CARTESIA_API_KEY": "cartesia-key",
    "ALLOWED_CALLERS": "+15551234567",
    "MEM0_API_KEY": "mem0-key",
}

MINIMAL_TOML = """
[session]
max_duration_seconds = 60
time_limit_message = "Se acabó el tiempo."
failure_message = "Algo salió mal."

[memory]
enabled = {memory_enabled}
timeout_seconds = 5.0
recall_wait_seconds = 0.5

[stt]
model = "flux-general-multi"
language_hints = ["en", "es"]

[llm]
model = "gpt-4.1"

[tts]
{tts}
"""

CARTESIA = 'provider = "cartesia"\nmodel = "sonic-3.6"\nvoice = "voice-id"\nlanguage = "es-MX"'


def write_config(tmp_path: Path, tts: str = CARTESIA, memory_enabled: bool = True) -> Path:
    path = tmp_path / "bot.toml"
    toml = MINIMAL_TOML.format(tts=tts, memory_enabled=str(memory_enabled).lower())
    path.write_text(toml, encoding="utf-8")
    return path


def test_committed_config_loads() -> None:
    config = load_app_config(ENV, DEFAULT_CONFIG_PATH)

    assert isinstance(config.bot.tts, DeepgramTts)
    assert config.bot.session.max_duration_seconds == 1800
    assert config.system_prompt.startswith("You are a warm, patient")
    assert config.greeting.startswith("Hola")


def test_loads_minimal_config(tmp_path: Path) -> None:
    config = load_app_config(ENV, write_config(tmp_path))

    assert config.secrets.tts_api_key.get_secret_value() == "cartesia-key"
    assert config.bot.llm.model == "gpt-4.1"
    assert config.runtime is Runtime.CLOUD


def test_runtime_is_passed_through(tmp_path: Path) -> None:
    config = load_app_config(ENV, write_config(tmp_path), Runtime.LOCAL)

    assert config.runtime is Runtime.LOCAL


DEEPGRAM_FLUX = 'provider = "deepgram"\nvoice = "flux-ernesto-es"\nexpressivity = 0'


@pytest.mark.parametrize(
    ("tts", "key_name", "expected_type"),
    [(CARTESIA, "CARTESIA_API_KEY", CartesiaTts), (DEEPGRAM_FLUX, "DEEPGRAM_API_KEY", DeepgramTts)],
)
def test_tts_provider_selects_its_api_key(
    tmp_path: Path, tts: str, key_name: str, expected_type: type
) -> None:
    config = load_app_config(ENV, write_config(tmp_path, tts))

    assert isinstance(config.bot.tts, expected_type)
    assert config.secrets.tts_api_key.get_secret_value() == ENV[key_name]


@pytest.mark.parametrize(
    "tts",
    [
        'provider = "deepgram"\nvoice = "aura-2-selena-es"',
        'provider = "deepgram"\nvoice = "flux-ernesto-es"\nexpressivity = 3',
        'provider = "elevenlabs"\nmodel = "eleven_flash_v2_5"\nvoice = "voice-id"',
    ],
)
def test_unsupported_tts_settings_are_rejected(tmp_path: Path, tts: str) -> None:
    with pytest.raises(ConfigError, match="tts"):
        load_app_config(ENV, write_config(tmp_path, tts))


def test_missing_secrets_are_listed_by_name_only(tmp_path: Path) -> None:
    env = {"DEEPGRAM_API_KEY": "dg-key"}

    with pytest.raises(ConfigError) as error:
        load_app_config(env, write_config(tmp_path))

    message = str(error.value)
    assert "OPENAI_API_KEY" in message
    assert "CARTESIA_API_KEY" in message
    assert "ALLOWED_CALLERS" in message
    assert "dg-key" not in message


def test_blank_secrets_count_as_missing(tmp_path: Path) -> None:
    with pytest.raises(ConfigError, match="OPENAI_API_KEY"):
        load_app_config({**ENV, "OPENAI_API_KEY": "  "}, write_config(tmp_path))


def test_invalid_allowlist_is_a_config_error(tmp_path: Path) -> None:
    with pytest.raises(ConfigError, match="ALLOWED_CALLERS"):
        load_app_config({**ENV, "ALLOWED_CALLERS": "555"}, write_config(tmp_path))


def test_unknown_config_keys_are_rejected(tmp_path: Path) -> None:
    path = write_config(tmp_path)
    path.write_text(path.read_text(encoding="utf-8") + "\n[extra]\nfoo = 1\n", encoding="utf-8")

    with pytest.raises(ConfigError):
        load_app_config(ENV, path)


def test_unknown_tts_provider_is_rejected(tmp_path: Path) -> None:
    with pytest.raises(ConfigError):
        load_app_config(ENV, write_config(tmp_path, 'provider = "polly"\nvoice = "x"'))


def test_zero_max_duration_is_rejected(tmp_path: Path) -> None:
    path = write_config(tmp_path)
    path.write_text(
        path.read_text(encoding="utf-8").replace(
            "max_duration_seconds = 60", "max_duration_seconds = 0"
        ),
        encoding="utf-8",
    )

    with pytest.raises(ConfigError):
        load_app_config(ENV, path)


def test_mem0_key_is_required_when_memory_is_enabled(tmp_path: Path) -> None:
    env = {k: v for k, v in ENV.items() if k != "MEM0_API_KEY"}

    with pytest.raises(ConfigError, match="MEM0_API_KEY"):
        load_app_config(env, write_config(tmp_path))


def test_mem0_key_is_optional_when_memory_is_disabled(tmp_path: Path) -> None:
    env = {k: v for k, v in ENV.items() if k != "MEM0_API_KEY"}

    config = load_app_config(env, write_config(tmp_path, memory_enabled=False))

    assert config.secrets.mem0_api_key is None
