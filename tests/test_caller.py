import pytest

from language_coach.caller import Allowlist, CallerNumber, InvalidPhoneNumberError


def test_parses_e164_number() -> None:
    caller = CallerNumber.parse("+15551234567")

    assert caller.e164 == "+15551234567"
    assert caller.last4 == "4567"


@pytest.mark.parametrize(
    "raw", ["", "5551234567", "+0551234567", "+1555-123-4567", "+1", "anonymous"]
)
def test_rejects_non_e164_numbers(raw: str) -> None:
    with pytest.raises(InvalidPhoneNumberError):
        CallerNumber.parse(raw)


def test_repr_and_str_never_expose_the_full_number() -> None:
    caller = CallerNumber.parse("+15551234567")

    assert "555123" not in repr(caller)
    assert "555123" not in str(caller)
    assert "4567" in str(caller)


def test_invalid_number_error_does_not_echo_input() -> None:
    with pytest.raises(InvalidPhoneNumberError) as error:
        CallerNumber.parse("+1555-123-4567")

    assert "123" not in str(error.value)


def test_allowlist_parses_comma_separated_numbers() -> None:
    allowlist = Allowlist.parse(" +15551234567, +5215512345678 ")

    assert allowlist.allows(CallerNumber.parse("+15551234567"))
    assert allowlist.allows(CallerNumber.parse("+5215512345678"))
    assert not allowlist.allows(CallerNumber.parse("+15550000000"))


@pytest.mark.parametrize("raw", ["", " , ", "+15551234567,not-a-number"])
def test_allowlist_rejects_empty_or_invalid_entries(raw: str) -> None:
    with pytest.raises(InvalidPhoneNumberError):
        Allowlist.parse(raw)
