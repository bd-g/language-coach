"""TTS provider construction and speaking-rate control."""

import math
from dataclasses import dataclass
from typing import ClassVar, Self

from pipecat.services.cartesia.tts import CartesiaTTSService, GenerationConfig
from pipecat.services.deepgram.flux.tts import DeepgramFluxTTSService
from pipecat.services.settings import TTSSettings
from pipecat.services.tts_service import TTSService

from language_coach.config import CartesiaTts, DeepgramTts, TtsConfig
from language_coach.practice import PracticeLanguage, PracticeLevel


@dataclass(frozen=True, slots=True)
class SpeakingRate:
    """Speech-rate multiplier, bounded to what sounds natural for coaching.

    `MIN` is Deepgram Flux's slowest speed; Cartesia's higher floor is applied
    in `speed_update`.
    """

    MIN: ClassVar[float] = 0.5
    MAX: ClassVar[float] = 1.3

    value: float

    @classmethod
    def clamped(cls, requested: float) -> Self:
        """Clamp a requested multiplier into the coaching range."""
        if not math.isfinite(requested):
            raise ValueError("speaking rate must be finite")
        return cls(min(max(requested, cls.MIN), cls.MAX))

    @classmethod
    def for_level(cls, level: PracticeLevel) -> Self:
        """Starting rate for a level: beginners get the slowest rate available."""
        match level:
            case PracticeLevel.BEGINNER:
                return cls(cls.MIN)
            case PracticeLevel.INTERMEDIATE:
                return cls(0.8)
            case PracticeLevel.ADVANCED:
                return cls(1.0)

    def within(self, low: float, high: float) -> float:
        """The rate limited to a provider's supported range."""
        return min(max(self.value, low), high)


def build_tts(config: TtsConfig, api_key: str) -> TTSService:
    """Construct the configured TTS service at its default speed."""
    match config:
        case CartesiaTts():
            return CartesiaTTSService(
                api_key=api_key,
                settings=CartesiaTTSService.Settings(
                    model=config.model, voice=config.voice, language=config.language
                ),
            )
        case DeepgramTts():
            return DeepgramFluxTTSService(
                api_key=api_key,
                settings=DeepgramFluxTTSService.Settings(
                    voice=config.voice, expressivity=config.expressivity
                ),
            )


def practice_language_update(config: TtsConfig, language: PracticeLanguage) -> TTSSettings | None:
    """Settings delta so role-play in the practice language gets its native accent.

    Cartesia speaks one locale per request, so the locale follows the practice
    language (American English or Mexican Spanish). Deepgram Flux voices are
    fixed per language and speak both languages on their own, so they are left alone.
    """
    match config:
        case CartesiaTts():
            locale = {PracticeLanguage.ENGLISH: "en-US", PracticeLanguage.SPANISH: "es-MX"}
            return CartesiaTTSService.Settings(language=locale[language])
        case DeepgramTts():
            return None


def speed_update(config: TtsConfig, rate: SpeakingRate) -> TTSSettings:
    """Settings delta that applies `rate` to the configured provider.

    Provider ranges: Cartesia 0.6-1.5; Deepgram Flux 0.5-1.5 in steps of 0.05,
    applied to the open connection without reconnecting.
    """
    match config:
        case CartesiaTts():
            return CartesiaTTSService.Settings(
                generation_config=GenerationConfig(speed=rate.within(0.6, 1.5))
            )
        case DeepgramTts():
            steps = round(rate.within(0.5, 1.5) / 0.05)
            return DeepgramFluxTTSService.Settings(speed=round(steps * 0.05, 2))
