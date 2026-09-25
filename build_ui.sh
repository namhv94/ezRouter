#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/ui"

if [ ! -d "node_modules" ]; then
    echo "Installing UI dependencies..."
    npm ci || npm install
fi

echo "Building UI for production..."
npm run build

echo "UI build complete at ${SCRIPT_DIR}/ui/dist"
