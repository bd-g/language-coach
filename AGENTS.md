# AGENTS.md

## Engineering Style

- Write small, direct Rust. Prefer the simplest correct design over framework, abstraction, or infrastructure growth.
- Lean on the type system. Parse into domain types at boundaries; do not pass loosely validated strings, JSON blobs, or primitive aliases through core logic.
- Avoid duplication aggressively. If two paths exist, share the protocol/session boundary rather than copy behavior.
- Keep infrastructure boring: one server binary, explicit config, local fake peers for tests, and no new services unless the plan requires them.
- Use concise rustdoc on public structs, enums, traits, and functions that model data flow or protocol behavior. Avoid obvious comments in function bodies.

## Testing

- Use TDD for new behavior: write the smallest failing unit or fake-peer integration test first, then implement.
- Prefer deterministic tests over live provider calls. Do not require Telnyx or Deepgram credentials in CI.
- Test protocol boundaries, config parsing, metrics side effects, and failure paths as first-class behavior.

## Rust Practices

- Prefer explicit enums and newtypes for session modes, IDs, auth tokens, phone numbers, codecs, provider events, and error classes.
- Keep parsing, validation, and redaction at the edge. Core code should receive typed, trusted values.
- Prefer iterators and small functions when they clarify intent, but do not split code just to create helpers.
- Propagate typed errors with enough context for logs and metrics without exposing secrets or PII.

## Verification

- Run `cargo fmt --check`, `cargo clippy --all-targets`, and relevant tests before considering code complete.
- If a change touches the full workspace or shared protocol behavior, run `cargo test --workspace`.
