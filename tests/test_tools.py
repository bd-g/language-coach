import pytest
from pydantic import ValidationError

from language_coach.config import DeepgramTts
from language_coach.practice import PracticeChoice
from language_coach.tools import EndCallArgs, EndCallReason, SetSpeakingRateArgs, coach_tools


def test_set_speaking_rate_args_parse() -> None:
    args = SetSpeakingRateArgs.model_validate({"speed": 0.85, "reason": "más despacio"})

    assert args.speed == pytest.approx(0.85)


@pytest.mark.parametrize("arguments", [{}, {"speed": "fast"}, {"speed": None}])
def test_set_speaking_rate_args_reject_invalid_input(arguments: dict[str, object]) -> None:
    with pytest.raises(ValidationError):
        SetSpeakingRateArgs.model_validate(arguments)


@pytest.mark.parametrize("reason", list(EndCallReason))
def test_end_call_reasons_parse(reason: EndCallReason) -> None:
    assert EndCallArgs.model_validate({"reason": reason.value}).reason is reason


def test_unknown_end_call_reason_is_rejected() -> None:
    with pytest.raises(ValidationError):
        EndCallArgs.model_validate({"reason": "free text with personal details"})


def test_coach_tools_expose_constrained_schemas() -> None:
    tools = {
        tool.name: tool
        for tool in coach_tools(DeepgramTts(provider="deepgram", voice="v"), PracticeChoice())
    }

    assert set(tools) == {"choose_practice_language", "set_speaking_rate", "end_call"}
    speed = tools["set_speaking_rate"].properties["speed"]
    assert (speed["minimum"], speed["maximum"]) == (0.7, 1.3)
    assert tools["end_call"].properties["reason"]["enum"] == [r.value for r in EndCallReason]
    assert all(tool.handler is not None for tool in tools.values())
