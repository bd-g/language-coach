from pipecat.runner.types import DailyDialinRequest

from language_coach.admission import RejectReason, rejection_reason
from language_coach.caller import Allowlist, CallerNumber
from language_coach.origin import DailyRoom, DailyRoomSession, PhoneCall, WebRtcSession

ALLOWLIST = Allowlist.parse("+15551234567")
ROOM = DailyRoom(url="https://example.daily.co/room", token="tok")
REQUEST = DailyDialinRequest.model_validate(
    {
        "dialin_settings": {"call_id": "call", "call_domain": "domain"},
        "daily_api_key": "key",
        "daily_api_url": "https://api.daily.co/v1",
    }
)


def phone_call(caller: str | None) -> PhoneCall:
    number = None if caller is None else CallerNumber.parse(caller)
    return PhoneCall(room=ROOM, request=REQUEST, caller=number)


def test_allowlisted_caller_is_admitted() -> None:
    assert rejection_reason(phone_call("+15551234567"), ALLOWLIST) is None


def test_unknown_caller_is_rejected() -> None:
    assert rejection_reason(phone_call("+15550000000"), ALLOWLIST) is RejectReason.NOT_ALLOWLISTED


def test_withheld_caller_id_is_rejected() -> None:
    assert rejection_reason(phone_call(None), ALLOWLIST) is RejectReason.NO_CALLER_ID


def test_browser_sessions_are_admitted() -> None:
    assert rejection_reason(DailyRoomSession(room=ROOM), ALLOWLIST) is None
    assert rejection_reason(WebRtcSession(connection=object()), ALLOWLIST) is None
