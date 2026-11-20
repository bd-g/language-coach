# TTS Providers

Exactly one provider is active, set by the `[tts]` block in `config/bot.toml`: Deepgram Flux TTS or Cartesia. Each provider needs only its own API key in the environment. Speaking-rate changes are mapped onto each provider's supported speed range.

| Provider | `[tts]` block | Key | List price (checked 2026-11-20) | Speed range |
|---|---|---|---|---|
| Deepgram Flux TTS (`/v2/speak`) | `provider = "deepgram"`, `voice = "flux-ernesto-es"`, `expressivity = 0` | `DEEPGRAM_API_KEY` | $0.045 / 1k chars pay-as-you-go | 0.5–1.5 in steps of 0.05 |
| Cartesia Sonic 3.6 | `provider = "cartesia"`, `model`, `voice`, `language` | `CARTESIA_API_KEY` | ~1 credit/char. Free plan has 20k credits/month; Pro is $5 for 100k; overage $65 per 1M on Pro | 0.6–1.5 |

Prices change often; check each provider's pricing page before relying on these numbers.

Deepgram Flux reuses the Deepgram key the bot already needs for speech-to-text. Its Spanish voices also speak English, and it can go down to half speed, so beginners get the full 0.5 there; on Cartesia beginners get 0.6. `expressivity` runs from -2 (calm) to 2 (animated). Cartesia switches its locale between `en-US` and `es-MX` to match the practice language. See https://developers.deepgram.com/docs/flux-tts/feature-overview.

## Comparing providers

Each session logs a usage line when it ends:

```text
session ended after 412s; tts=cartesia tts_characters=5123 llm_prompt_tokens=... llm_completion_tokens=...
```

Multiply `tts_characters` by the provider's per-character price to compare cost per call. To try a provider locally, edit `[tts]`, add its key to `.env`, and restart `python bot.py`. To try it on the phone, merge the config change through a pull request.

What to listen for: natural Mexican Spanish in coaching turns, clear native-sounding English in role-play turns, and whether switching between them inside one reply sounds smooth.
