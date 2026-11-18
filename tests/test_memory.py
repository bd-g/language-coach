import asyncio
from dataclasses import dataclass, field
from datetime import UTC, datetime
from typing import Any

import pytest

from language_coach import memory as memory_module
from language_coach.caller import CallerNumber
from language_coach.memory import (
    ConversationMessage,
    LearnerId,
    Mem0Store,
    MemoryRecord,
    SessionMemory,
    conversation_for_memory,
    parse_memory_records,
)
from language_coach.practice import PracticeLanguage

CALLER = CallerNumber.parse("+15551234567")


def test_learner_id_is_scoped_per_practice_language() -> None:
    english = LearnerId.for_caller(CALLER, PracticeLanguage.ENGLISH)
    spanish = LearnerId.for_caller(CALLER, PracticeLanguage.SPANISH)

    assert english != spanish
    assert english.value == "phone:+15551234567:english"


def test_learner_id_repr_is_redacted() -> None:
    learner = LearnerId.for_caller(CALLER, PracticeLanguage.SPANISH)

    assert "555123" not in repr(learner)
    assert "555123" not in str(learner)


# Shape of Mem0's v3 get_all response: https://docs.mem0.ai/api-reference/memory/get-memories
GET_ALL_RESPONSE: dict[str, Any] = {
    "count": 2,
    "next": None,
    "previous": None,
    "results": [
        {
            "id": "m1",
            "memory": "Struggles with ser versus estar",
            "created_at": "2026-11-10T18:00:00Z",
            "updated_at": "2026-11-12T18:00:00Z",
            "categories": ["mistakes"],
        },
        {"id": "m2", "memory": "Learned the phrase 'me da igual'", "created_at": None},
    ],
}


def test_parses_mem0_records_and_tolerates_extra_fields() -> None:
    records = parse_memory_records(GET_ALL_RESPONSE)

    assert [r.memory for r in records] == [
        "Struggles with ser versus estar",
        "Learned the phrase 'me da igual'",
    ]
    assert records[0].updated_at == datetime(2026, 11, 12, 18, tzinfo=UTC)


def test_malformed_records_are_skipped() -> None:
    response = {"results": [{"id": "x"}, {"memory": ""}, {"memory": "valid"}, "junk"]}

    assert [r.memory for r in parse_memory_records(response)] == ["valid"]


def test_non_paginated_list_response_is_accepted() -> None:
    assert len(parse_memory_records([{"memory": "a"}, {"memory": "b"}])) == 2


def test_conversation_keeps_only_spoken_user_and_assistant_text() -> None:
    messages: list[Any] = [
        {"role": "system", "content": "prompt"},
        {"role": "developer", "content": "hidden"},
        {"role": "assistant", "content": "Hola, ¿qué quieres practicar?"},
        {"role": "user", "content": [{"type": "text", "text": "Quiero practicar inglés."}]},
        {"role": "assistant", "tool_calls": [{"id": "1"}]},
        {"role": "tool", "content": '{"ok": true}'},
        {"role": "user", "content": "   "},
    ]

    assert conversation_for_memory(messages) == [
        ConversationMessage(role="assistant", content="Hola, ¿qué quieres practicar?"),
        ConversationMessage(role="user", content="Quiero practicar inglés."),
    ]


@dataclass
class FakeStore:
    records: list[MemoryRecord] = field(default_factory=list[MemoryRecord])
    saved: list[tuple[LearnerId, PracticeLanguage, list[ConversationMessage]]] = field(
        default_factory=list[tuple[LearnerId, PracticeLanguage, list[ConversationMessage]]]
    )
    fail: bool = False
    delay: float = 0.0
    recalled: list[LearnerId] = field(default_factory=list[LearnerId])

    async def recall(self, learner: LearnerId) -> list[MemoryRecord]:
        self.recalled.append(learner)
        await asyncio.sleep(self.delay)
        if self.fail:
            raise RuntimeError("mem0 down")
        return self.records

    async def remember(
        self,
        learner: LearnerId,
        language: PracticeLanguage,
        conversation: list[ConversationMessage],
    ) -> None:
        await asyncio.sleep(self.delay)
        if self.fail:
            raise RuntimeError("mem0 down")
        self.saved.append((learner, language, conversation))


