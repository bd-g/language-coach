"""Per-learner long-term memory, stored in Mem0.

One Mem0 lookup when the learner picks a practice language, and one Mem0 add
when the session ends. Mem0 extracts the memories from the conversation with
`EXTRACTION_INSTRUCTIONS`. The chosen practice level is stored separately as an
explicit record, so it never depends on what extraction picks up. Pipecat's
`Mem0MemoryService` is not used because it searches and adds on every turn,
which adds latency and per-turn extraction cost.
See https://docs.mem0.ai/platform/quickstart
"""

import asyncio
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import datetime
from typing import Literal, Protocol, Self

from loguru import logger
from mem0 import AsyncMemoryClient
from mem0.client.types import AddMemoryOptions, GetAllMemoryOptions
from pydantic import BaseModel, ConfigDict, Field, TypeAdapter, ValidationError

from language_coach.caller import CallerNumber
from language_coach.practice import PracticeLanguage, PracticeLevel

RECALL_LIMIT = 30
LEVEL_KIND = "practice_level"
_OBJECT_LIST = TypeAdapter(list[object])

EXTRACTION_INSTRUCTIONS = """\
This is a phone call between a language coach and a learner. Extract only facts that help \
coach this learner in future calls:
- Phrases, words, or grammar points the coach taught, quoting the phrase.
- Recurring mistakes, naming the category (for example ser versus estar, false friend, \
th sound, verb tense) with one short example.
- Progress: things the learner now does correctly that they previously got wrong, or new \
skills they showed.
- The learner's apparent level and how it changed.
- Preferences: speaking speed, amount of correction, favorite or requested scenarios.
- Scenarios practiced in this call.
- Personal details the learner shared that make good future conversation topics.
Write each memory as one short sentence in English. Ignore greetings, small talk with \
no coaching value, and anything the coach said about itself.\
"""


@dataclass(frozen=True, slots=True, repr=False)
class LearnerId:
    """Mem0 `user_id`: one memory profile per caller and practice language."""

    value: str
    caller: CallerNumber
    language: PracticeLanguage

    @classmethod
    def for_caller(cls, caller: CallerNumber, language: PracticeLanguage) -> Self:
        """Build the Mem0 user id for a caller practicing `language`."""
        return cls(f"phone:{caller.e164}:{language.value}", caller, language)

    def __str__(self) -> str:
        return f"{self.caller}:{self.language.value}"

    def __repr__(self) -> str:
        return f"LearnerId({self})"


class RecordMetadata(BaseModel):
    """Metadata this bot attaches to memories; other keys are ignored.

    Explicit level records have `kind == LEVEL_KIND` and a `level`.
    """

    model_config = ConfigDict(frozen=True, extra="ignore")

    kind: str | None = None
    level: PracticeLevel | None = None


class MemoryRecord(BaseModel):
    """One stored memory from Mem0. Unknown response fields are ignored."""

    model_config = ConfigDict(frozen=True, extra="ignore")

    memory: str = Field(min_length=1)
    metadata: RecordMetadata | None = None
    created_at: datetime | None = None
    updated_at: datetime | None = None

    @property
    def practice_level(self) -> PracticeLevel | None:
        """The recorded level, if this is an explicit level record."""
        if self.metadata is None or self.metadata.kind != LEVEL_KIND:
            return None
        return self.metadata.level

    def for_prompt(self) -> str:
        """Memory text prefixed with the date it was last updated, if known."""
        when = self.updated_at or self.created_at
        return f"{when:%Y-%m-%d}: {self.memory}" if when else self.memory


class ConversationMessage(BaseModel):
    """A spoken turn sent to Mem0 for extraction."""

    model_config = ConfigDict(frozen=True)

    role: Literal["user", "assistant"]
    content: str


class _Mem0Page(BaseModel):
    """Mem0 v3 paginated response; records are validated one by one."""

    results: list[object]


class _TextPart(BaseModel):
    type: Literal["text"]
    text: str


class _ContextMessage(BaseModel):
    """The parts of a Pipecat LLM context message that memory needs."""

    model_config = ConfigDict(extra="ignore")

    role: str
    content: str | list[_TextPart | object] | None = None

    def spoken_text(self) -> str:
        if isinstance(self.content, str):
            return self.content.strip()
        if self.content is None:
            return ""
        return " ".join(p.text for p in self.content if isinstance(p, _TextPart)).strip()


