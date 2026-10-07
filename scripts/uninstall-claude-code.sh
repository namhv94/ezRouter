#!/usr/bin/env bash
# ==============================================================================
# ezRouter - Script Gỡ Bỏ Cấu Hình Claude Code CLI
# Hỗ trợ: macOS & Linux
# Chạy 1 bước: curl -fsSL https://router.namhv.vip/uninstall-claude-code.sh | bash
# ==============================================================================

set -euo pipefail

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${CYAN}======================================================${NC}"
echo -e "${CYAN}     ⚡ ezRouter - Gỡ Bỏ Cấu Hình Claude Code          ${NC}"
echo -e "${CYAN}======================================================${NC}"
echo ""

MARKER_START="# >>> ezRouter Claude Code Configuration >>>"
MARKER_END="# <<< ezRouter Claude Code Configuration <<<"

FOUND=0
for RC_FILE in "$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.profile"; do
    if [ -f "$RC_FILE" ] && grep -q "$MARKER_START" "$RC_FILE" 2>/dev/null; then
        sed -i "/$MARKER_START/,/$MARKER_END/d" "$RC_FILE"
        echo -e "${YELLOW}🗑️ Đã xóa cấu hình ezRouter khỏi: ${RC_FILE}${NC}"
        FOUND=1
    fi
done

SETTINGS_FILE="$HOME/.claude/settings.json"
if [ -f "$SETTINGS_FILE" ]; then
    if command -v node >/dev/null 2>&1; then
        node -e "
const fs = require('fs');
const path = '$SETTINGS_FILE';
try {
    let settings = JSON.parse(fs.readFileSync(path, 'utf8'));
    if (settings.env) {
        delete settings.env.ANTHROPIC_BASE_URL;
        delete settings.env.ANTHROPIC_AUTH_TOKEN;
        delete settings.env.ANTHROPIC_API_KEY;
        delete settings.env.ANTHROPIC_MODEL;
        delete settings.env.ANTHROPIC_SMALL_FAST_MODEL;
        delete settings.env.CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT;
        fs.writeFileSync(path, JSON.stringify(settings, null, 2));
    }
} catch {}
"
        echo -e "${YELLOW}🗑️ Đã dọn dẹp các biến ezRouter khỏi: ${SETTINGS_FILE}${NC}"
    fi
fi

if [ "$FOUND" -eq 0 ]; then
    echo -e "${YELLOW}ℹ️ Không tìm thấy cấu hình ezRouter trong shell rc files.${NC}"
fi

echo ""
echo -e "${GREEN}🎉 Gỡ bỏ cấu hình hoàn tất!${NC}"
echo -e "💡 Hãy chạy: ${CYAN}unset ANTHROPIC_BASE_URL ANTHROPIC_AUTH_TOKEN ANTHROPIC_API_KEY ANTHROPIC_MODEL ANTHROPIC_SMALL_FAST_MODEL CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT${NC} để khôi phục terminal hiện tại về mặc định của Anthropic."
echo ""
