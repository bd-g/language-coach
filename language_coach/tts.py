"""TTS provider construction and speaking-rate control."""

import math
from dataclasses import dataclass
from typing import ClassVar, Self

from pipecat.services.cartesia.tts import CartesiaTTSService, GenerationConfig
from pipecat.services.deepgram.tts import DeepgramTTSService
from pipecat.services.elevenlabs.tts import ElevenLabsTTSService
from pipecat.services.settings import TTSSettings
from pipecat.services.tts_service import TTSService

from language_coach.config import CartesiaTts, DeepgramTts, ElevenLabsTts, TtsConfig


@dataclass(frozen=True, slots=True)
class SpeakingRate:
    """Speech-rate multiplier, bounded to what sounds natural for coaching."""

    MIN: ClassVar[float] = 0.7
    MAX: ClassVar[float] = 1.3

    value: float

    @classmethod
    def clamped(cls, requested: float) -> Self:
        """Clamp a requested multiplier into the coaching range."""
        if not math.isfinite(requested):
            raise ValueError("speaking rate must be finite")
        return cls(min(max(requested, cls.MIN), cls.MAX))

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
            return DeepgramTTSService(
                api_key=api_key, settings=DeepgramTTSService.Settings(voice=config.voice)
            )
        case ElevenLabsTts():
            return ElevenLabsTTSService(
                api_key=api_key,
                settings=ElevenLabsTTSService.Settings(
                    model=config.model, voice=config.voice, language=config.language
                ),
            )


def speed_update(config: TtsConfig, rate: SpeakingRate) -> TTSSettings:
    """Settings delta that applies `rate` to the configured provider.

    Provider ranges: Cartesia 0.6-1.5, Aura-2 0.7-1.5, ElevenLabs 0.7-1.2.
    """
    match config:
        case CartesiaTts():
            return CartesiaTTSService.Settings(
                generation_config=GenerationConfig(speed=rate.within(0.6, 1.5))
            )
        case DeepgramTts():
            return DeepgramTTSService.Settings(speed=rate.within(0.7, 1.5))
        case ElevenLabsTts():
            return ElevenLabsTTSService.Settings(speed=rate.within(0.7, 1.2))