def parse_memory_records(response: object) -> list[MemoryRecord]:
    """Parse a Mem0 `get_all` response, newest first, skipping malformed entries.

    Mem0 v3 returns `{"count", "next", "previous", "results": [...]}`; a bare list is
    also accepted.
    """
    try:
        items = (
            _Mem0Page.model_validate(response).results
            if isinstance(response, dict)
            else _OBJECT_LIST.validate_python(response)
        )
    except ValidationError:
        return []
    records: list[MemoryRecord] = []
    for item in items:
        try:
            records.append(MemoryRecord.model_validate(item))
        except ValidationError:
            continue
    return sorted(records, key=_recency, reverse=True)


def _recency(record: MemoryRecord) -> float:
    when = record.updated_at or record.created_at
    return when.timestamp() if when else 0.0


def conversation_for_memory(messages: Sequence[object]) -> list[ConversationMessage]:
    """Spoken user and assistant text from an LLM context; drops prompts and tool traffic."""
    conversation: list[ConversationMessage] = []
    for raw in messages:
        try:
            message = _ContextMessage.model_validate(raw)
        except ValidationError:
            continue
        text = message.spoken_text()
        if text and message.role in ("user", "assistant"):
            role: Literal["user", "assistant"] = "user" if message.role == "user" else "assistant"
            conversation.append(ConversationMessage(role=role, content=text))
    return conversation


class LearnerMemoryStore(Protocol):
    """Long-term memory backend."""

    async def recall(self, learner: LearnerId) -> list[MemoryRecord]:
        """Memories for `learner`, newest first."""
        ...

    async def remember(
        self,
        learner: LearnerId,
        language: PracticeLanguage,
        conversation: list[ConversationMessage],
    ) -> None:
        """Extract and store memories from a finished conversation."""
        ...

    async def remember_level(self, learner: LearnerId, level: PracticeLevel) -> None:
        """Store `level` verbatim as the learner's chosen practice level."""
        ...


class Mem0Store:
    """`LearnerMemoryStore` backed by the hosted Mem0 platform."""

    def __init__(self, client: AsyncMemoryClient) -> None:
        self._client = client

    @classmethod
    def connect(cls, api_key: str) -> Self:
        """Create the client. Blocks to validate the key, so call it at startup only."""
        return cls(AsyncMemoryClient(api_key=api_key))

    @classmethod
    def try_connect(cls, api_key: str) -> Self | None:
        """`connect`, or `None` with an error log if Mem0 is unreachable or rejects the key.

        Instances cold-start per call (scale to zero), so a Mem0 outage at startup must
        not stop the bot from answering; calls then run without memory.
        """
        try:
            return cls.connect(api_key)
        except Exception as error:
            logger.error(f"memory disabled: Mem0 connection failed ({type(error).__name__})")
            return None

    async def recall(self, learner: LearnerId) -> list[MemoryRecord]:
        options = GetAllMemoryOptions(filters={"user_id": learner.value}, page_size=RECALL_LIMIT)
        # AsyncMemoryClient methods also declare untyped **kwargs.
        response: object = await self._client.get_all(options)  # pyright: ignore[reportUnknownMemberType]
        return parse_memory_records(response)[:RECALL_LIMIT]

    async def remember(
        self,
        learner: LearnerId,
        language: PracticeLanguage,
        conversation: list[ConversationMessage],
    ) -> None:
        options = AddMemoryOptions(
            metadata={"practice_language": language.value},
            custom_instructions=EXTRACTION_INSTRUCTIONS,
        )
        await self._add(learner, [message.model_dump() for message in conversation], options)

    async def remember_level(self, learner: LearnerId, level: PracticeLevel) -> None:
        options = AddMemoryOptions(
            metadata={
                "practice_language": learner.language.value,
                "kind": LEVEL_KIND,
                "level": level.value,
            },
            infer=False,
        )
        await self._add(
            learner, [{"role": "user", "content": f"Practice level: {level.value}"}], options
        )

    async def _add(
        self, learner: LearnerId, messages: list[dict[str, str]], options: AddMemoryOptions
    ) -> None:
        """Add memories for `learner`.

        The SDK says to pass entity ids in `filters`, but the v3 add endpoint rejects
        that with a 400 and needs a top-level `user_id`.
        """
        # AsyncMemoryClient methods also declare untyped **kwargs.
        await self._client.add(messages, options, user_id=learner.value)  # pyright: ignore[reportUnknownMemberType]

    async def delete(self, learner: LearnerId) -> None:
        """Delete every memory for `learner`."""
        # Like add, delete_all needs a top-level `user_id`; `filters` is rejected.
        # AsyncMemoryClient methods also declare untyped **kwargs.
        await self._client.delete_all(user_id=learner.value)  # pyright: ignore[reportUnknownMemberType]


