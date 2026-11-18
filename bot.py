"""Pipecat Cloud entry point. Run locally with `python bot.py` for the browser playground."""

import os

from dotenv import load_dotenv
from pipecat.runner.types import RunnerArguments

from language_coach.config import DEFAULT_CONFIG_PATH, Runtime, load_app_config
from language_coach.memory import Mem0Store
from language_coach.session import run_session

load_dotenv()
# The dev runner (`python bot.py`) runs this module as __main__; Pipecat Cloud imports it.
RUNTIME = Runtime.LOCAL if __name__ == "__main__" else Runtime.CLOUD
CONFIG = load_app_config(os.environ, DEFAULT_CONFIG_PATH, RUNTIME)
# Memory is keyed by phone caller, so local browser sessions never use it. If Mem0 is
# unreachable or rejects the key, calls still work, just without memory.
MEMORY_STORE = (
    Mem0Store.try_connect(CONFIG.secrets.mem0_api_key.get_secret_value())
    if CONFIG.secrets.mem0_api_key is not None and RUNTIME is Runtime.CLOUD
    else None
)


async def bot(runner_args: RunnerArguments) -> None:
    """Run one coaching session."""
    await run_session(runner_args, CONFIG, MEMORY_STORE)


if __name__ == "__main__":
    from pipecat.runner.run import main

    main()
