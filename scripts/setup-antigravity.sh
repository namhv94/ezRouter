#!/usr/bin/env bash
# ==============================================================================
# ezRouter - Script Cấu hình Tích hợp Antigravity IDE
# Hỗ trợ: Linux & macOS
# ==============================================================================

set -euo pipefail

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${CYAN}======================================================${NC}"
echo -e "${CYAN}    ⚡ ezRouter - Hướng Dẫn Tích Hợp Antigravity IDE   ${NC}"
echo -e "${CYAN}======================================================${NC}"
echo ""

# 1. Nhập thông tin Router
DEFAULT_URL="http://127.0.0.1:20229/v1"
DEFAULT_KEY="ag-proxy-key"

read -p "Nhập ezRouter Base URL [Mặc định: ${DEFAULT_URL}]: " USER_URL
ROUTER_URL="${USER_URL:-$DEFAULT_URL}"

read -p "Nhập ezRouter API Key [Mặc định: ${DEFAULT_KEY}]: " USER_KEY
API_KEY="${USER_KEY:-$DEFAULT_KEY}"

echo ""
echo -e "${YELLOW}Chọn phương thức tích hợp:${NC}"
echo -e "  [1] ${GREEN}Cách 1 (Khuyên dùng):${NC} Tự động cấu hình Extension Continue trong Antigravity IDE"
echo -e "  [2] ${GREEN}Cách 2 (Nâng cao):${NC} Cấu hình Proxy / Native Settings cho Antigravity IDE"
echo -e "  [3] ${GREEN}Kiểm tra kết nối:${NC} Test gọi API tới ezRouter"
echo -e "  [0] Thoát"
echo ""

read -p "Vui lòng chọn [1/2/3/0]: " CHOICE

