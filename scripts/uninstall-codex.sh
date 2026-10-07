#!/usr/bin/env bash
# ==============================================================================
# ezRouter - Script Gỡ Bỏ Cấu Hình OpenAI Codex IDE & CLI
# Hỗ trợ: macOS & Linux
# Chạy 1 bước: curl -fsSL https://router.namhv.vip/uninstall-codex.sh | bash
# ==============================================================================

set -euo pipefail

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${CYAN}======================================================${NC}"
echo -e "${CYAN}     ⚡ ezRouter - Gỡ Bỏ Cấu Hình Codex (Uninstall)     ${NC}"
echo -e "${CYAN}======================================================${NC}"
echo ""

CODEX_DIR="$HOME/.codex"
CONFIG_FILE="$CODEX_DIR/config.toml"

if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${YELLOW}ℹ️ Không tìm thấy file cấu hình tại: ${CONFIG_FILE}${NC}"
    echo -e "Codex hiện đang ở trạng thái mặc định (chưa cấu hình ezRouter)."
    exit 0
fi

# Kiểm tra nếu config.toml không chứa ezRouter thì không can thiệp để tránh xóa nhầm cấu hình riêng của user
if ! grep -qi "ezrouter" "$CONFIG_FILE"; then
    echo -e "${YELLOW}ℹ️ File cấu hình không chứa thiết lập ezRouter: ${CONFIG_FILE}${NC}"
    echo -e "Codex hiện không trỏ về ezRouter proxy. Cấu hình được giữ nguyên."
    exit 0
fi

# Sao lưu cấu hình hiện tại trước khi gỡ
PRE_UNINSTALL_BAK="${CONFIG_FILE}.uninstall.$(date +%s)"
cp "$CONFIG_FILE" "$PRE_UNINSTALL_BAK"
echo -e "${YELLOW}📦 Đã sao lưu config hiện tại: ${PRE_UNINSTALL_BAK}${NC}"

# Tìm bản sao lưu gần nhất không chứa ezRouter (nếu có) để khôi phục cấu hình gốc
CLEAN_BAK=""
while IFS= read -r bak; do
    if [ -n "$bak" ] && [ -f "$bak" ]; then
        if ! grep -qi "ezrouter" "$bak"; then
            CLEAN_BAK="$bak"
            break
        fi
    fi
done < <(find "$CODEX_DIR" -maxdepth 1 -name "config.toml.bak*" 2>/dev/null | sort -r)

if [ -n "$CLEAN_BAK" ] && [ -f "$CLEAN_BAK" ]; then
    echo -e "${CYAN}🔄 Đang khôi phục từ bản sao lưu sạch: ${CLEAN_BAK}...${NC}"
    cp "$CLEAN_BAK" "$CONFIG_FILE"
    echo -e "${GREEN}✅ Đã khôi phục file cấu hình gốc thành công!${NC}"
else
    echo -e "${YELLOW}🗑️ Đang gỡ bỏ cấu hình ezRouter khỏi config.toml...${NC}"
    rm -f "$CONFIG_FILE"
    echo -e "${GREEN}✅ Đã gỡ bỏ file cấu hình proxy thành công!${NC}"
fi

echo ""
echo -e "${GREEN}🎉 Gỡ bỏ cấu hình hoàn tất!${NC}"
echo -e "💡 Codex IDE Extension (VS Code / Cursor) và Codex CLI đã trở về trạng thái mặc định của OpenAI."
echo ""
