import pytest
from pipecat.services.cartesia.tts import CartesiaTTSService
from pipecat.services.deepgram.flux.tts import DeepgramFluxTTSService

from language_coach.config import CartesiaTts, DeepgramTts
from language_coach.practice import PracticeLevel
from language_coach.tts import SpeakingRate, build_tts, speed_update

CARTESIA = CartesiaTts(provider="cartesia", model="sonic-3.6", voice="voice-id", language="es-MX")
DEEPGRAM = DeepgramTts(provider="deepgram", voice="flux-ernesto-es", expressivity=0)


@pytest.mark.parametrize(
    ("requested", "expected"), [(0.85, 0.85), (0.1, 0.5), (5.0, 1.3), (1.0, 1.0)]
)
def test_speaking_rate_clamps_to_app_range(requested: float, expected: float) -> None:
    assert SpeakingRate.clamped(requested).value == pytest.approx(expected)


def test_speaking_rate_rejects_non_finite_values() -> None:
    with pytest.raises(ValueError, match="finite"):
        SpeakingRate.clamped(float("nan"))


def test_cartesia_speed_goes_in_generation_config() -> None:
    delta = speed_update(CARTESIA, SpeakingRate.clamped(0.85))

    assert isinstance(delta, CartesiaTTSService.Settings)
    assert getattr(delta.generation_config, "speed", None) == pytest.approx(0.85)


def test_cartesia_speed_is_raised_to_its_provider_minimum() -> None:
    delta = speed_update(CARTESIA, SpeakingRate.for_level(PracticeLevel.BEGINNER))

    assert isinstance(delta, CartesiaTTSService.Settings)
    assert getattr(delta.generation_config, "speed", None) == pytest.approx(0.6)


@pytest.mark.parametrize(("requested", "expected"), [(0.5, 0.5), (0.83, 0.85), (1.12, 1.1)])
def test_deepgram_flux_speed_uses_its_0_05_steps(requested: float, expected: float) -> None:
    delta = speed_update(DEEPGRAM, SpeakingRate.clamped(requested))

    assert isinstance(delta, DeepgramFluxTTSService.Settings)
    assert delta.speed == expected


@pytest.mark.parametrize(
    ("level", "expected"),
    [
        (PracticeLevel.BEGINNER, 0.5),
        (PracticeLevel.INTERMEDIATE, 0.8),
        (PracticeLevel.ADVANCED, 1.0),
    ],
)
def test_each_level_starts_at_its_own_rate(level: PracticeLevel, expected: float) -> None:
    assert SpeakingRate.for_level(level).value == pytest.approx(expected)


def test_build_tts_constructs_cartesia() -> None:
    assert isinstance(build_tts(CARTESIA, "api-key"), CartesiaTTSService)


def test_build_tts_constructs_deepgram_flux_with_its_expressivity() -> None:
    service = build_tts(DEEPGRAM.model_copy(update={"expressivity": -1}), "api-key")

    assert isinstance(service, DeepgramFluxTTSService)
    query = service._build_query_string()  # pyright: ignore[reportPrivateUsage] -- the /v2/speak URL is only exposed here
    assert "model=flux-ernesto-es" in query
    assert "expressivity=-1" in query
