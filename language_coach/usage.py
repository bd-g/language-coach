"""Per-session provider usage, logged at session end to compare provider costs."""

from dataclasses import dataclass

from pipecat.frames.frames import MetricsFrame
from pipecat.metrics.metrics import LLMUsageMetricsData, TTSUsageMetricsData
from pipecat.observers.base_observer import BaseObserver, FramePushed


@dataclass(frozen=True, slots=True)
class UsageSummary:
    """Billable usage for one session."""

    tts_characters: int
    llm_prompt_tokens: int
    llm_completion_tokens: int


class UsageObserver(BaseObserver):
    """Totals Pipecat's usage metrics (requires `enable_usage_metrics`)."""

    def __init__(self) -> None:
        # BaseObserver.__init__ declares untyped **kwargs.
        super().__init__()  # pyright: ignore[reportUnknownMemberType]
        self._tts_characters = 0
        self._llm_prompt_tokens = 0
        self._llm_completion_tokens = 0

    async def on_push_frame(self, data: FramePushed) -> None:
        if not data.first_push or not isinstance(data.frame, MetricsFrame):
            return
        for metric in data.frame.data:
            match metric:
                case TTSUsageMetricsData():
                    self._tts_characters += metric.value
                case LLMUsageMetricsData():
                    self._llm_prompt_tokens += metric.value.prompt_tokens
                    self._llm_completion_tokens += metric.value.completion_tokens
                case _:
                    pass

    def summary(self) -> UsageSummary:
        """Usage so far."""
        return UsageSummary(
            tts_characters=self._tts_characters,
            llm_prompt_tokens=self._llm_prompt_tokens,
            llm_completion_tokens=self._llm_completion_tokens,
        )
