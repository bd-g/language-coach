# QUESTIONS.md

## Open

1. **Voice evaluation.** The earlier "Daniela" voice ID does not exist in Cartesia's library. No Cartesia voice is natively Mexican and also native in `en-US`. We are testing Skylar (multilingual: native `en-US` plus an `es-MX` accent) with `language = "es-MX"`. Fernanda (native Mexican only) is commented in `config/bot.toml`. If neither works, build the per-sentence language switching processor, so Skylar can speak English sentences with `en-US` and Spanish with `es-MX`. How does Skylar sound?
2. **Greeting locale.** The bilingual greeting is spoken before a practice language is chosen, so it uses the configured Cartesia locale (`es-MX`). Once the learner chooses, the voice switches to `en-US` for English practice or `es-MX` for Spanish practice. Does the English half of the greeting, and English coaching for Spanish learners, sound natural enough with Skylar?
3. **Rejected-call billing.** Unknown callers are connected and then disconnected immediately, without any STT, LLM, or TTS services being created. Daily's docs don't say whether pinless hold time is billed. Please check the first invoice line items after a test from a non-allowlisted number to confirm this is the cheapest path.
4. **Mem0 extraction LLM key.** Mem0's free plan pays for its extraction model for your first 30 days (10,000 adds and 1,000 retrievals a month). After that, adds fail until you connect your own LLM key in the Mem0 dashboard; with your own key the limits are 50,000 adds and 10,000 retrievals a month. This bot uses one add and one retrieval per call. Use your OpenAI key there when the trial ends?
5. **Memory tuning.** Mem0 extracts memories with the instructions in `language_coach/memory.py`. After a few real calls, review them with `python -m language_coach.memory_admin show <number>`. Are they the right granularity (phrases, mistakes, progress), or too noisy?
6. **Eval harness.** Pipecat's `pipecat eval` scripted scenarios could check tool calls (speed change, goodbye) end to end, but they need live LLM keys and, for judged criteria, a local Ollama model. Add a small manual eval suite later, or keep to unit tests for now?

## Follow-ups

- **daily-python pin.** `pyproject.toml` pins `daily-python<0.37`. daily-python 0.37 changed `on_dialin_ready` to pass the whole event instead of the SIP URI string, and pipecat-ai 1.16's Daily transport forwards it unchanged to `pinlessCallUpdate`, which fails with `"sipUri" must be a string`. Remove the pin after upgrading to a pipecat-ai release that handles the new payload.

## Resolved

- Language: Pipecat requires Python, so the Rust workspace was replaced. Typing is enforced with pyright strict.
- Telephony: a Daily number via Pipecat Cloud's managed dial-in webhook. Unknown callers are rejected inside the bot, with no extra webhook service.
- Scaling: `min_agents = 0`, `max_agents = 3`, region `us-east`.
- LLM: Pipecat quickstart default (`OpenAIResponsesLLMService`, `gpt-4.1`).
- Deploys: the Pipecat GitHub integration deploys `main`; `main` is protected and requires the Checks workflow on pull requests.
- Krisp VIVA: enabled (`tel` filter, free for the first 10,000 minutes a month), applied only on Pipecat Cloud.
- Learner memory: Mem0 (hosted), one recall when the language is chosen and one save at session end, kept separate per practice language. The coach says what it remembers and compliments progress. Review and delete with `language_coach.memory_admin`.
- Transcript privacy: the image sets `PCC_WITHHOLD_CONTENT=true`, so Pipecat Cloud does not publish transcripts or tool arguments. This matches the old no-persistence rule.
