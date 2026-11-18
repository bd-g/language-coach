"""Classify how a session reached the bot, parsing runner arguments at the boundary."""

from dataclasses import dataclass
from typing import Any

from pipecat.runner.types import (
    DailyDialinRequest,
    DailyRunnerArguments,
    RunnerArguments,
    SmallWebRTCRunnerArguments,
)
from pydantic import ValidationError

from language_coach.caller import CallerNumber, InvalidPhoneNumberError


class UnsupportedSessionError(Exception):
    """The runner handed the bot a session type or body it does not serve."""


@dataclass(frozen=True, slots=True)
class DailyRoom:
    """A Daily room the bot joins."""

    url: str
    token: str | None


@dataclass(frozen=True, slots=True)
class PhoneCall:
    """An inbound PSTN call through Pipecat Cloud's managed Daily dial-in webhook.

    `caller` is `None` when the caller ID is withheld or is not an E.164 number.
    See https://docs.pipecat.ai/deployment/pipecat-cloud/guides/telephony/daily-dial-in
    """

    room: DailyRoom
    request: DailyDialinRequest
    caller: CallerNumber | None


@dataclass(frozen=True, slots=True)
class DailyRoomSession:
    """A browser or `pipecat cloud agent start --use-daily` session in a Daily room."""

    room: DailyRoom


@dataclass(frozen=True, slots=True)
class WebRtcSession:
    """A local SmallWebRTC playground session from the Pipecat dev runner."""

    connection: Any


type SessionOrigin = PhoneCall | DailyRoomSession | WebRtcSession


def parse_origin(args: RunnerArguments) -> SessionOrigin:
    """Turn runner arguments into a typed session origin."""
    if isinstance(args, DailyRunnerArguments):
        room = DailyRoom(url=args.room_url, token=args.token)
        request = _dialin_request(args.body)
        if request is None:
            return DailyRoomSession(room=room)
        return PhoneCall(room=room, request=request, caller=_caller(request))
    if isinstance(args, SmallWebRTCRunnerArguments):
        return WebRtcSession(connection=args.webrtc_connection)
    raise UnsupportedSessionError(f"unsupported session type {type(args).__name__}")


def _dialin_request(body: Any) -> DailyDialinRequest | None:
    if isinstance(body, DailyDialinRequest):
        return body
    if not isinstance(body, dict) or "dialin_settings" not in body:
        return None
    try:
        return DailyDialinRequest.model_validate(body)
    except ValidationError as error:
        fields = sorted({".".join(map(str, e["loc"])) for e in error.errors()})
        raise UnsupportedSessionError(f"malformed dial-in body: {', '.join(fields)}") from None


def _caller(request: DailyDialinRequest) -> CallerNumber | None:
    raw = request.dialin_settings.From
    if raw is None:
        return None
    try:
        return CallerNumber.parse(raw)
    except InvalidPhoneNumberError:
        return None
