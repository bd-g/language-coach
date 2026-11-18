FROM dailyco/pipecat-base:0.4.2-py3.13

ENV UV_COMPILE_BYTECODE=1
ENV UV_LINK_MODE=copy
# Keep transcripts and tool arguments out of what Pipecat Cloud publishes.
ENV PCC_WITHHOLD_CONTENT=true
# Turn off the mem0ai SDK's usage telemetry.
ENV MEM0_TELEMETRY=False

RUN --mount=type=cache,target=/root/.cache/uv \
    --mount=type=bind,source=uv.lock,target=uv.lock \
    --mount=type=bind,source=pyproject.toml,target=pyproject.toml \
    uv sync --locked --no-install-project --no-dev

COPY ./config config
COPY ./language_coach language_coach
COPY ./bot.py bot.py
