"""Decide when a provider failure should end the session."""

import time
from collections import deque
from collections.abc import Callable, Mapping
from enum import StrEnum


class Provider(StrEnum):
    """AI services a coaching session depends on."""

    STT = "stt"
    LLM = "llm"
    TTS = "tts"


class ProviderFailureMonitor:
    """Ends a session once a provider is unusable or keeps failing.

    Pipecat marks a service unusable for permanent errors (rejected credentials,
    invalid requests). Some provider errors arrive uncategorized and leave the
    service "usable" while every request fails, such as Cartesia's
    `voice_not_found`, so repeated errors within a window also end the session.
    """

    def __init__(
        self,
        providers: Mapping[object, Provider],
        *,
        max_errors: int,
        window_seconds: float,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self._providers = providers
        self._max_errors = max_errors
        self._window_seconds = window_seconds
        self._clock = clock
        self._errors: dict[Provider, deque[float]] = {p: deque() for p in Provider}
        self._ended = False

    def record(self, processor: object | None, *, usable: bool) -> Provider | None:
        """Record an error; returns the failed provider the first time the session should end."""
        provider = self._providers.get(processor) if processor is not None else None
        if provider is None or self._ended:
            return None

        now = self._clock()
        errors = self._errors[provider]
        errors.append(now)
        while errors and now - errors[0] > self._window_seconds:
            errors.popleft()

        if usable and len(errors) < self._max_errors:
            return None
        self._ended = True
        return provider
