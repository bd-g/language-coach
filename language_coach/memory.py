"""Per-learner long-term memory, stored in Mem0.

One Mem0 lookup when the learner picks a practice language, and one Mem0 add
when the session ends. Mem0 extracts the memories from the conversation with
`EXTRACTION_INSTRUCTIONS`. Pipecat's `Mem0MemoryService` is not used because it
searches and adds on every turn, which adds latency and per-turn extraction cost.
See https://docs.mem0.ai/platform/quickstart
"""

import asyncio
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import datetime
from typing import Literal, Protocol, Self

from loguru import logger
from mem0 import AsyncMemoryClient
from mem0.client.types import AddMemoryOptions, DeleteAllMemoryOptions, GetAllMemoryOptions
from pydantic import BaseModel, ConfigDict, Field, TypeAdapter, ValidationError

from language_coach.caller import CallerNumber
from language_coach.practice import PracticeLanguage

RECALL_LIMIT = 30
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


class MemoryRecord(BaseModel):
    """One stored memory from Mem0. Unknown response fields are ignored."""

    model_config = ConfigDict(frozen=True, extra="ignore")

    memory: str = Field(min_length=1)
    created_at: datetime | None = None
    updated_at: datetime | None = None

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
            filters={"user_id": learner.value},
            metadata={"practice_language": language.value},
            custom_instructions=EXTRACTION_INSTRUCTIONS,
        )
        messages = [message.model_dump() for message in conversation]
        # AsyncMemoryClient methods also declare untyped **kwargs.
        await self._client.add(messages, options)  # pyright: ignore[reportUnknownMemberType]

    async def delete(self, learner: LearnerId) -> None:
        """Delete every memory for `learner`."""
        options = DeleteAllMemoryOptions(filters={"user_id": learner.value})
        # AsyncMemoryClient methods also declare untyped **kwargs.
        await self._client.delete_all(options)  # pyright: ignore[reportUnknownMemberType]


class SessionMemory:
    """Memory for one phone caller. Failures and timeouts never break the call."""

    def __init__(
        self, store: LearnerMemoryStore, caller: CallerNumber, *, timeout_seconds: float
    ) -> None:
        self._store = store
        self._caller = caller
        self._timeout_seconds = timeout_seconds

    async def recall(self, language: PracticeLanguage) -> list[MemoryRecord]:
        """Memories for this caller in `language`; empty if Mem0 is slow or failing."""
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


def memories_for_prompt(records: Sequence[MemoryRecord]) -> list[str]:
    """Memory lines for the LLM, newest first."""
    return [record.for_prompt() for record in records]
