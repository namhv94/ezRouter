# ==============================================================================
# ezRouter - Script Cấu hình Tự động Tích hợp Claude Code CLI (PowerShell)
# Hỗ trợ: Windows PowerShell / PowerShell 7+
# Chạy 1 bước: iwr -useb https://router.namhv.vip/setup-claude-code.ps1 | iex
# ==============================================================================

param(
    [string]$RouterUrl = "https://router.namhv.vip",
    [string]$ApiKey = "ag-proxy-key",
    [string]$Model = "ag/claude-sonnet-5-5-high"
)

Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "     ⚡ ezRouter - Tích Hợp Claude Code CLI (1 Bước)    " -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

$CleanUrl = $RouterUrl.TrimEnd('/').TrimEnd('/v1')

Write-Host "⚙️ Đang cấu hình biến môi trường User (Persistent)..." -ForegroundColor Yellow
Write-Host "   - Base URL : $CleanUrl" -ForegroundColor Green
Write-Host "   - Model    : $Model" -ForegroundColor Green

[Environment]::SetEnvironmentVariable("ANTHROPIC_BASE_URL", $CleanUrl, "User")
[Environment]::SetEnvironmentVariable("ANTHROPIC_API_KEY", $ApiKey, "User")
[Environment]::SetEnvironmentVariable("ANTHROPIC_MODEL", $Model, "User")

# Set cho process hiện tại
$env:ANTHROPIC_BASE_URL = $CleanUrl
$env:ANTHROPIC_API_KEY = $ApiKey
$env:ANTHROPIC_MODEL = $Model

Write-Host ""
Write-Host "✅ Tích hợp hoàn tất thành công!" -ForegroundColor Green
Write-Host "💡 Đã lưu biến môi trường ANTHROPIC_BASE_URL và ANTHROPIC_MODEL vào User Environment." -ForegroundColor Gray
Write-Host ""
Write-Host "👉 Hãy mở terminal mới hoặc chạy:" -ForegroundColor Cyan
Write-Host "   claude" -ForegroundColor White
Write-Host ""
