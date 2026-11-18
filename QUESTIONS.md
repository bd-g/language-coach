# QUESTIONS.md

## Open

1. **Cartesia voice and locale.** The default voice is Cartesia "Daniela" (`5c5ad5e7-1020-476b-8b91-fdcbe9cc313c`, Mexican Spanish) with `language = "es-MX"`. That ID comes from a third-party voice list and needs checking against your Cartesia account. Cartesia sends one language per request and warns that mixing languages in one generation can sound accented, so English role-play lines may come out with a Spanish accent. Do you want to test this voice first, or pick another one in the Cartesia library that supports both `en-US` and `es-MX` accents?
2. **Per-sentence TTS language switching.** If no single voice and locale sounds good in both English and Spanish, we could add a small processor that detects each sentence's language and updates the TTS language mid-turn. That adds complexity. Try the plain providers first (including Deepgram Aura-2 Spanish voices, which code-switch), or plan for this now?
3. **Rejected-call billing.** Unknown callers are connected and then disconnected immediately, without any STT, LLM, or TTS services being created. Daily's docs don't say whether pinless hold time is billed. Please check the first invoice line items after a test from a non-allowlisted number to confirm this is the cheapest path.
4. **Krisp noise cancellation.** Pipecat Cloud offers Krisp VIVA (`[krisp_viva] audio_filter = "tel"`) for phone audio. It is left off to keep cost and setup minimal. Enable it if speech recognition struggles with noisy calls?
5. **Eval harness.** Pipecat's `pipecat eval` scripted scenarios could check tool calls (speed change, goodbye) end to end, but they need live LLM keys and, for judged criteria, a local Ollama model. Add a small manual eval suite later, or keep to unit tests for now?

## Resolved

- Language: Pipecat requires Python, so the Rust workspace was replaced. Typing is enforced with pyright strict.
- Telephony: a Daily number via Pipecat Cloud's managed dial-in webhook. Unknown callers are rejected inside the bot, with no extra webhook service.
- Scaling: `min_agents = 0`, `max_agents = 3`, region `us-east`.
- LLM: Pipecat quickstart default (`OpenAIResponsesLLMService`, `gpt-4.1`).
- Deploys: the Pipecat GitHub integration deploys `main`; `main` is protected and requires the Checks workflow on pull requests.
- Transcript privacy: the image sets `PCC_WITHHOLD_CONTENT=true`, so Pipecat Cloud does not publish transcripts or tool arguments. This matches the old no-persistence rule.
