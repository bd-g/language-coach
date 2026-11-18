# QUESTIONS.md

## Open

1. **Voice evaluation.** The earlier "Daniela" voice ID does not exist in Cartesia's library. No Cartesia voice is natively Mexican and also native in `en-US`. We are testing Skylar (multilingual: native `en-US` plus an `es-MX` accent) with `language = "es-MX"`. Fernanda (native Mexican only) is commented in `config/bot.toml`. If neither works, build the per-sentence language switching processor, so Skylar can speak English sentences with `en-US` and Spanish with `es-MX`. How does Skylar sound?
2. **Rejected-call billing.** Unknown callers are connected and then disconnected immediately, without any STT, LLM, or TTS services being created. Daily's docs don't say whether pinless hold time is billed. Please check the first invoice line items after a test from a non-allowlisted number to confirm this is the cheapest path.
3. **Eval harness.** Pipecat's `pipecat eval` scripted scenarios could check tool calls (speed change, goodbye) end to end, but they need live LLM keys and, for judged criteria, a local Ollama model. Add a small manual eval suite later, or keep to unit tests for now?

## Resolved

- Language: Pipecat requires Python, so the Rust workspace was replaced. Typing is enforced with pyright strict.
- Telephony: a Daily number via Pipecat Cloud's managed dial-in webhook. Unknown callers are rejected inside the bot, with no extra webhook service.
- Scaling: `min_agents = 0`, `max_agents = 3`, region `us-east`.
- LLM: Pipecat quickstart default (`OpenAIResponsesLLMService`, `gpt-4.1`).
- Deploys: the Pipecat GitHub integration deploys `main`; `main` is protected and requires the Checks workflow on pull requests.
- Krisp VIVA: enabled (`tel` filter, free for the first 10,000 minutes a month), applied only on Pipecat Cloud.
- Transcript privacy: the image sets `PCC_WITHHOLD_CONTENT=true`, so Pipecat Cloud does not publish transcripts or tool arguments. This matches the old no-persistence rule.
