"""Which language the caller is practicing."""

from dataclasses import dataclass
from enum import StrEnum


class PracticeLanguage(StrEnum):
    """The language used for role-play; coaching happens in the other one."""

    ENGLISH = "english"
    SPANISH = "spanish"


@dataclass(slots=True)
class PracticeChoice:
    """Per-session practice language, set by the `choose_practice_language` tool.

    `None` until the caller has chosen; logged with the session summary.
    """

    language: PracticeLanguage | None = None

    def label(self) -> str:
        """Value for logs."""
        return self.language.value if self.language else "unselected"
