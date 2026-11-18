import pytest
from pipecat.services.cartesia.tts import CartesiaTTSService
from pipecat.services.deepgram.tts import DeepgramTTSService
from pipecat.services.elevenlabs.tts import ElevenLabsTTSService

from language_coach.config import CartesiaTts, DeepgramTts, ElevenLabsTts
from language_coach.tts import SpeakingRate, build_tts, speed_update

CARTESIA = CartesiaTts(provider="cartesia", model="sonic-3.6", voice="voice-id", language="es-MX")
DEEPGRAM = DeepgramTts(provider="deepgram", voice="aura-2-selena-es")
ELEVENLABS = ElevenLabsTts(provider="elevenlabs", model="eleven_flash_v2_5", voice="voice-id")


@pytest.mark.parametrize(
    ("requested", "expected"), [(0.85, 0.85), (0.1, 0.7), (5.0, 1.3), (1.0, 1.0)]
)
def test_speaking_rate_clamps_to_app_range(requested: float, expected: float) -> None:
    assert SpeakingRate.clamped(requested).value == pytest.approx(expected)


def test_speaking_rate_rejects_non_finite_values() -> None:
    with pytest.raises(ValueError, match="finite"):
        SpeakingRate.clamped(float("nan"))


def test_cartesia_speed_goes_in_generation_config() -> None:
    delta = speed_update(CARTESIA, SpeakingRate.clamped(0.85))

    assert isinstance(delta, CartesiaTTSService.Settings)
    assert isinstance(delta.generation_config, object)
    assert getattr(delta.generation_config, "speed", None) == pytest.approx(0.85)


def test_deepgram_speed_is_a_direct_setting() -> None:
    delta = speed_update(DEEPGRAM, SpeakingRate.clamped(1.15))

    assert isinstance(delta, DeepgramTTSService.Settings)
    assert delta.speed == pytest.approx(1.15)


def test_elevenlabs_speed_is_capped_at_its_provider_maximum() -> None:
    delta = speed_update(ELEVENLABS, SpeakingRate.clamped(1.3))

    assert isinstance(delta, ElevenLabsTTSService.Settings)
    assert delta.speed == pytest.approx(1.2)


@pytest.mark.parametrize(
    ("config", "service_type"),
    [
        (CARTESIA, CartesiaTTSService),
        (DEEPGRAM, DeepgramTTSService),
        (ELEVENLABS, ElevenLabsTTSService),
    ],
)
def test_build_tts_constructs_the_configured_provider(
    config: CartesiaTts | DeepgramTts | ElevenLabsTts, service_type: type
) -> None:
    assert isinstance(build_tts(config, "api-key"), service_type)
