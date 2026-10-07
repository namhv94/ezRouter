#!/usr/bin/env bash
# ==============================================================================
# ezRouter - Script Cấu hình Tự động Tích hợp Claude Code CLI
# Hỗ trợ: macOS & Linux
# Chạy 1 bước: curl -fsSL https://router.namhv.vip/setup-claude-code.sh | bash
# ==============================================================================

set -euo pipefail

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${CYAN}======================================================${NC}"
echo -e "${CYAN}     ⚡ ezRouter - Tích Hợp Claude Code CLI (1 Bước)    ${NC}"
echo -e "${CYAN}======================================================${NC}"
echo ""

DEFAULT_URL="https://router.namhv.vip"
DEFAULT_KEY="namhv94"
DEFAULT_MODEL="cx/gpt-6.1-sol"

ROUTER_URL="${1:-$DEFAULT_URL}"
API_KEY="${2:-$DEFAULT_KEY}"
MODEL="${3:-$DEFAULT_MODEL}"

# Loại bỏ trailing slash và /v1 nếu có (Anthropic SDK tự thêm /v1/messages)
ROUTER_URL="${ROUTER_URL%/}"
ROUTER_URL="${ROUTER_URL%/v1}"

# Xác định shell profile file
RC_FILE=""
if [ -n "${ZSH_VERSION:-}" ] || [ -f "$HOME/.zshrc" ]; then
    RC_FILE="$HOME/.zshrc"
elif [ -f "$HOME/.bashrc" ]; then
    RC_FILE="$HOME/.bashrc"
else
    RC_FILE="$HOME/.profile"
fi

CLAUDE_DIR="$HOME/.claude"
mkdir -p "$CLAUDE_DIR"
SETTINGS_FILE="$CLAUDE_DIR/settings.json"

echo -e "${YELLOW}⚙️ Đang cấu hình môi trường Claude Code...${NC}"
echo -e "   - Base URL : ${GREEN}${ROUTER_URL}${NC}"
echo -e "   - Model    : ${GREEN}${MODEL}${NC}"
echo -e "   - RC File  : ${GREEN}${RC_FILE}${NC}"
echo -e "   - Settings : ${GREEN}${SETTINGS_FILE}${NC}"

# 1. Cập nhật settings.json của Claude Code CLI (Claude Code ưu tiên file này hơn shell env)
if [ -f "$SETTINGS_FILE" ]; then
    cp "$SETTINGS_FILE" "${SETTINGS_FILE}.bak.$(date +%s)" 2>/dev/null || true
fi

if command -v node >/dev/null 2>&1; then
    node -e "
const fs = require('fs');
const path = '$SETTINGS_FILE';
let settings = {};
try { settings = JSON.parse(fs.readFileSync(path, 'utf8')); } catch {}
settings.env = settings.env || {};
settings.env.ANTHROPIC_BASE_URL = '$ROUTER_URL';
settings.env.ANTHROPIC_AUTH_TOKEN = '$API_KEY';
settings.env.ANTHROPIC_API_KEY = '$API_KEY';
settings.env.ANTHROPIC_MODEL = '$MODEL';
settings.env.ANTHROPIC_SMALL_FAST_MODEL = '$MODEL';
settings.env.CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT = '1';
fs.writeFileSync(path, JSON.stringify(settings, null, 2));
"
elif command -v python3 >/dev/null 2>&1; then
    python3 -c "
import json, os
path = os.path.expanduser('$SETTINGS_FILE')
try:
    with open(path, 'r') as f:
        data = json.load(f)
except Exception:
    data = {}
env = data.setdefault('env', {})
env['ANTHROPIC_BASE_URL'] = '$ROUTER_URL'
env['ANTHROPIC_AUTH_TOKEN'] = '$API_KEY'
env['ANTHROPIC_API_KEY'] = '$API_KEY'
env['ANTHROPIC_MODEL'] = '$MODEL'
env['ANTHROPIC_SMALL_FAST_MODEL'] = '$MODEL'
env['CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT'] = '1'
with open(path, 'w') as f:
    json.dump(data, f, indent=2)
"
else
    cat <<EOF > "$SETTINGS_FILE"
{
  "env": {
    "ANTHROPIC_BASE_URL": "${ROUTER_URL}",
    "ANTHROPIC_AUTH_TOKEN": "${API_KEY}",
    "ANTHROPIC_API_KEY": "${API_KEY}",
    "ANTHROPIC_MODEL": "${MODEL}",
    "ANTHROPIC_SMALL_FAST_MODEL": "${MODEL}",
    "CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT": "1"
  }
}
EOF
fi

# 2. Tạo block ezRouter trong rc file
MARKER_START="# >>> ezRouter Claude Code Configuration >>>"
MARKER_END="# <<< ezRouter Claude Code Configuration <<<"

if grep -q "$MARKER_START" "$RC_FILE" 2>/dev/null; then
    sed -i "/$MARKER_START/,/$MARKER_END/d" "$RC_FILE"
fi

cat <<EOF >> "$RC_FILE"
$MARKER_START
export ANTHROPIC_BASE_URL="${ROUTER_URL}"
export ANTHROPIC_AUTH_TOKEN="${API_KEY}"
export ANTHROPIC_API_KEY="${API_KEY}"
export ANTHROPIC_MODEL="${MODEL}"
export ANTHROPIC_SMALL_FAST_MODEL="${MODEL}"
export CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT="1"
$MARKER_END
EOF

echo ""
echo -e "${GREEN}✅ Tích hợp hoàn tất thành công!${NC}"
echo -e "💡 Đã ghi cấu hình vào: ${SETTINGS_FILE} và ${RC_FILE}"
echo ""
echo -e "${CYAN}👉 Cách kích hoạt và sử dụng ngay lập tức:${NC}"
echo -e "   source ${RC_FILE}"
echo -e "   claude"
echo ""
