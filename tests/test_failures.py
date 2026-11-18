from language_coach.failures import Provider, ProviderFailureMonitor


class FakeClock:
    def __init__(self) -> None:
        self.now = 0.0

    def __call__(self) -> float:
        return self.now


STT, LLM, TTS = object(), object(), object()


def monitor(clock: FakeClock) -> ProviderFailureMonitor:
    return ProviderFailureMonitor(
        {STT: Provider.STT, LLM: Provider.LLM, TTS: Provider.TTS},
        max_errors=3,
        window_seconds=30.0,
        clock=clock,
    )


def test_unusable_provider_ends_the_session_immediately() -> None:
    failures = monitor(FakeClock())

    assert failures.record(LLM, usable=False) is Provider.LLM


def test_repeated_errors_within_the_window_end_the_session() -> None:
    clock = FakeClock()
    failures = monitor(clock)

    assert failures.record(TTS, usable=True) is None
    clock.now = 10.0
    assert failures.record(TTS, usable=True) is None
    clock.now = 20.0
    assert failures.record(TTS, usable=True) is Provider.TTS


def test_errors_spread_beyond_the_window_are_tolerated() -> None:
    clock = FakeClock()
    failures = monitor(clock)

    for second in (0.0, 20.0, 40.0, 60.0):
        clock.now = second
        assert failures.record(STT, usable=True) is None


def test_errors_are_counted_per_provider() -> None:
    failures = monitor(FakeClock())

    assert failures.record(STT, usable=True) is None
    assert failures.record(LLM, usable=True) is None
    assert failures.record(TTS, usable=True) is None


def test_errors_from_other_processors_are_ignored() -> None:
    failures = monitor(FakeClock())

    assert failures.record(object(), usable=False) is None
    assert failures.record(None, usable=False) is None


def test_session_is_ended_only_once() -> None:
    failures = monitor(FakeClock())

    assert failures.record(LLM, usable=False) is Provider.LLM
    assert failures.record(STT, usable=False) is None
