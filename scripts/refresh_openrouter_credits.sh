#!/usr/bin/env bash
set -e

DB_PATH="/home/namhv/.ag-proxy-rust-staging/data.sqlite"
JSON_OUT="/home/namhv/ag-proxy-rust/ui/dist/assets/openrouter-credits.json"

if [ ! -f "$DB_PATH" ]; then
  exit 0
fi

# Query OpenRouter API key from staging database
KEY=$(sqlite3 "$DB_PATH" "SELECT api_key FROM providers WHERE (type='openrouter' OR prefix='openrouter') AND is_active=1 LIMIT 1;" 2>/dev/null || true)

if [ -z "$KEY" ]; then
  exit 0
fi

# Fetch credits from OpenRouter API
RESP=$(curl -s -m 10 -H "Authorization: Bearer $KEY" https://openrouter.ai/api/v1/credits 2>/dev/null || true)

if echo "$RESP" | grep -q '"total_credits"'; then
  DATA=$(echo "$RESP" | jq -c '.data + {updated_at: (now|floor)}')
  mkdir -p "$(dirname "$JSON_OUT")"
  echo "$DATA" > "$JSON_OUT"

  # Also save to SQLite quota_refresh_settings for backend consistency
  ESCAPED_DATA=$(echo "$DATA" | sed "s/'/''/g")
  sqlite3 "$DB_PATH" "INSERT OR REPLACE INTO quota_refresh_settings (key, value, updated_at) VALUES ('openrouter_credits', '$ESCAPED_DATA', datetime('now'));" 2>/dev/null || true
fi
