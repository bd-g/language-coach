"""Decide whether a session may start a coaching pipeline."""

from enum import StrEnum

from language_coach.caller import Allowlist
from language_coach.origin import DailyRoomSession, PhoneCall, SessionOrigin, WebRtcSession


class RejectReason(StrEnum):
    """Why a phone call is turned away before any AI service starts."""

    NO_CALLER_ID = "no_caller_id"
    NOT_ALLOWLISTED = "not_allowlisted"


def rejection_reason(origin: SessionOrigin, allowlist: Allowlist) -> RejectReason | None:
    """`None` admits the session.

    Browser sessions can only be started with Pipecat Cloud credentials or on a
    local dev runner, so only phone calls are checked against the allowlist.
    """
    match origin:
        case PhoneCall(caller=None):
            return RejectReason.NO_CALLER_ID
        case PhoneCall(caller=caller) if caller is not None and not allowlist.allows(caller):
            return RejectReason.NOT_ALLOWLISTED
        case PhoneCall() | DailyRoomSession() | WebRtcSession():
            return None