CONVERSATION = [
    ConversationMessage(role="assistant", content="Let's order food."),
    ConversationMessage(role="user", content="I want a taco please."),
]


def session_memory(store: FakeStore, *, wait: float = 1.0) -> SessionMemory:
    return SessionMemory(store, CALLER, timeout_seconds=1.0, recall_wait_seconds=wait)


async def test_prefetched_memories_are_returned_for_the_chosen_language() -> None:
    store = FakeStore(records=[MemoryRecord(memory="Learned 'me da igual'")])
    memory = session_memory(store)

    memory.prefetch()
    records = await memory.recalled(PracticeLanguage.SPANISH)

    assert [r.memory for r in records] == ["Learned 'me da igual'"]


async def test_prefetch_looks_up_both_languages_once() -> None:
    store = FakeStore()
    memory = session_memory(store)

    memory.prefetch()
    memory.prefetch()
    await memory.recalled(PracticeLanguage.ENGLISH)
    await memory.recalled(PracticeLanguage.SPANISH)

    assert sorted(learner.value for learner in store.recalled) == [
        LearnerId.for_caller(CALLER, PracticeLanguage.ENGLISH).value,
        LearnerId.for_caller(CALLER, PracticeLanguage.SPANISH).value,
    ]


async def test_slow_mem0_never_delays_the_reply_beyond_the_wait_budget() -> None:
    memory = session_memory(FakeStore(delay=1.0), wait=0.05)
    memory.prefetch()
    loop = asyncio.get_running_loop()

    started = loop.time()
    records = await memory.recalled(PracticeLanguage.ENGLISH)

    assert records == []
    assert loop.time() - started < 0.5
    await memory.close()


async def test_recall_failures_return_no_memories() -> None:
    memory = session_memory(FakeStore(fail=True))

    memory.prefetch()

    assert await memory.recalled(PracticeLanguage.ENGLISH) == []


async def test_recalled_without_prefetch_still_works() -> None:
    store = FakeStore(records=[MemoryRecord(memory="a")])

    assert len(await session_memory(store).recalled(PracticeLanguage.ENGLISH)) == 1


async def test_close_cancels_a_pending_prefetch() -> None:
    memory = session_memory(FakeStore(delay=10.0))
    memory.prefetch()

    await memory.close()


async def test_remember_saves_under_the_language_scoped_learner() -> None:
    store = FakeStore()
    memory = session_memory(store)

    await memory.remember(PracticeLanguage.ENGLISH, CONVERSATION)

    [(learner, language, conversation)] = store.saved
    assert learner == LearnerId.for_caller(CALLER, PracticeLanguage.ENGLISH)
    assert language is PracticeLanguage.ENGLISH
    assert conversation == CONVERSATION


async def test_remember_skips_conversations_without_learner_speech() -> None:
    store = FakeStore()
    memory = session_memory(store)

    await memory.remember(PracticeLanguage.ENGLISH, CONVERSATION[:1])

    assert store.saved == []


def test_unreachable_mem0_at_startup_disables_memory_instead_of_failing(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def unreachable(*_args: object, **_kwargs: object) -> None:
        raise ValueError("Error: Invalid API key.")

    monkeypatch.setattr(memory_module, "AsyncMemoryClient", unreachable)

    assert Mem0Store.try_connect("bad-key") is None


async def test_remember_failures_are_contained() -> None:
    memory = session_memory(FakeStore(fail=True))

    await memory.remember(PracticeLanguage.ENGLISH, CONVERSATION)
