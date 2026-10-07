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
DEFAULT_KEY="ag-proxy-key"
DEFAULT_MODEL="ag/claude-sonnet-5-5-high"

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

echo -e "${YELLOW}⚙️ Đang cấu hình môi trường Claude Code...${NC}"
echo -e "   - Base URL : ${GREEN}${ROUTER_URL}${NC}"
echo -e "   - Model    : ${GREEN}${MODEL}${NC}"
echo -e "   - RC File  : ${GREEN}${RC_FILE}${NC}"

# Tạo block ezRouter trong rc file nếu chưa có
MARKER_START="# >>> ezRouter Claude Code Configuration >>>"
MARKER_END="# <<< ezRouter Claude Code Configuration <<<"

if grep -q "$MARKER_START" "$RC_FILE" 2>/dev/null; then
    # Xóa block cũ
    sed -i "/$MARKER_START/,/$MARKER_END/d" "$RC_FILE"
fi

cat <<EOF >> "$RC_FILE"
$MARKER_START
export ANTHROPIC_BASE_URL="${ROUTER_URL}"
export ANTHROPIC_API_KEY="${API_KEY}"
export ANTHROPIC_MODEL="${MODEL}"
$MARKER_END
EOF

echo ""
echo -e "${GREEN}✅ Tích hợp hoàn tất thành công!${NC}"
echo -e "💡 Đã ghi cấu hình vào: ${RC_FILE}"
echo ""
echo -e "${CYAN}👉 Cách kích hoạt và sử dụng ngay lập tức:${NC}"
echo -e "   source ${RC_FILE}"
echo -e "   claude"
echo ""
