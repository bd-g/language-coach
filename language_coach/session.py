"""Session lifecycle: reject unknown callers or run the coaching pipeline."""

import asyncio
import time
from dataclasses import dataclass

from loguru import logger
from pipecat.audio.filters.base_audio_filter import BaseAudioFilter
from pipecat.frames.frames import EndFrame, ErrorFrame, TTSSpeakFrame
from pipecat.pipeline.pipeline import Pipeline
from pipecat.pipeline.worker import PipelineParams, PipelineWorker
from pipecat.processors.aggregators.llm_context import LLMContext
from pipecat.processors.aggregators.llm_response_universal import LLMContextAggregatorPair
from pipecat.runner.types import RunnerArguments
from pipecat.services.deepgram.flux.stt import DeepgramFluxSTTService
from pipecat.services.openai.responses.llm import OpenAIResponsesLLMService
from pipecat.transports.base_transport import BaseTransport, TransportParams
from pipecat.transports.daily.transport import DailyDialinSettings, DailyParams, DailyTransport
from pipecat.transports.smallwebrtc.transport import SmallWebRTCTransport
from pipecat.workers.runner import WorkerRunner

from language_coach.admission import rejection_reason
from language_coach.config import AppConfig, Runtime, SessionConfig
from language_coach.failures import Provider, ProviderFailureMonitor
from language_coach.memory import LearnerMemoryStore, SessionMemory, conversation_for_memory
from language_coach.origin import (
    DailyRoomSession,
    PhoneCall,
    SessionOrigin,
    WebRtcSession,
    parse_origin,
)
from language_coach.practice import PracticeChoice
from language_coach.tools import coach_tools
from language_coach.tts import build_tts
from language_coach.usage import UsageObserver

BOT_NAME = "Language Coach"
PHONE_SAMPLE_RATE = 8000
REJECT_TIMEOUT_SECONDS = 15.0
PROVIDER_MAX_ERRORS = 3
PROVIDER_ERROR_WINDOW_SECONDS = 30.0


@dataclass(frozen=True, slots=True)
class _Audio:
    in_sample_rate: int | None
    out_sample_rate: int | None

    def pipeline_params(self) -> PipelineParams:
        params = PipelineParams(enable_metrics=True, enable_usage_metrics=True)
        if self.in_sample_rate is not None:
            params.audio_in_sample_rate = self.in_sample_rate
        if self.out_sample_rate is not None:
            params.audio_out_sample_rate = self.out_sample_rate
        return params


_PHONE_AUDIO = _Audio(PHONE_SAMPLE_RATE, PHONE_SAMPLE_RATE)
_BROWSER_AUDIO = _Audio(None, None)


async def run_session(
    args: RunnerArguments, config: AppConfig, memory_store: LearnerMemoryStore | None
) -> None:
    """Entry point for one session started by the Pipecat runner."""
    origin = parse_origin(args)
    reason = rejection_reason(origin, config.secrets.allowlist)
    if isinstance(origin, PhoneCall) and reason is not None:
        logger.info(f"rejecting caller {origin.caller}: {reason}")
        await _reject(origin, args.handle_sigint)
        return
    caller = origin.caller if isinstance(origin, PhoneCall) else "n/a"
    logger.info(f"starting {type(origin).__name__} session; caller {caller}")
    memory = (
        SessionMemory(
            memory_store,
            origin.caller,
            timeout_seconds=config.bot.memory.timeout_seconds,
            recall_wait_seconds=config.bot.memory.recall_wait_seconds,
        )
        if memory_store is not None and isinstance(origin, PhoneCall) and origin.caller
        else None
    )
    await _coach(origin, args.handle_sigint, config, memory)


