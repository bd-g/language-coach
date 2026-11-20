from dataclasses import dataclass, field
from typing import Any, cast

import pytest
from pipecat.frames.frames import Frame, TTSUpdateSettingsFrame
from pipecat.services.cartesia.tts import CartesiaTTSService
from pipecat.services.elevenlabs.tts import ElevenLabsTTSService
from pipecat.services.llm_service import FunctionCallParams

from language_coach.caller import CallerNumber
from language_coach.config import CartesiaTts, DeepgramTts, ElevenLabsTts
from language_coach.memory import LearnerId, MemoryRecord, RecordMetadata, SessionMemory
from language_coach.practice import PracticeChoice, PracticeLanguage, PracticeLevel
from language_coach.tools import coach_tools
from language_coach.tts import practice_language_update

CARTESIA = CartesiaTts(provider="cartesia", model="sonic-3.6", voice="v", language="es-MX")
DEEPGRAM = DeepgramTts(provider="deepgram", voice="aura-2-selena-es")
ELEVENLABS = ElevenLabsTts(provider="elevenlabs", model="eleven_flash_v2_5", voice="v")


@pytest.mark.parametrize(
    ("language", "locale"),
    [(PracticeLanguage.ENGLISH, "en-US"), (PracticeLanguage.SPANISH, "es-MX")],
)
def test_cartesia_speaks_with_the_practice_language_locale(
    language: PracticeLanguage, locale: str
) -> None:
    delta = practice_language_update(CARTESIA, language)

    assert isinstance(delta, CartesiaTTSService.Settings)
    assert delta.language == locale


def test_elevenlabs_uses_base_language_codes() -> None:
    delta = practice_language_update(ELEVENLABS, PracticeLanguage.SPANISH)

    assert isinstance(delta, ElevenLabsTTSService.Settings)
    assert delta.language == "es"


def test_deepgram_voice_is_left_unchanged() -> None:
    assert practice_language_update(DEEPGRAM, PracticeLanguage.ENGLISH) is None


@dataclass
class FakeLlm:
    pushed: list[Frame] = field(default_factory=list[Frame])

    async def push_frame(self, frame: Frame) -> None:
        self.pushed.append(frame)


@dataclass
class FakeParams:
    arguments: dict[str, Any]
    llm: FakeLlm = field(default_factory=FakeLlm)
    results: list[Any] = field(default_factory=list[Any])

    async def result_callback(self, result: Any, *, properties: Any = None) -> None:
        self.results.append(result)


async def call_choose(
    arguments: dict[str, Any], choice: PracticeChoice, memory: SessionMemory | None = None
) -> FakeParams:
    tools = {tool.name: tool for tool in coach_tools(CARTESIA, choice, memory)}
    handler = tools["choose_practice_language"].handler
    assert handler is not None
    params = FakeParams(arguments)
    await handler(cast(FunctionCallParams, params))
    return params


async def test_choosing_a_language_records_it_and_switches_the_voice_locale() -> None:
    choice = PracticeChoice()

    params = await call_choose({"language": "spanish"}, choice)

    assert choice.language is PracticeLanguage.SPANISH
    [frame] = params.llm.pushed
    assert isinstance(frame, TTSUpdateSettingsFrame)
    assert params.results[0]["practice_language"] == "spanish"


async def test_learner_can_switch_languages_later() -> None:
    choice = PracticeChoice()

    await call_choose({"language": "spanish"}, choice)
    await call_choose({"language": "english"}, choice)

    assert choice.language is PracticeLanguage.ENGLISH


async def test_invalid_language_is_rejected_without_changing_state() -> None:
    choice = PracticeChoice()

    params = await call_choose({"language": "french"}, choice)

    assert choice.language is None
    assert params.llm.pushed == []
    assert "error" in params.results[0]


def test_tool_schema_lists_both_languages() -> None:
    tools = {tool.name: tool for tool in coach_tools(DEEPGRAM, PracticeChoice(), None)}

    language = tools["choose_practice_language"].properties["language"]
    assert language["enum"] == ["english", "spanish"]


@dataclass
class RecallStore:
    records: list[MemoryRecord]
    recalled: list[LearnerId] = field(default_factory=list[LearnerId])

    async def recall(self, learner: LearnerId) -> list[MemoryRecord]:
        self.recalled.append(learner)
        return self.records

    async def remember(self, learner: LearnerId, language: PracticeLanguage, conversation: Any):
        raise AssertionError("not expected during the call")

    async def remember_level(self, learner: LearnerId, level: PracticeLevel) -> None:
        raise AssertionError("not expected during the call")


CALLER = CallerNumber.parse("+15551234567")


