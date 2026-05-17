# AGENTS.md

## Engineering Style

- Write small, direct Rust. Prefer the simplest correct design over framework, abstraction, or infrastructure growth.
- Lean on the type system. Parse into domain types at boundaries; do not pass loosely validated strings, JSON blobs, or primitive aliases through core logic.
- Avoid duplication aggressively. If two paths exist, share the protocol/session boundary rather than copy behavior.
- Keep infrastructure boring: one server binary, explicit config, local fake peers for tests, and no new services unless the plan requires them.
- Use concise rustdoc on public structs, enums, traits, and functions that model data flow or protocol behavior. Avoid obvious comments in function bodies.
- Keep an explicit running list of unresolved user questions in `QUESTIONS.md` during implementation. Update it as you discover uncertainties, then present that list at the end of a working phase.
- Incorporate user code review feedback into both code changes and this file when the feedback describes a durable engineering preference.
- Prefer module layouts that keep entrypoints small and group concerns into named files like `handler.rs` plus `handler/route.rs`; avoid concentrating project logic in `main.rs` or `lib.rs`.

## Testing

- Use TDD for new behavior: write the smallest failing unit or fake-peer integration test first, then implement.
- Prefer deterministic tests over live provider calls. Do not require Telnyx or Deepgram credentials in CI.
- Test protocol boundaries, config parsing, metrics side effects, and failure paths as first-class behavior.

## Rust Practices

- Prefer explicit enums and newtypes for session modes, IDs, auth tokens, phone numbers, codecs, provider events, and error classes.
- Keep parsing, validation, and redaction at the edge. Core code should receive typed, trusted values.
- Model external API payloads with strict Serde structs/enums (including event discriminators and documented variants) instead of manual JSON/value-based deserialization.
- Prefer Serde-derived models over ad-hoc parsing: use tagged enums for discriminated payloads, and field-level custom deserializers only when the API shape requires adaptation.
- Avoid passing `serde_json::Value` through core logic; deserialize into typed domain structs as early as possible and keep `Value` usage isolated to schema-like passthrough fields.
- Prefer iterators and small functions when they clarify intent, but do not split code just to create helpers.
- Propagate typed errors with enough context for logs and metrics without exposing secrets or PII.
- Avoid redundant Serde attributes; do not add `#[serde(default)]` on `Option<T>` fields because missing values already deserialize to `None`.
- Never invoke async code between iterations of a `tokio::select!` loop. Queue async sink writes through `sink_queue` or an equivalent send task so the loop returns directly to `tokio::select!` after handling each branch.

## Protocol Modeling

- Treat `crates/protocol` as the provider wire-format boundary. Telnyx, Deepgram, and debug media JSON should deserialize into typed Rust structs/enums before reaching session logic.
- Put provider wire types in a provider-named module (`telnyx::Event`, `deepgram::ServerMessage`) and avoid repeating the provider in type names inside that module.
- Every public wire message enum, variant payload, and struct must have concise rustdoc with a canonical API spec URL when the type maps to provider behavior. Rustdoc should explain what is not obvious from the module/type/field names: wire-format gotchas, provider quirks, and why a shape is modeled a certain way. Do not restate provider, direction, or discriminator details when the module name, parent enum, Serde attributes, and variant/type names already communicate them.
- Prefer native Serde attributes for provider shapes: `tag`, `rename`, `rename_all`, `with`, `flatten`, and `untagged` when they directly match the API shape.
- Do not hand-write JSON dispatch, stringly typed message parsing, or manual discriminator matching when Serde can model the protocol directly.
- Use field-level custom serializers/deserializers only for real wire-format mismatches, such as JSON strings containing nested JSON or base64-encoded bytes. Document the provider reason in rustdoc or a focused test name.
- Model discriminators, provider names, codecs, containers, roles, and message types as enums/newtypes instead of unconstrained `String`s unless the provider explicitly allows arbitrary values.
- Unknown additive fields from providers may be tolerated. Unknown message type/event discriminants must be surfaced as explicit `tracing` errors at the receive boundary.
- Do not pass `serde_json::Value` through core protocol/session logic. Keep it only for schema passthrough fields or immediately parse it into typed tool/request structs at the boundary.
- Protocol tests must include documented provider fixtures for every modeled message type, including malformed payloads and unknown discriminators.
- When provider docs change or ambiguity is discovered, update rustdoc spec links and `QUESTIONS.md` before broadening protocol types.

## Verification

- Run `cargo fmt --check`, `cargo clippy --all-targets`, and relevant tests before considering code complete.
- If a change touches the full workspace or shared protocol behavior, run `cargo test --workspace`.
- Use the nix shell for all Rust development. If a new system dependency is needed, add it to `shell.nix` first. Do not rely on implicit host/system packages.
