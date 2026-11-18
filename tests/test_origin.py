from typing import Any, cast

import pytest
from pipecat.runner.types import (
    DailyDialinRequest,
    DailyRunnerArguments,
    RunnerArguments,
    SmallWebRTCRunnerArguments,
    WebSocketRunnerArguments,
)

from language_coach.caller import CallerNumber
from language_coach.origin import (
    DailyRoomSession,
    PhoneCall,
    UnsupportedSessionError,
    WebRtcSession,
    parse_origin,
)

# Body Pipecat Cloud's managed dial-in webhook passes to bot():
# https://docs.pipecat.ai/deployment/pipecat-cloud/guides/telephony/daily-dial-in
DIALIN_BODY: dict[str, Any] = {
    "dialin_settings": {
        "From": "+15551234567",
        "To": "+15559876543",
        "call_id": "a7bf8461-7d33-4083-9677-5cfdb4f337a5",
        "call_domain": "9c975848-a067-4de0-bf47-1b4179ceba4e",
    },
    "daily_api_key": "daily-key",
    "daily_api_url": "https://api.daily.co/v1",
}


def daily_args(body: Any) -> DailyRunnerArguments:
    return DailyRunnerArguments(room_url="https://example.daily.co/room", token="tok", body=body)


def test_dialin_body_becomes_phone_call() -> None:
    origin = parse_origin(daily_args(DIALIN_BODY))

    assert isinstance(origin, PhoneCall)
    assert origin.caller == CallerNumber.parse("+15551234567")
    assert origin.room.url == "https://example.daily.co/room"
    assert origin.request.dialin_settings.call_id == DIALIN_BODY["dialin_settings"]["call_id"]


def test_already_parsed_dialin_request_is_accepted() -> None:
    origin = parse_origin(daily_args(DailyDialinRequest.model_validate(DIALIN_BODY)))

    assert isinstance(origin, PhoneCall)


@pytest.mark.parametrize("from_value", [None, "anonymous", "sip:someone@example.com"])
def test_dialin_without_e164_caller_has_no_caller(from_value: str | None) -> None:
    body = {
        **DIALIN_BODY,
        "dialin_settings": {**DIALIN_BODY["dialin_settings"], "From": from_value},
    }

    origin = parse_origin(daily_args(body))

    assert isinstance(origin, PhoneCall)
    assert origin.caller is None


def test_malformed_dialin_body_is_unsupported() -> None:
    body = {"dialin_settings": {"From": "+15551234567"}}

    with pytest.raises(UnsupportedSessionError):
        parse_origin(daily_args(body))


@pytest.mark.parametrize("body", [{}, None, {"something": "else"}])
def test_daily_room_without_dialin_is_a_browser_session(body: Any) -> None:
    origin = parse_origin(daily_args(body))

    assert isinstance(origin, DailyRoomSession)
    assert origin.room.token == "tok"


def test_small_webrtc_is_a_webrtc_session() -> None:
    connection = object()

    origin = parse_origin(SmallWebRTCRunnerArguments(webrtc_connection=connection))

    assert isinstance(origin, WebRtcSession)
    assert origin.connection is connection


@pytest.mark.parametrize(
    "args",
    [RunnerArguments(), WebSocketRunnerArguments(websocket=cast("Any", object()))],
)
def test_other_session_types_are_unsupported(args: RunnerArguments) -> None:
    with pytest.raises(UnsupportedSessionError):
        parse_origin(args)
