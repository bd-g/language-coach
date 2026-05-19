FROM rust:1-bookworm AS builder

WORKDIR /workspace
COPY . .
RUN cargo build --release -p language-coach-server

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /workspace/target/release/language-coach-server /usr/local/bin/language-coach-server

CMD ["/usr/local/bin/language-coach-server", "--config", "/app/config/production.toml"]
