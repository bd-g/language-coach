"""LLM tools: practice language and level, speaking-rate control, and ending the call.

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
from language_coach.memory import (
    MemoryRecord,
    SessionMemory,
    memories_for_prompt,
    remembered_level,
)
from language_coach.practice import PracticeChoice, PracticeLanguage, PracticeLevel
from language_coach.tts import SpeakingRate, practice_language_update, speed_update


class EndCallReason(StrEnum):
    """Why the coach ended the call; logged as a category, never free text."""

    LEARNER_DONE = "learner_done"
    GOODBYE = "goodbye"
    SCENARIO_COMPLETE = "scenario_complete"
    OTHER = "other"


def _practice_language_result(
    language: PracticeLanguage, records: list[MemoryRecord] | None, level: PracticeLevel | None
) -> dict[str, object]:
    """Tool result for a language choice. `records` is `None` for sessions without memory."""
    instruction = (
        f"The learner is practicing {language.value}. Follow the rules for that direction. "
        "Confirm the choice in one short sentence in the learner's native language"
    )
    if level is None:
        next_step = "ask which level they want: beginner, intermediate, or advanced"
    else:
        next_step = (
            f"say in a few words that you will continue at their {level.value} level from "
            "earlier calls, which they can change anytime, then ask what situation they want "
            f"to practice. Follow the rules for the {level.value} level"
        )
    if records is None:
        return {
            "practice_language": language.value,
            "instruction": f"{instruction}, then {next_step}.",
        }

    if records:
        instruction += (
            ". This is a returning learner: in the same breath, mention one specific thing "
            f"you remember from earlier calls, then {next_step}. Use learner_memory as "
            "described in your instructions about memory."
        )
    else:
        instruction += f". This is the learner's first session in this language, then {next_step}."
    result: dict[str, object] = {
        "practice_language": language.value,
        "learner_memory": memories_for_prompt(records),
        "instruction": instruction,
    }
    if level is not None:
        result["level"] = level.value
    return result


class SetSpeakingRateArgs(BaseModel):
    """Arguments for `set_speaking_rate`."""

    model_config = ConfigDict(frozen=True)

    speed: float
    reason: str | None = None


class ChoosePracticeLanguageArgs(BaseModel):
    """Arguments for `choose_practice_language`."""

    model_config = ConfigDict(frozen=True)

    language: PracticeLanguage


class ChooseLevelArgs(BaseModel):
    """Arguments for `choose_level`."""

    model_config = ConfigDict(frozen=True)

    level: PracticeLevel


class EndCallArgs(BaseModel):
    """Arguments for `end_call`."""

    model_config = ConfigDict(frozen=True)

    reason: EndCallReason


def coach_tools(
    tts: TtsConfig, choice: PracticeChoice, memory: SessionMemory | None
) -> list[FunctionSchema]:
    """Tool schemas with handlers bound to the session's TTS, practice choice, and memory.

    `memory` is `None` for sessions without a known learner (browser sessions).
    """

    async def apply_level(params: FunctionCallParams, level: PracticeLevel) -> SpeakingRate:
        previous = choice.level_label()
        choice.level = level
        rate = SpeakingRate.for_level(level)
        await params.llm.push_frame(TTSUpdateSettingsFrame(delta=speed_update(tts, rate)))
        logger.info(f"level: {previous} -> {level}; speaking rate {rate.value:.2f}")
        return rate

    async def choose_practice_language(params: FunctionCallParams) -> None:
        try:
            language = ChoosePracticeLanguageArgs.model_validate(params.arguments).language
        except ValidationError:
            logger.warning("choose_practice_language called with invalid arguments")
            await params.result_callback({"error": "language must be 'english' or 'spanish'"})
            return

        previous = choice.label()
        choice.language = language
        delta = practice_language_update(tts, language)
        if delta is not None:
            await params.llm.push_frame(TTSUpdateSettingsFrame(delta=delta))
        logger.info(f"practice language: {previous} -> {language}")
        records = await memory.recalled(language) if memory is not None else None
        level = remembered_level(records) if records else None
        if level is not None:
            await apply_level(params, level)
        await params.result_callback(_practice_language_result(language, records, level))

    async def choose_level(params: FunctionCallParams) -> None:
        try:
            level = ChooseLevelArgs.model_validate(params.arguments).level
        except ValidationError:
            logger.warning("choose_level called with invalid arguments")
            await params.result_callback(
                {"error": "level must be 'beginner', 'intermediate', or 'advanced'"}
            )
            return

        rate = await apply_level(params, level)
        await params.result_callback(
            {
                "level": level.value,
                "speed": rate.value,
                "instruction": (
                    f"The learner chose the {level.value} level. Follow the rules for that "
                    "level from now on. Confirm in one short sentence in the learner's native "
                    "language, then continue."
                ),
            }
        )

    async def set_speaking_rate(params: FunctionCallParams) -> None:
        try:
            args = SetSpeakingRateArgs.model_validate(params.arguments)
            rate = SpeakingRate.clamped(args.speed)
        except (ValidationError, ValueError):
            logger.warning("set_speaking_rate called with invalid arguments")
            limits = f"{SpeakingRate.MIN} to {SpeakingRate.MAX}"
            await params.result_callback({"error": f"invalid speed; use a number from {limits}"})
            return

        await params.llm.push_frame(TTSUpdateSettingsFrame(delta=speed_update(tts, rate)))
        logger.info(f"speaking rate set to {rate.value:.2f}")
        await params.result_callback(
            {
                "speed": rate.value,
                "instruction": (
                    "Briefly confirm the change in the learner's native language and continue."
                ),
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
            name="choose_practice_language",
            description=(
                "Record which language the learner wants to practice, as soon as they say so "
                "at the start of the call or when they ask to switch."
            ),
            properties={
                "language": {
                    "type": "string",
                    "enum": [language.value for language in PracticeLanguage],
                    "description": "The language the learner wants to practice speaking.",
                },
            },
            required=["language"],
            handler=choose_practice_language,
        ),
        FunctionSchema(
            name="choose_level",
            description=(
                "Record the learner's level after they choose it, and when they ask to change "
                "it. This also resets the speaking speed to the level's default."
            ),
            properties={
                "level": {
                    "type": "string",
                    "enum": [level.value for level in PracticeLevel],
                    "description": "The level the learner wants to practice at.",
                },
            },
            required=["level"],
            handler=choose_level,
        ),
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
                    "description": (
                        "Speaking speed multiplier. 1.0 is a native speaker's normal speed."
                    ),
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
