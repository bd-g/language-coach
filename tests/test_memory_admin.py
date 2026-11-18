import pytest

from language_coach.caller import CallerNumber
from language_coach.memory_admin import Action, AdminCommand, parse_args
from language_coach.practice import PracticeLanguage


def test_show_defaults_to_both_languages() -> None:
    command = parse_args(["show", "+15551234567"])

    assert command == AdminCommand(
        action=Action.SHOW,
        caller=CallerNumber.parse("+15551234567"),
        languages=(PracticeLanguage.ENGLISH, PracticeLanguage.SPANISH),
    )


def test_delete_can_target_one_language() -> None:
    command = parse_args(["delete", "+15551234567", "--language", "spanish"])

    assert command.action is Action.DELETE
    assert command.languages == (PracticeLanguage.SPANISH,)


@pytest.mark.parametrize(
    "argv",
    [["show", "555"], ["purge", "+15551234567"], ["show", "+15551234567", "--language", "fr"]],
)
def test_invalid_arguments_exit(argv: list[str]) -> None:
    with pytest.raises(SystemExit):
        parse_args(argv)