async def _reject(call: PhoneCall, handle_sigint: bool) -> None:
    """Connect the held call and leave at once, ending it without creating AI services.

    Never joining would leave the caller on Daily's hold music for up to about
    two minutes, which may be billed as PSTN time and keeps the caller waiting.
    """
    transport = _phone_transport(call, noise_filter=None)
    worker = PipelineWorker(
        Pipeline([transport.input(), transport.output()]),
        params=_PHONE_AUDIO.pipeline_params(),
        enable_rtvi=False,
        idle_timeout_secs=REJECT_TIMEOUT_SECONDS,
    )
    runner = WorkerRunner(handle_sigint=handle_sigint)
    await runner.add_workers(worker)

    @transport.event_handler("on_dialin_connected")
    async def _hang_up(_transport: BaseTransport, _data: object) -> None:
        await runner.cancel(reason="caller rejected")

    @transport.event_handler("on_dialin_error")
    async def _dialin_error(_transport: BaseTransport, _data: object) -> None:
        await runner.cancel(reason="dial-in error")

    await runner.run()


async def _coach(
    origin: SessionOrigin,
    handle_sigint: bool,
    config: AppConfig,
    memory: SessionMemory | None,
) -> None:
    if memory is not None:
        # Look up memories while the call connects and the greeting plays.
        memory.prefetch()
    transport, audio = _transport(origin, config.runtime)
    secrets = config.secrets
    bot = config.bot

    stt = DeepgramFluxSTTService(
        api_key=secrets.deepgram_api_key.get_secret_value(),
        settings=DeepgramFluxSTTService.Settings(
            model=bot.stt.model, language_hints=bot.stt.language_hints
        ),
    )
    llm = OpenAIResponsesLLMService(
        api_key=secrets.openai_api_key.get_secret_value(),
        settings=OpenAIResponsesLLMService.Settings(
            model=bot.llm.model, system_instruction=config.system_prompt
        ),
    )
    tts = build_tts(bot.tts, secrets.tts_api_key.get_secret_value())

    practice = PracticeChoice()
    context = LLMContext(tools=list(coach_tools(bot.tts, practice, memory)))
    user_aggregator, assistant_aggregator = LLMContextAggregatorPair(context)

    usage = UsageObserver()
    worker = PipelineWorker(
        Pipeline(
            [
                transport.input(),
                stt,
                user_aggregator,
                llm,
                tts,
                transport.output(),
                assistant_aggregator,
            ]
        ),
        params=audio.pipeline_params(),
        observers=[usage],
    )
    runner = WorkerRunner(handle_sigint=handle_sigint)
    await runner.add_workers(worker)

    started: list[asyncio.Task[None]] = []

    @transport.event_handler("on_client_connected")
    async def _greet(_transport: BaseTransport, _client: object) -> None:
        if started:
            return
        logger.info("client connected; greeting")
        await worker.queue_frames([TTSSpeakFrame(config.greeting)])
        started.append(asyncio.create_task(_enforce_time_limit(worker, bot.session)))

    @transport.event_handler("on_client_disconnected")
    async def _disconnected(_transport: BaseTransport, _client: object) -> None:
        logger.info("client disconnected")
        await runner.cancel(reason="client disconnected")

    end_on_provider_failure(
        worker,
        ProviderFailureMonitor(
            {stt: Provider.STT, llm: Provider.LLM, tts: Provider.TTS},
            max_errors=PROVIDER_MAX_ERRORS,
            window_seconds=PROVIDER_ERROR_WINDOW_SECONDS,
        ),
        bot.session,
    )

    if isinstance(transport, DailyTransport):

        @transport.event_handler("on_dialin_error")
        async def _dialin_error(_transport: BaseTransport, _data: object) -> None:
            logger.error("dial-in error")
            await runner.cancel(reason="dial-in error")

    started_at = time.monotonic()
    try:
        await runner.run()
    finally:
        for task in started:
            task.cancel()
        # Runs after hang-ups too: the runner returns once the client disconnects, and
        # Pipecat Cloud keeps the session alive until bot() returns.
        if memory is not None:
            await memory.close()
            if practice.language is not None:
                await memory.remember(
                    practice.language, conversation_for_memory(context.get_messages())
                )
                if practice.level is not None:
                    await memory.remember_level(practice.language, practice.level)
        summary = usage.summary()
        logger.info(
            f"session ended after {time.monotonic() - started_at:.0f}s; "
            f"practice_language={practice.label()} level={practice.level_label()} "
            f"tts={bot.tts.provider} tts_characters={summary.tts_characters} "
            f"llm_prompt_tokens={summary.llm_prompt_tokens} "
            f"llm_completion_tokens={summary.llm_completion_tokens}"
        )


