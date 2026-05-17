# QUESTIONS.md

## Open

1. For initial Deepgram integration, should we keep Flux turn-taking fully default (no `eot_threshold` overrides) exactly as the implementation plan suggests, or do you already want a fixed non-default tuning profile for local debug sessions?
2. Should one-time Telnyx call tokens use a dedicated TTL separate from the webhook dedupe TTL, and if so what should that duration be?
