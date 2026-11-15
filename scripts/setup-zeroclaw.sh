#!/usr/bin/env bash
# Configure ZeroClaw to use ezRouter as its model provider.
#
# - Adds a `zerorouter.9router` provider profile pointing at the local ezRouter
#   OpenAI-compatible gateway (http://127.0.0.1:20229/v1, admin key 123456).
# - Routes the `main` agent to it with the default risk profile.
#
# Requires: zeroclaw on PATH, ezRouter running on port 20229.
# Idempotent: safe to re-run.
set -euo pipefail

command -v zeroclaw >/dev/null || { echo "error: zeroclaw not on PATH"; exit 1; }
CFG="$HOME/.zeroclaw/config.toml"

# We write api_key in plaintext, so disable secret encryption.
zeroclaw config set secrets.encrypt false >/dev/null 2>&1 || true

# --- Ensure provider profile: [providers.models.zerorouter.9router] ---
if ! grep -q 'providers.models.zerorouter.9router' "$CFG"; then
    cat >> "$CFG" <<'EOF'

[providers.models.zerorouter.9router]
uri = "http://127.0.0.1:20229/v1"
api_key = "123456"
model = "ag/gemini-4-argon"
EOF
    echo "==> added provider zerorouter.9router -> http://127.0.0.1:20229/v1"
else
    echo "==> provider zerorouter.9router already present, leaving as-is"
fi

# --- Ensure risk profile: [risk_profiles.default] ---
if ! grep -q '^\[risk_profiles.default\]' "$CFG"; then
    cat >> "$CFG" <<'EOF'

[risk_profiles.default]
EOF
    echo "==> added risk_profiles.default"
fi

# --- Ensure agent routes through it ---
zeroclaw agents create main >/dev/null 2>&1 || true   # idempotent; fails if exists
zeroclaw config set agents.main.model_provider "zerorouter.9router" >/dev/null 2>&1 || true
zeroclaw config set agents.main.risk_profile "default" >/dev/null 2>&1 || true

echo "==> ZeroClaw configured. Verify with: zeroclaw self-test"
echo "==> Note: ezRouter itself must have an upstream configured to actually answer requests."