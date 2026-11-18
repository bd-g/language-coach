import asyncio

from pipecat.frames.frames import EndFrame, Frame, StartFrame, TextFrame, TTSSpeakFrame
from pipecat.pipeline.pipeline import Pipeline
from pipecat.pipeline.worker import PipelineWorker
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor
from pipecat.workers.runner import WorkerRunner

from language_coach.config import SessionConfig
from language_coach.failures import Provider, ProviderFailureMonitor
from language_coach.session import end_on_provider_failure

SESSION = SessionConfig(
    max_duration_seconds=60, time_limit_message="tiempo", failure_message="algo salió mal"
)


class FailingService(FrameProcessor):
    """Reports an error for every text frame, like a provider whose requests all fail."""

    def __init__(self, *, permanent: bool) -> None:
        # FrameProcessor.__init__ declares untyped **kwargs.
        super().__init__()  # pyright: ignore[reportUnknownMemberType]
        self._permanent = permanent

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        if isinstance(frame, TextFrame) and not isinstance(frame, TTSSpeakFrame):
            await self.push_error(
                error_msg="provider failed", force_treat_as_permanent=self._permanent
            )
            return
        await self.push_frame(frame, direction)


class Recorder(FrameProcessor):
    def __init__(self) -> None:
        # FrameProcessor.__init__ declares untyped **kwargs.
        super().__init__()  # pyright: ignore[reportUnknownMemberType]
        self.frames: list[Frame] = []

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        self.frames.append(frame)
        await self.push_frame(frame, direction)


async def run_session(
    service: FailingService, provider: Provider, texts: int, timeout: float = 5.0
) -> Recorder:
    recorder = Recorder()
    worker = PipelineWorker(Pipeline([service, recorder]), enable_rtvi=False)
    monitor = ProviderFailureMonitor({service: provider}, max_errors=3, window_seconds=30.0)
    end_on_provider_failure(worker, monitor, SESSION)

    @worker.event_handler("on_pipeline_started")
    async def _send(_worker: PipelineWorker, _frame: StartFrame) -> None:
        await worker.queue_frames([TextFrame(f"turn {i}") for i in range(texts)])

    runner = WorkerRunner(handle_sigint=False)
    await runner.add_workers(worker)
    await asyncio.wait_for(runner.run(), timeout=timeout)
    return recorder


async def test_unusable_llm_speaks_failure_message_then_ends() -> None:
    recorder = await run_session(FailingService(permanent=True), Provider.LLM, texts=1)

    spoken = [f.text for f in recorder.frames if isinstance(f, TTSSpeakFrame)]
    assert spoken == [SESSION.failure_message]
    assert isinstance(recorder.frames[-1], EndFrame)


async def test_repeatedly_failing_tts_ends_without_speaking() -> None:
    recorder = await run_session(FailingService(permanent=False), Provider.TTS, texts=3)

    assert not any(isinstance(f, TTSSpeakFrame) for f in recorder.frames)
    assert isinstance(recorder.frames[-1], EndFrame)


async def test_occasional_errors_do_not_end_the_session() -> None:
    # The runner absorbs the timeout's cancellation, so check what reached the sink instead.
    recorder = await run_session(
        FailingService(permanent=False), Provider.STT, texts=2, timeout=1.0
    )

    assert not any(isinstance(f, TTSSpeakFrame | EndFrame) for f in recorder.frames)
