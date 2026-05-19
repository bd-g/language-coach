# Production Setup

This is the short Fly.io and Telnyx runbook for the in-between server. It follows the implementation plan: one always-on Fly machine, public Telnyx webhooks/media WebSockets on port `8080`, internal Fly metrics on port `9091`, and production config mounted from a Fly secret.

## Prerequisites

- Install `flyctl` and log in with `fly auth login`.
- Have a Telnyx account with a purchased US local DID.
- Have a Deepgram API key.
- Choose the Fly app name. The checked-in default is `language-coach`; override commands with `FLY_APP=<name>` if needed.

## Production Config

Create `config/production.toml`. Do not commit it.

```toml
public_base_url = "https://language-coach.brentdgeorge.com"
telnyx_public_key = "<Telnyx Ed25519 public key>"
allowlist_numbers = ["+15551234567"]
```

The rest of the server settings use validated defaults or can be copied from `config/server.example.toml` when you need overrides.

## Fly.io Setup

Create the app and certificate once:

```bash
fly apps create language-coach
fly certs add language-coach.brentdgeorge.com -a language-coach
```

Set DNS for the custom domain:

```text
Type:  CNAME
Name:  language-coach
Value: language-coach.fly.dev.
TTL:   300
```

Set runtime secrets once:

```bash
fly secrets set \
  TELNYX_API_KEY="<telnyx-api-key>" \
  DEEPGRAM_API_KEY="<deepgram-api-key>" \
  DIRECT_ACCESS_TOKEN="$(openssl rand -hex 32)" \
  -a language-coach
```

Deploy, including the mounted production config secret:

```bash
scripts/deploy-fly.sh
```

For a non-default app or health URL:

```bash
FLY_APP=my-app FLY_HEALTH_URL=https://language-coach.brentdgeorge.com scripts/deploy-fly.sh
```

## Telnyx Setup

In Telnyx Mission Control:

- Create a Call Control application.
- Set the webhook URL to `https://language-coach.brentdgeorge.com/telnyx/webhook`.
- Use Webhook API Version `V2 EdDSA signatures`.
- Copy the Ed25519 public key into `telnyx_public_key` in `config/production.toml`.
- Assign the purchased DID to the Call Control application.
- Enable inbound call screening for spam and robocall rejection.
- Add the caller phone numbers allowed to use the coach to `allowlist_numbers` in E.164 format.

After changing `config/production.toml`, rerun `scripts/deploy-fly.sh` so Fly receives the updated `SERVER_CONFIG_TOML` secret and redeploys the server.

## Automated Main-Branch Deploys

The GitHub workflow deploys to Fly after checks pass on `main`. Add this GitHub Actions secret:

```text
FLY_API_TOKEN
```

Create it with:

```bash
fly tokens create deploy -a language-coach
```

The workflow assumes the Fly app already has `TELNYX_API_KEY`, `DEEPGRAM_API_KEY`, `DIRECT_ACCESS_TOKEN`, and `SERVER_CONFIG_TOML` set. Use `scripts/deploy-fly.sh` locally whenever you rotate secrets or change `config/production.toml`.

## Verification

Check deployment health:

```bash
curl --fail https://language-coach.brentdgeorge.com/healthz
curl --fail https://language-coach.brentdgeorge.com/readyz
```

Check the deployed debug path without Telnyx:

```bash
export DIRECT_ACCESS_TOKEN="<deployed-direct-access-token>"
cargo run -p language-coach-debug-client -- \
  --config debug-client.local.toml \
  --url wss://language-coach.brentdgeorge.com/dev/connect
```
