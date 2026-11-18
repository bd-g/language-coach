# Production Setup

The coach runs as one Pipecat Cloud agent (`language-coach`, region `us-east`, scale to zero, at most 3 concurrent sessions, Krisp VIVA noise filtering). A Daily phone number on Pipecat Cloud's managed dial-in webhook starts a session for each call. The Pipecat GitHub integration deploys `main`.

Run every command below from `nix-shell` in the repository root.

## One-time setup

1. Log in:

   ```bash
   pipecat cloud auth login
   ```

2. Create the secret set. Copy `.env.example` to a local `.env`, fill in real keys and the allowlisted numbers, then:

   ```bash
   pipecat cloud secrets set language-coach-secrets --file .env --region us-east
   pipecat cloud secrets list --region us-east
   ```

   Memory needs `MEM0_API_KEY`: create a Mem0 account at https://app.mem0.ai and copy an API key from the dashboard. Every line in `.env` needs a value; comment out keys you don't use, or the command fails with "Empty key or value found". The secret set must exist in the agent's region (`us-east`) before `pipecat cloud deploy`, or the deploy fails with "Secret set 'language-coach-secrets' not found". Only the key for the TTS provider selected in `config/bot.toml` is required. `ALLOWED_CALLERS` is a comma-separated list of E.164 numbers; every other caller is disconnected as soon as the call connects.

3. Set a spend limit for the organization:

   ```bash
   pipecat cloud spend-limit set 20
   ```

4. Create the agent with a first cloud build:

   ```bash
   pipecat cloud deploy
   ```

5. Connect GitHub and link the agent to `main`, so merges deploy automatically. The agent must already exist from step 4 (otherwise the link returns 404 "Service not found"), and its name uses a hyphen:

   ```bash
   pipecat cloud github connect
   pipecat cloud project link language-coach --repo bd-g/language-coach --branch main
   ```

6. Protect `main` on GitHub (Settings → Rules → Rulesets, or Branches):
   - Require a pull request before merging.
   - Require the `python` status check from the Checks workflow to pass.
   - Do not allow bypassing, including for administrators, because every change to `main` is deployed.

7. Buy a phone number in the Pipecat Cloud dashboard under **Settings → Telephony**, then assign it to the `language-coach` agent. This uses the managed dial-in webhook, so there is no webhook server to host.

## Changing secrets

Re-run `pipecat cloud secrets set language-coach-secrets --file .env --region us-east`. A secret change alone does not create a new agent version. Run `pipecat cloud deploy --force` so running instances pick it up.

## Verification

- Browser session against the deployed agent, without the phone:

  ```bash
  pipecat cloud agent start language-coach --use-daily
  ```

  Open the printed Daily room URL and talk to the coach.

- Phone: call the Daily number from an allowlisted phone. Then call from a number that is not on the list and confirm the call disconnects right away.

- Logs:

  ```bash
  pipecat cloud agent logs language-coach
  ```

  Each session ends with a line such as `session ended after 412s; tts=cartesia tts_characters=...`. Rejected callers appear as `rejecting caller …1234: not allowlisted`.
