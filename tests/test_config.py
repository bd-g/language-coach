from pathlib import Path

import pytest

from language_coach.config import (
    DEFAULT_CONFIG_PATH,
    CartesiaTts,
    ConfigError,
    DeepgramTts,
    ElevenLabsTts,
    Runtime,
    load_app_config,
)

ENV = {
    "DEEPGRAM_API_KEY": "dg-key",
    "OPENAI_API_KEY": "openai-key",
    "CARTESIA_API_KEY": "cartesia-key",
    "ALLOWED_CALLERS": "+15551234567",
}

MINIMAL_TOML = """
[session]
max_duration_seconds = 60
time_limit_message = "Se acabó el tiempo."

[stt]
model = "flux-general-multi"
language_hints = ["en", "es"]

[llm]
model = "gpt-4.1"

[tts]
{tts}
"""

CARTESIA = 'provider = "cartesia"\nmodel = "sonic-3.6"\nvoice = "voice-id"\nlanguage = "es-MX"'


def write_config(tmp_path: Path, tts: str = CARTESIA) -> Path:
    path = tmp_path / "bot.toml"
    path.write_text(MINIMAL_TOML.format(tts=tts), encoding="utf-8")
    return path


def test_committed_config_loads() -> None:
    config = load_app_config(ENV, DEFAULT_CONFIG_PATH)

    assert isinstance(config.bot.tts, CartesiaTts)
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


@pytest.mark.parametrize(
    ("tts", "key_name", "expected_type"),
    [
        ('provider = "deepgram"\nvoice = "aura-2-selena-es"', "DEEPGRAM_API_KEY", DeepgramTts),
        (
            'provider = "elevenlabs"\nmodel = "eleven_flash_v2_5"\nvoice = "voice-id"',
            "ELEVENLABS_API_KEY",
            ElevenLabsTts,
        ),
    ],
)
def test_tts_provider_selects_its_api_key(
    tmp_path: Path, tts: str, key_name: str, expected_type: type
) -> None:
    env = {**ENV, "ELEVENLABS_API_KEY": "eleven-key"}

    config = load_app_config(env, write_config(tmp_path, tts))

    assert isinstance(config.bot.tts, expected_type)
    assert config.secrets.tts_api_key.get_secret_value() == env[key_name]


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