def end_on_provider_failure(
    worker: PipelineWorker, failures: ProviderFailureMonitor, session: SessionConfig
) -> None:
    """End the session gracefully when `failures` says a provider has failed.

    The learner hears `failure_message` first unless the TTS provider is the one that failed.
    """

    @worker.event_handler("on_pipeline_error")
    async def _provider_error(_worker: PipelineWorker, frame: ErrorFrame) -> None:
        processor = frame.processor
        usable = processor.is_usable if processor is not None else True
        provider = failures.record(processor, usable=usable)
        if provider is None:
            return
        logger.error(f"ending session: {provider} provider failed")
        reason = f"{provider} failed"
        match provider:
            case Provider.TTS:
                await worker.queue_frames([EndFrame(reason=reason)])
            case Provider.STT | Provider.LLM:
                await worker.queue_frames(
                    [TTSSpeakFrame(session.failure_message), EndFrame(reason=reason)]
                )


async def _enforce_time_limit(worker: PipelineWorker, session: SessionConfig) -> None:
    await asyncio.sleep(session.max_duration_seconds)
    logger.info("session reached its time limit")
    await worker.queue_frames(
        [TTSSpeakFrame(session.time_limit_message), EndFrame(reason="time limit")]
    )


def _transport(origin: SessionOrigin, runtime: Runtime) -> tuple[BaseTransport, _Audio]:
    noise_filter = _noise_filter(runtime)
    match origin:
        case PhoneCall():
            return _phone_transport(origin, noise_filter), _PHONE_AUDIO
        case DailyRoomSession(room=room):
            params = DailyParams(
                audio_in_enabled=True, audio_in_filter=noise_filter, audio_out_enabled=True
            )
            return DailyTransport(room.url, room.token, BOT_NAME, params=params), _BROWSER_AUDIO
        case WebRtcSession(connection=connection):
            params = TransportParams(
                audio_in_enabled=True, audio_in_filter=noise_filter, audio_out_enabled=True
            )
            transport = SmallWebRTCTransport(webrtc_connection=connection, params=params)
            return transport, _BROWSER_AUDIO


def _noise_filter(runtime: Runtime) -> BaseAudioFilter | None:
    """Krisp VIVA on Pipecat Cloud (enabled by `[krisp_viva]` in pcc-deploy.toml).

    The Krisp SDK only exists in the Pipecat Cloud runtime, so it is imported lazily.
    See https://docs.pipecat.ai/pipecat-cloud/guides/krisp-viva
    """
    match runtime:
        case Runtime.LOCAL:
            return None
        case Runtime.CLOUD:
            from pipecat.audio.filters.krisp_viva_filter import KrispVivaFilter

            return KrispVivaFilter()


def _phone_transport(call: PhoneCall, noise_filter: BaseAudioFilter | None) -> DailyTransport:
    settings = call.request.dialin_settings
    params = DailyParams(
        api_key=call.request.daily_api_key,
        api_url=call.request.daily_api_url,
        dialin_settings=DailyDialinSettings(
            call_id=settings.call_id, call_domain=settings.call_domain
        ),
        audio_in_enabled=True,
        audio_in_filter=noise_filter,
        audio_out_enabled=True,
    )
    return DailyTransport(call.room.url, call.room.token, BOT_NAME, params=params)
