#!/usr/bin/env bash
set -euo pipefail

APP="${FLY_APP:-language-coach}"
CONFIG="${SERVER_CONFIG:-config/production.toml}"
HEALTH_URL="${FLY_HEALTH_URL:-https://${APP}.fly.dev}"

if ! command -v fly >/dev/null 2>&1; then
  printf 'fly CLI is required. Install it from https://fly.io/docs/flyctl/install/\n' >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  printf 'curl is required for post-deploy health checks.\n' >&2
  exit 1
fi

if [[ ! -f "${CONFIG}" ]]; then
  printf 'Missing production config: %s\n' "${CONFIG}" >&2
  exit 1
fi

base64_file() {
  if base64 --help 2>&1 | grep -q -- '-w'; then
    base64 -w0 "$1"
  else
    base64 <"$1" | tr -d '\n'
  fi
}

printf 'Updating SERVER_CONFIG_TOML for Fly app %s from %s\n' "${APP}" "${CONFIG}"
fly secrets set SERVER_CONFIG_TOML="$(base64_file "${CONFIG}")" -a "${APP}"

printf 'Deploying Fly app %s\n' "${APP}"
fly deploy -a "${APP}"

printf 'Checking %s\n' "${HEALTH_URL}"
curl --fail --silent --show-error "${HEALTH_URL}/healthz" >/dev/null
curl --fail --silent --show-error "${HEALTH_URL}/readyz" >/dev/null
printf 'Deploy complete.\n'
