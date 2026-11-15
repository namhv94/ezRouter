#!/usr/bin/env bash
# Install ezRouter as a global `ezrouter` command (Termux/Android).
# Builds if no release binary exists, installs to $PREFIX/bin, and creates
# a runtime env file at ~/.ezrouter/env.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BIN_SRC="${REPO_DIR}/target/release/ezrouter"

if [ "$(uname -o)" != "Android" ] || [ -z "${PREFIX:-}" ]; then
    echo "Error: This script targets Termux (\$PREFIX required)."
    exit 1
fi

# 1. Build if needed
if [ ! -x "$BIN_SRC" ]; then
    echo "==> No release binary found, building (cargo build --release)..."
    (cd "$REPO_DIR" && cargo build --release)
fi

# 2. Install binary to $PREFIX/lib/ezrouter
mkdir -p "$PREFIX/lib"
cp "$BIN_SRC" "$PREFIX/lib/ezrouter"
chmod +x "$PREFIX/lib/ezrouter"

# 3. Launcher script in $PREFIX/bin
cat > "$PREFIX/bin/ezrouter" <<'LAUNCHER'
#!/data/data/com.termux/files/usr/bin/sh
# ezRouter launcher - loads ~/.ezrouter/env (if present) then runs the binary.

EZROUTER_ENV="${EZROUTER_ENV:-$HOME/.ezrouter/env}"
if [ -f "$EZROUTER_ENV" ]; then
    . "$EZROUTER_ENV"
fi

# Termux-safe defaults
AG_HOST="${AG_HOST:-0.0.0.0}"
AG_PORT="${AG_PORT:-20229}"
AG_DATA_DIR="${AG_DATA_DIR:-$HOME/.ezrouter/data}"
export AG_HOST AG_PORT AG_DATA_DIR

exec "$PREFIX/lib/ezrouter" "$@"
LAUNCHER
chmod +x "$PREFIX/bin/ezrouter"

# 4. Create runtime env file if missing
mkdir -p "$HOME/.ezrouter/data"
if [ ! -f "$HOME/.ezrouter/env" ]; then
    cat > "$HOME/.ezrouter/env" <<'EOF'
# ezRouter runtime environment (loaded by the ezrouter launcher)
AG_HOST=0.0.0.0
AG_PORT=20229
AG_DATA_DIR=/data/data/com.termux/files/home/.ezrouter/data
RUST_LOG=ezrouter=info,tower_http=info
EOF
    echo "==> Created $HOME/.ezrouter/env (edit it to change port, data dir, API keys, etc.)"
fi

echo "==> Installed: $PREFIX/bin/ezrouter"
echo "==> Run: ezrouter   (from any directory)"
