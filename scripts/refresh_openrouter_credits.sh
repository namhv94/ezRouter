#!/usr/bin/env bash
# Refresh the OpenRouter credit balance shown in the dashboard.
#
# The dashboard reads `ui/public/assets/openrouter-credits.json`, which Vite
# copies verbatim into `ui/dist/assets/` on every build. Writing here (rather
# than straight into ui/dist) keeps the value across UI rebuilds.
#
# Override the defaults with AG_DATA_DIR / AG_UI_PUBLIC_DIR if you deploy
# somewhere else.
set -eu

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

DB_PATH="${AG_DATA_DIR:-$HOME/.ezrouter/data}/data.sqlite"
JSON_OUT="${AG_UI_PUBLIC_DIR:-${REPO_DIR}/ui/public}/assets/openrouter-credits.json"

if [ ! -f "$DB_PATH" ]; then
  exit 0
fi

# Query the OpenRouter API key from the providers table
KEY=$(sqlite3 "$DB_PATH" "SELECT api_key FROM providers WHERE (type='openrouter' OR prefix='openrouter') AND is_active=1 AND api_key != '' LIMIT 1;" 2>/dev/null || true)

if [ -z "$KEY" ]; then
  exit 0
fi

RESP=$(curl -s -m 10 -H "Authorization: Bearer $KEY" https://openrouter.ai/api/v1/credits 2>/dev/null || true)

if echo "$RESP" | grep -q '"total_credits"'; then
  DATA=$(echo "$RESP" | jq -c '.data + {updated_at: (now|floor)}')
  mkdir -p "$(dirname "$JSON_OUT")"
  echo "$DATA" > "$JSON_OUT"
fi
