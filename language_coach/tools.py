"""LLM tools: speaking-rate control and ending the call.

Tool arguments come from the LLM, so handlers parse them into typed models
before acting and return a safe error result when parsing fails.
"""

from enum import StrEnum

from loguru import logger
from pipecat.adapters.llm.schemas.function_schema import FunctionSchema
from pipecat.frames.frames import (
    EndWorkerFrame,
    FunctionCallResultProperties,
    TTSUpdateSettingsFrame,
)
from pipecat.services.llm_service import FunctionCallParams
from pydantic import BaseModel, ConfigDict, ValidationError

from language_coach.config import TtsConfig
from language_coach.tts import SpeakingRate, speed_update


class EndCallReason(StrEnum):
    """Why the coach ended the call; logged as a category, never free text."""

    LEARNER_DONE = "learner_done"
    GOODBYE = "goodbye"
    SCENARIO_COMPLETE = "scenario_complete"
    OTHER = "other"


class SetSpeakingRateArgs(BaseModel):
    """Arguments for `set_speaking_rate`."""

    model_config = ConfigDict(frozen=True)

    speed: float
    reason: str | None = None


class EndCallArgs(BaseModel):
    """Arguments for `end_call`."""

    model_config = ConfigDict(frozen=True)

    reason: EndCallReason


def coach_tools(tts: TtsConfig) -> list[FunctionSchema]:
    """Tool schemas with handlers bound to the session's TTS provider."""

    async def set_speaking_rate(params: FunctionCallParams) -> None:
        try:
            args = SetSpeakingRateArgs.model_validate(params.arguments)
            rate = SpeakingRate.clamped(args.speed)
        except (ValidationError, ValueError):
            logger.warning("set_speaking_rate called with invalid arguments")
            await params.result_callback({"error": "invalid speed; use a number from 0.7 to 1.3"})
            return

        await params.llm.push_frame(TTSUpdateSettingsFrame(delta=speed_update(tts, rate)))
        logger.info(f"speaking rate set to {rate.value:.2f}")
        await params.result_callback(
            {
                "speed": rate.value,
                "instruction": "Confirma brevemente en español el cambio y continúa.",
            }
        )

    async def end_call(params: FunctionCallParams) -> None:
        try:
            reason = EndCallArgs.model_validate(params.arguments).reason
        except ValidationError:
            reason = EndCallReason.OTHER
        logger.info(f"ending call: {reason}")
        await params.result_callback(
            {"ended": True}, properties=FunctionCallResultProperties(run_llm=False)
        )
        await params.llm.push_frame(EndWorkerFrame(reason=f"end_call:{reason}"))

    return [
        FunctionSchema(
            name="set_speaking_rate",
            description=(
                "Adjust the coach's speaking speed when the learner asks to speak slower, "
                "faster, or normally."
            ),
            properties={
                "speed": {
                    "type": "number",
                    "minimum": SpeakingRate.MIN,
                    "maximum": SpeakingRate.MAX,
                    "description": "Speaking speed multiplier. Use 1.0 for normal speed.",
                },
                "reason": {
                    "type": "string",
                    "description": "Short reason for the adjustment.",
                },
            },
            required=["speed"],
            handler=set_speaking_rate,
        ),
        FunctionSchema(
            name="end_call",
            description=(
                "End the session after a short closing, when the learner is done, asks to "
                "stop, or says goodbye."
            ),
            properties={
                "reason": {
                    "type": "string",
                    "enum": [reason.value for reason in EndCallReason],
                    "description": "Why the session is ending.",
                },
            },
            required=["reason"],
            handler=end_call,
        ),
    ]
