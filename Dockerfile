FROM rust:1-alpine AS builder

RUN apk add --no-cache build-base ca-certificates perl

WORKDIR /workspace
COPY . .
RUN cargo build --release -p language-coach-server --target x86_64-unknown-linux-musl \
    && strip /workspace/target/x86_64-unknown-linux-musl/release/language-coach-server

FROM scratch

WORKDIR /app
COPY --from=builder /workspace/target/x86_64-unknown-linux-musl/release/language-coach-server /usr/local/bin/language-coach-server

USER 65532:65532

ENTRYPOINT ["/usr/local/bin/language-coach-server"]
CMD ["--config", "/app/config/production.toml"]