async def test_returning_learner_gets_memories_for_the_chosen_language() -> None:
    store = RecallStore([MemoryRecord(memory="Confuses ser and estar")])
    memory = SessionMemory(store, CALLER, timeout_seconds=1.0, recall_wait_seconds=1.0)

    params = await call_choose({"language": "spanish"}, PracticeChoice(), memory)

    assert LearnerId.for_caller(CALLER, PracticeLanguage.SPANISH) in store.recalled
    assert params.results[0]["learner_memory"] == ["Confuses ser and estar"]
    assert "returning learner" in params.results[0]["instruction"]


async def test_new_learner_is_told_this_is_a_first_session() -> None:
    memory = SessionMemory(RecallStore([]), CALLER, timeout_seconds=1.0, recall_wait_seconds=1.0)

    params = await call_choose({"language": "english"}, PracticeChoice(), memory)

    assert params.results[0]["learner_memory"] == []
    assert "first session" in params.results[0]["instruction"]


async def test_sessions_without_memory_do_not_mention_it() -> None:
    params = await call_choose({"language": "english"}, PracticeChoice())

    assert "learner_memory" not in params.results[0]


async def call_level(arguments: dict[str, Any], choice: PracticeChoice) -> FakeParams:
    tools = {tool.name: tool for tool in coach_tools(CARTESIA, choice, None)}
    handler = tools["choose_level"].handler
    assert handler is not None
    params = FakeParams(arguments)
    await handler(cast(FunctionCallParams, params))
    return params


@pytest.mark.parametrize(
    ("level", "speed"),
    [
        (PracticeLevel.BEGINNER, 0.6),
        (PracticeLevel.INTERMEDIATE, 0.8),
        (PracticeLevel.ADVANCED, 1.0),
    ],
)
async def test_choosing_a_level_records_it_and_sets_its_speed(
    level: PracticeLevel, speed: float
) -> None:
    choice = PracticeChoice()

    params = await call_level({"level": level.value}, choice)

    assert choice.level is level
    [frame] = params.llm.pushed
    assert isinstance(frame, TTSUpdateSettingsFrame)
    assert isinstance(frame.delta, CartesiaTTSService.Settings)
    assert getattr(frame.delta.generation_config, "speed", None) == pytest.approx(speed)
    assert params.results[0]["level"] == level.value


async def test_invalid_level_is_rejected_without_changing_state() -> None:
    choice = PracticeChoice()

    params = await call_level({"level": "expert"}, choice)

    assert choice.level is None
    assert params.llm.pushed == []
    assert "error" in params.results[0]


def test_level_schema_lists_all_levels() -> None:
    tools = {tool.name: tool for tool in coach_tools(DEEPGRAM, PracticeChoice(), None)}

    level = tools["choose_level"].properties["level"]
    assert level["enum"] == ["beginner", "intermediate", "advanced"]


async def test_choosing_a_language_asks_for_a_level_next() -> None:
    params = await call_choose({"language": "spanish"}, PracticeChoice())

    assert "level" in params.results[0]["instruction"]


LEVEL_RECORD = MemoryRecord(
    memory="Practice level: beginner",
    metadata=RecordMetadata(kind="practice_level", level=PracticeLevel.BEGINNER),
)


async def test_returning_learner_with_a_recorded_level_skips_the_level_question() -> None:
    store = RecallStore([MemoryRecord(memory="Ordered tacos"), LEVEL_RECORD])
    memory = SessionMemory(store, CALLER, timeout_seconds=1.0, recall_wait_seconds=1.0)
    choice = PracticeChoice()

    params = await call_choose({"language": "spanish"}, choice, memory)

    assert choice.level is PracticeLevel.BEGINNER
    speeds = [
        getattr(frame.delta.generation_config, "speed", None)
        for frame in params.llm.pushed
        if isinstance(frame, TTSUpdateSettingsFrame)
        and isinstance(frame.delta, CartesiaTTSService.Settings)
    ]
    assert speeds[-1] == pytest.approx(0.6)
    result = params.results[0]
    assert result["level"] == "beginner"
    assert "ask which level" not in result["instruction"]
    assert result["learner_memory"] == ["Ordered tacos"]


async def test_returning_learner_without_a_recorded_level_is_asked_once() -> None:
    store = RecallStore([MemoryRecord(memory="The learner seems intermediate")])
    memory = SessionMemory(store, CALLER, timeout_seconds=1.0, recall_wait_seconds=1.0)
    choice = PracticeChoice()

    params = await call_choose({"language": "spanish"}, choice, memory)

    assert choice.level is None
    assert "ask which level" in params.results[0]["instruction"]
