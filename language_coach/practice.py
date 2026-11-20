"""Which language the caller is practicing, and at what level."""

from dataclasses import dataclass
from enum import StrEnum


class PracticeLanguage(StrEnum):
    """The language used for role-play; coaching happens in the other one."""

    ENGLISH = "english"
    SPANISH = "spanish"


class PracticeLevel(StrEnum):
    """The learner's chosen level; sets the starting speed and the coaching style."""

    BEGINNER = "beginner"
    INTERMEDIATE = "intermediate"
    ADVANCED = "advanced"


@dataclass(slots=True)
class PracticeChoice:
    """Per-session practice language and level, set by the coach's tools.

    Each is `None` until the caller has chosen; both are logged with the session summary.
    """

    language: PracticeLanguage | None = None
    level: PracticeLevel | None = None

    def label(self) -> str:
        """Language for logs."""
        return self.language.value if self.language else "unselected"

    def level_label(self) -> str:
        """Level for logs."""
        return self.level.value if self.level else "unselected"
