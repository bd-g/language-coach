"""Review or delete a learner's memories.

    python -m language_coach.memory_admin show +15551234567 [--language spanish]
    python -m language_coach.memory_admin delete +15551234567 [--language english]

Reads `MEM0_API_KEY` from the environment or `.env`.
"""

import argparse
import asyncio
import os
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from enum import StrEnum

from dotenv import load_dotenv

from language_coach.caller import CallerNumber, InvalidPhoneNumberError
from language_coach.memory import LearnerId, Mem0Store
from language_coach.practice import PracticeLanguage


class Action(StrEnum):
    """What to do with the learner's memories."""

    SHOW = "show"
    DELETE = "delete"


@dataclass(frozen=True, slots=True)
class AdminCommand:
    """A parsed command line."""

    action: Action
    caller: CallerNumber
    languages: tuple[PracticeLanguage, ...]


def parse_args(argv: Sequence[str]) -> AdminCommand:
    """Parse and validate the command line; exits with usage on invalid input."""
    parser = argparse.ArgumentParser(prog="python -m language_coach.memory_admin")
    parser.add_argument("action", choices=[action.value for action in Action])
    parser.add_argument("phone", help="E.164 number, e.g. +15551234567")
    parser.add_argument("--language", choices=[language.value for language in PracticeLanguage])
    args = parser.parse_args(argv)
    action = Action(str(args.action))
    language = str(args.language) if args.language is not None else None
    try:
        caller = CallerNumber.parse(str(args.phone))
    except InvalidPhoneNumberError as error:
        parser.error(str(error))
    languages = (PracticeLanguage(language),) if language else tuple(PracticeLanguage)
    return AdminCommand(action=action, caller=caller, languages=languages)


async def _run(command: AdminCommand, store: Mem0Store) -> None:
    for language in command.languages:
        learner = LearnerId.for_caller(command.caller, language)
        match command.action:
            case Action.SHOW:
                records = await store.recall(learner)
                print(f"{language.value}: {len(records)} memories")
                for record in records:
                    print(f"  {record.for_prompt()}")
            case Action.DELETE:
                await store.delete(learner)
                print(f"{language.value}: deleted")


def main() -> None:
    """Command-line entry point."""
    command = parse_args(sys.argv[1:])
    load_dotenv()
    api_key = os.environ.get("MEM0_API_KEY", "").strip()
    if not api_key:
        sys.exit("MEM0_API_KEY is not set")
    asyncio.run(_run(command, Mem0Store.connect(api_key)))


if __name__ == "__main__":
    main()
