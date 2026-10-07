#!/usr/bin/env bash
# ==============================================================================
# ezRouter - Script Cấu hình Tự động Tích hợp OpenAI Codex IDE & CLI
# Hỗ trợ: macOS & Linux
# Chạy 1 bước: curl -fsSL https://router.namhv.vip/setup-codex.sh | bash
# ==============================================================================

set -euo pipefail

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${CYAN}======================================================${NC}"
echo -e "${CYAN}     ⚡ ezRouter - Tích Hợp OpenAI Codex (1 Bước)      ${NC}"
echo -e "${CYAN}======================================================${NC}"
echo ""

# Nhận tham số hoặc giá trị mặc định
DEFAULT_URL="https://router.namhv.vip/v1"
DEFAULT_KEY="ag-proxy-key"
DEFAULT_MODEL="cx/gpt-5.6-sol"

ROUTER_URL="${1:-$DEFAULT_URL}"
API_KEY="${2:-$DEFAULT_KEY}"
MODEL="${3:-$DEFAULT_MODEL}"

# Nếu chạy interactive (có TTY và không truyền tham số qua pipe)
if [ -t 0 ] && [ "$#" -eq 0 ]; then
    read -r -p "Nhập ezRouter Base URL [Mặc định: ${DEFAULT_URL}]: " USER_URL
    ROUTER_URL="${USER_URL:-$DEFAULT_URL}"

    read -r -p "Nhập ezRouter API Key [Mặc định: ${DEFAULT_KEY}]: " USER_KEY
    API_KEY="${USER_KEY:-$DEFAULT_KEY}"

    read -r -p "Nhập Model mặc định [Mặc định: ${DEFAULT_MODEL}]: " USER_MODEL
    MODEL="${USER_MODEL:-$DEFAULT_MODEL}"
    echo ""
fi

# Loại bỏ trailing slash nếu có
ROUTER_URL="${ROUTER_URL%/}"

CODEX_DIR="$HOME/.codex"
mkdir -p "$CODEX_DIR"
CONFIG_FILE="$CODEX_DIR/config.toml"

echo -e "${YELLOW}⚙️ Đang ghi cấu hình Codex...${NC}"
echo -e "   - Base URL : ${GREEN}${ROUTER_URL}${NC}"
echo -e "   - Model    : ${GREEN}${MODEL}${NC}"
echo -e "   - Config   : ${GREEN}${CONFIG_FILE}${NC}"

# Backup config cũ nếu có
if [ -f "$CONFIG_FILE" ]; then
    BACKUP_PATH="${CONFIG_FILE}.bak.$(date +%s)"
    cp "$CONFIG_FILE" "$BACKUP_PATH"
    echo -e "   - Đã sao lưu config cũ: ${BACKUP_PATH}"
fi

cat <<EOF > "$CONFIG_FILE"
# ==============================================================================
# Cấu hình ezRouter cho OpenAI Codex IDE & Codex CLI
# Tự động tạo bởi ezRouter One-Step Installer
# ==============================================================================

model = "${MODEL}"
model_provider = "ezrouter"

[model_providers.ezrouter]
name = "ezRouter"
base_url = "${ROUTER_URL}"
wire_api = "responses"
supports_websockets = false
experimental_bearer_token = "${API_KEY}"
requires_openai_auth = false
request_max_retries = 4
stream_max_retries = 10
stream_idle_timeout_ms = 300000
EOF

echo ""
echo -e "${GREEN}✅ Tích hợp hoàn tất thành công!${NC}"
echo -e "💡 Codex IDE Extension (VS Code / Cursor) và Codex CLI đã sẵn sàng sử dụng."
echo ""
echo -e "${CYAN}👉 Cách kiểm tra hoạt động:${NC}"
echo -e "   codex -m ${MODEL} \"Xin chào! Bạn là ai?\""
echo ""