case "$CHOICE" in
  1)
    echo ""
    echo -e "${CYAN}==> [Cách 1] Đang cấu hình Extension Continue cho Antigravity IDE...${NC}"
    
    CONTINUE_DIR="$HOME/.continue"
    mkdir -p "$CONTINUE_DIR"
    CONFIG_FILE="$CONTINUE_DIR/config.json"

    # Backup nếu đã có
    if [ -f "$CONFIG_FILE" ]; then
        cp "$CONFIG_FILE" "$CONFIG_FILE.bak.$(date +%s)"
        echo -e "Đã backup file config cũ sang: ${CONFIG_FILE}.bak.*"
    fi

    cat <<EOF > "$CONFIG_FILE"
{
  "models": [
    {
      "title": "Gemini 3.8 Flash High (ezRouter)",
      "provider": "openai",
      "model": "ag/gemini-3.8-flash-high",
      "apiBase": "${ROUTER_URL}",
      "apiKey": "${API_KEY}"
    },
    {
      "title": "Claude Opus 5.5 (ezRouter)",
      "provider": "openai",
      "model": "claude-opus-5.5",
      "apiBase": "${ROUTER_URL}",
      "apiKey": "${API_KEY}"
    },
    {
      "title": "GPT-5 Codex (ezRouter)",
      "provider": "openai",
      "model": "cx/gpt-5",
      "apiBase": "${ROUTER_URL}",
      "apiKey": "${API_KEY}"
    },
    {
      "title": "Gemini 3.0 Pro (ezRouter)",
      "provider": "openai",
      "model": "ag/gemini-3.0-pro",
      "apiBase": "${ROUTER_URL}",
      "apiKey": "${API_KEY}"
    }
  ],
  "tabAutocompleteModel": {
    "title": "Gemini 2.5 Flash Autocomplete",
    "provider": "openai",
    "model": "ag/gemini-2.5-flash",
    "apiBase": "${ROUTER_URL}",
    "apiKey": "${API_KEY}"
  },
  "embeddingsProvider": {
    "provider": "openai",
    "model": "text-embedding-3-small",
    "apiBase": "${ROUTER_URL}",
    "apiKey": "${API_KEY}"
  },
  "customCommands": [
    {
      "name": "review",
      "prompt": "Hãy review đoạn code được chọn, chỉ ra các lỗi tiềm ẩn và cách tối ưu ngắn gọn:",
      "description": "Review code"
    }
  ],
  "allowAnonymousTelemetry": false
}
EOF

    echo -e "${GREEN}✓ Đã tạo file cấu hình thành công tại: ${CONFIG_FILE}${NC}"
    echo ""
    echo -e "${YELLOW}👉 Các bước tiếp theo trong Antigravity IDE:${NC}"
    echo -e "  1. Mở Antigravity IDE."
    echo -e "  2. Bấm tổ hợp phím ${CYAN}Ctrl+Shift+X${NC} (hoặc Cmd+Shift+X trên Mac) để mở Extensions."
    echo -e "  3. Tìm kiếm ${CYAN}Continue${NC} và bấm ${GREEN}Install${NC} (Continue - Codestral, Claude, and more)."
    echo -e "  4. Mở sidebar Continue (Ctrl+L hoặc Cmd+L), chọn model ${GREEN}Gemini 3.8 Flash High (ezRouter)${NC} và bắt đầu code!"
    ;;

  2)
    echo ""
    echo -e "${CYAN}==> [Cách 2] Hướng dẫn cấu hình Native / Proxy cho Antigravity IDE...${NC}"
    echo ""
    echo -e "Antigravity IDE là bản fork của VS Code. Để điều hướng request AI:"
    echo ""
    echo -e "${YELLOW}Bước 1: Cấu hình Proxy trong Antigravity IDE settings${NC}"
    
    # Tìm thư mục settings của Antigravity IDE
    OS="$(uname -s)"
    if [ "$OS" = "Darwin" ]; then
        AG_SETTINGS_DIR="$HOME/Library/Application Support/Antigravity/User"
    else
        AG_SETTINGS_DIR="$HOME/.config/Antigravity/User"
    fi

    echo -e "Thư mục cấu hình IDE dự kiến: ${CYAN}${AG_SETTINGS_DIR}${NC}"
    mkdir -p "$AG_SETTINGS_DIR"
    SETTINGS_FILE="$AG_SETTINGS_DIR/settings.json"

    echo ""
    echo -e "Thêm các dòng sau vào file ${CYAN}${SETTINGS_FILE}${NC}:"
    cat <<EOF
{
  "http.proxy": "http://127.0.0.1:20229",
  "http.proxyStrictSSL": false
}
EOF
    echo ""
    echo -e "${YELLOW}Bước 2: Cấu hình Environment khi khởi chạy Antigravity IDE${NC}"
    echo -e "Chạy lệnh sau trong terminal trước khi mở IDE:"
    echo -e "${CYAN}export HTTP_PROXY=\"http://127.0.0.1:20229\"${NC}"
    echo -e "${CYAN}export HTTPS_PROXY=\"http://127.0.0.1:20229\"${NC}"
    echo -e "${CYAN}export NODE_TLS_REJECT_UNAUTHORIZED=\"0\"${NC}"
    ;;

  3)
    echo ""
    echo -e "${CYAN}==> [Kiểm tra kết nối] Đang gửi request test tới ${ROUTER_URL}...${NC}"
    HEALTH_URL="${ROUTER_URL%/v1}/health"
    
    echo -e "Checking health: ${HEALTH_URL}"
    if curl -s -f "$HEALTH_URL" > /dev/null; then
        echo -e "${GREEN}✓ ezRouter đang hoạt động bình thường!${NC}"
        curl -s "$HEALTH_URL" | jq . || curl -s "$HEALTH_URL"
    else
        echo -e "${RED}✗ Không kết nối được tới ezRouter tại ${HEALTH_URL}${NC}"
        echo -e "Vui lòng kiểm tra lại dịch vụ ezRouter đã bật chưa (port 20229)."
        exit 1
    fi

    echo ""
    echo -e "Gửi thử 1 request chat completion test:"
    curl -s -X POST "${ROUTER_URL}/chat/completions" \
      -H "Authorization: Bearer ${API_KEY}" \
      -H "Content-Type: application/json" \
      -d '{
        "model": "ag/gemini-3.8-flash-high",
        "messages": [{"role": "user", "content": "Ping"}],
        "max_tokens": 10
      }' | jq . || true
    echo ""
    echo -e "${GREEN}✓ Hoàn tất kiểm tra!${NC}"
    ;;

  0)
    echo "Tạm biệt!"
    exit 0
    ;;

  *)
    echo -e "${RED}Lựa chọn không hợp lệ.${NC}"
    exit 1
    ;;
esac

echo ""
echo -e "${GREEN}Chúc anh em vibe code vui vẻ cùng ezRouter & Antigravity IDE! 🚀${NC}"
