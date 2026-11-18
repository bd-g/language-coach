"""Pipecat Cloud entry point. Run locally with `python bot.py` for the browser playground."""

import os

from dotenv import load_dotenv
from pipecat.runner.types import RunnerArguments

from language_coach.config import DEFAULT_CONFIG_PATH, Runtime, load_app_config
from language_coach.session import run_session

load_dotenv()
# The dev runner (`python bot.py`) runs this module as __main__; Pipecat Cloud imports it.
CONFIG = load_app_config(
    os.environ,
    DEFAULT_CONFIG_PATH,
    Runtime.LOCAL if __name__ == "__main__" else Runtime.CLOUD,
)


async def bot(runner_args: RunnerArguments) -> None:
    """Run one coaching session."""
    await run_session(runner_args, CONFIG)


if __name__ == "__main__":
    from pipecat.runner.run import main

    main()
