#!/usr/bin/env bash
set -euo pipefail

REPO="namhv94/ezRouter"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

echo "==> Installing ezRouter from ${REPO}..."

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

if [ "$OS" != "linux" ]; then
    echo "Error: ezRouter automated installer currently supports Linux (x86_64)."
    echo "For macOS or other platforms, please build from source: cargo build --release"
    exit 1
fi

if [ "$ARCH" != "x86_64" ]; then
    echo "Error: Automated binary download currently targets x86_64. Found: $ARCH."
    echo "Please build from source: cargo build --release"
    exit 1
fi

LATEST_RELEASE=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
    echo "Error: Could not retrieve latest release tag."
    exit 1
fi

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_RELEASE}/ezrouter-${LATEST_RELEASE}-linux-x86_64.tar.gz"

echo "==> Downloading ezRouter ${LATEST_RELEASE}..."
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

curl -fsSL "$DOWNLOAD_URL" -o "${TMP_DIR}/ezrouter.tar.gz"
tar -xzf "${TMP_DIR}/ezrouter.tar.gz" -C "$TMP_DIR"

if [ ! -w "$INSTALL_DIR" ]; then
    echo "==> Need sudo privileges to copy binary to ${INSTALL_DIR}"
    sudo mv "${TMP_DIR}/ezrouter" "${INSTALL_DIR}/ezrouter"
    sudo chmod +x "${INSTALL_DIR}/ezrouter"
else
    mv "${TMP_DIR}/ezrouter" "${INSTALL_DIR}/ezrouter"
    chmod +x "${INSTALL_DIR}/ezrouter"
fi

echo "==> ezRouter installed successfully to ${INSTALL_DIR}/ezrouter"
echo "==> Run 'ezrouter --help' or start with 'ezrouter'"
