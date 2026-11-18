from pipecat.frames.frames import MetricsFrame
from pipecat.metrics.metrics import LLMTokenUsage, LLMUsageMetricsData, TTSUsageMetricsData
from pipecat.processors.filters.identity_filter import IdentityFilter
from pipecat.tests.utils import run_test

from language_coach.usage import UsageObserver


async def test_usage_observer_totals_tts_characters_and_llm_tokens() -> None:
    observer = UsageObserver()
    frames = [
        MetricsFrame(data=[TTSUsageMetricsData(processor="tts", value=120)]),
        MetricsFrame(
            data=[
                TTSUsageMetricsData(processor="tts", value=30),
                LLMUsageMetricsData(
                    processor="llm",
                    value=LLMTokenUsage(prompt_tokens=200, completion_tokens=40, total_tokens=240),
                ),
            ]
        ),
    ]

    await run_test(IdentityFilter(), frames_to_send=frames, observers=[observer])

    summary = observer.summary()
    assert summary.tts_characters == 150
    assert summary.llm_prompt_tokens == 200
    assert summary.llm_completion_tokens == 40
