"""Caller phone numbers and the allowlist that gates phone sessions."""

import re
from dataclasses import dataclass
from typing import Self

_E164 = re.compile(r"\+[1-9][0-9]{6,14}")


class InvalidPhoneNumberError(ValueError):
    """A phone number was not E.164. The message never echoes the input."""


@dataclass(frozen=True, slots=True, repr=False)
class CallerNumber:
    """An E.164 phone number.

    `str` and `repr` show only the last four digits so the number can be logged safely.
    """

    e164: str

    @classmethod
    def parse(cls, raw: str) -> Self:
        """Parse an E.164 number such as `+15551234567`."""
        candidate = raw.strip()
        if not _E164.fullmatch(candidate):
            raise InvalidPhoneNumberError("phone number is not in E.164 format")
        return cls(candidate)

    @property
    def last4(self) -> str:
        """The last four digits, for logs."""
        return self.e164[-4:]

    def __str__(self) -> str:
        return f"…{self.last4}"

    def __repr__(self) -> str:
        return f"CallerNumber(…{self.last4})"


@dataclass(frozen=True, slots=True)
class Allowlist:
    """Callers allowed to reach the coach by phone."""

    numbers: frozenset[CallerNumber]

    @classmethod
    def parse(cls, raw: str) -> Self:
        """Parse a non-empty, comma-separated list of E.164 numbers."""
        numbers = frozenset(CallerNumber.parse(part) for part in raw.split(",") if part.strip())
        if not numbers:
            raise InvalidPhoneNumberError("allowlist must contain at least one number")
        return cls(numbers)

    def allows(self, caller: CallerNumber) -> bool:
        """Whether `caller` may start a coaching session."""
        return caller in self.numbers
