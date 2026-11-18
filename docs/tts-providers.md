# TTS Providers

Exactly one provider is active, set by the `[tts]` block in `config/bot.toml`. Each provider needs only its own API key in the environment. Speaking-rate changes are mapped onto each provider's supported speed range.

| Provider | `[tts]` block | Key | List price (checked 2026-11-18) | Speed range |
|---|---|---|---|---|
| Deepgram Aura-2 | `provider = "deepgram"`, `voice = "aura-2-selena-es"` | `DEEPGRAM_API_KEY` | $0.030 / 1k chars pay-as-you-go | 0.7–1.5 (Deepgram recommends ≥ 0.9 for Spanish) |
| Cartesia Sonic 3.6 | `provider = "cartesia"`, `model`, `voice`, `language` | `CARTESIA_API_KEY` | ~1 credit/char. Free plan has 20k credits/month; Pro is $5 for 100k; overage $65 per 1M on Pro | 0.6–1.5 |
| ElevenLabs Flash v2.5 | `provider = "elevenlabs"`, `model = "eleven_flash_v2_5"`, `voice` | `ELEVENLABS_API_KEY` | $0.04 / 1k chars | 0.7–1.2 |

Prices change often; check each provider's pricing page before relying on these numbers.

At list prices, Deepgram Aura-2 is the cheapest option. It also reuses the Deepgram key the bot already needs for Flux speech-to-text, and its Spanish voices code-switch with English. Cartesia's free tier (about 27 minutes of audio a month) is enough to evaluate it.

## Comparing providers

Each session logs a usage line when it ends:

```text
session ended after 412s; tts=cartesia tts_characters=5123 llm_prompt_tokens=... llm_completion_tokens=...
```

Multiply `tts_characters` by the provider's per-character price to compare cost per call. To try a provider locally, edit `[tts]`, add its key to `.env`, and restart `python bot.py`. To try it on the phone, merge the config change through a pull request.

What to listen for: natural Mexican Spanish in coaching turns, clear native-sounding English in role-play turns, and whether switching between them inside one reply sounds smooth.