class SessionMemory:
    """Memory for one phone caller. Never slows or breaks the call.

    `prefetch` looks up both practice languages in the background as soon as the
    session starts, while the greeting plays. `recalled` waits at most
    `recall_wait_seconds` for that lookup, then continues without memory.
    """

    def __init__(
        self,
        store: LearnerMemoryStore,
        caller: CallerNumber,
        *,
        timeout_seconds: float,
        recall_wait_seconds: float,
    ) -> None:
        self._store = store
        self._caller = caller
        self._timeout_seconds = timeout_seconds
        self._recall_wait_seconds = recall_wait_seconds
        self._prefetch: asyncio.Task[dict[PracticeLanguage, list[MemoryRecord]]] | None = None

    def prefetch(self) -> None:
        """Start looking up memories for both practice languages; idempotent."""
        if self._prefetch is None:
            self._prefetch = asyncio.create_task(self._recall_all(), name="memory-prefetch")

    async def recalled(self, language: PracticeLanguage) -> list[MemoryRecord]:
        """Prefetched memories for `language`, or none if not ready within the wait budget."""
        self.prefetch()
        task = self._prefetch
        assert task is not None
        done, _ = await asyncio.wait({task}, timeout=self._recall_wait_seconds)
        if not done:
            learner = LearnerId.for_caller(self._caller, language)
            logger.warning(f"memory not ready in time for {learner}; continuing without it")
            return []
        return task.result()[language]

    async def close(self) -> None:
        """Cancel a lookup that is still running when the session ends."""
        if self._prefetch is not None and not self._prefetch.done():
            self._prefetch.cancel()
            await asyncio.gather(self._prefetch, return_exceptions=True)

    async def _recall_all(self) -> dict[PracticeLanguage, list[MemoryRecord]]:
        languages = list(PracticeLanguage)
        results = await asyncio.gather(*(self._recall(language) for language in languages))
        return dict(zip(languages, results, strict=True))

    async def _recall(self, language: PracticeLanguage) -> list[MemoryRecord]:
        learner = LearnerId.for_caller(self._caller, language)
        try:
            records = await asyncio.wait_for(self._store.recall(learner), self._timeout_seconds)
        except Exception as error:
            logger.warning(f"memory recall failed for {learner}: {type(error).__name__}")
            return []
        logger.info(f"recalled {len(records)} memories for {learner}")
        return records

    async def remember(
        self, language: PracticeLanguage, conversation: list[ConversationMessage]
    ) -> None:
        """Store memories from this session; skipped if the learner never spoke."""
        learner = LearnerId.for_caller(self._caller, language)
        if not any(message.role == "user" for message in conversation):
            logger.info(f"no learner speech to remember for {learner}")
            return
        try:
            await asyncio.wait_for(
                self._store.remember(learner, language, conversation), self._timeout_seconds
            )
        except Exception as error:
            logger.warning(f"memory save failed for {learner}: {type(error).__name__}")
            return
        logger.info(f"saved {len(conversation)} messages to memory for {learner}")

    async def remember_level(self, language: PracticeLanguage, level: PracticeLevel) -> None:
        """Record the session's level unless the prefetched memories already have it."""
        learner = LearnerId.for_caller(self._caller, language)
        if self._prefetched_level(language) is level:
            return
        try:
            await asyncio.wait_for(
                self._store.remember_level(learner, level), self._timeout_seconds
            )
        except Exception as error:
            logger.warning(f"level save failed for {learner}: {type(error).__name__}")
            return
        logger.info(f"saved level {level} for {learner}")

    def _prefetched_level(self, language: PracticeLanguage) -> PracticeLevel | None:
        task = self._prefetch
        if task is None or not task.done() or task.cancelled():
            return None
        return remembered_level(task.result()[language])


def memories_for_prompt(records: Sequence[MemoryRecord]) -> list[str]:
    """Memory lines for the LLM, newest first, without explicit level records."""
    return [record.for_prompt() for record in records if record.practice_level is None]


def remembered_level(records: Sequence[MemoryRecord]) -> PracticeLevel | None:
    """The level from the first (newest) explicit level record, if any."""
    return next((r.practice_level for r in records if r.practice_level is not None), None)
