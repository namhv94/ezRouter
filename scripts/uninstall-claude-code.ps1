# ==============================================================================
# ezRouter - Script Gỡ Bỏ Cấu Hình Claude Code CLI (PowerShell)
# Hỗ trợ: Windows PowerShell / PowerShell 7+
# Chạy 1 bước: iwr -useb https://router.namhv.vip/uninstall-claude-code.ps1 | iex
# ==============================================================================

Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "     ⚡ ezRouter - Gỡ Bỏ Cấu Hình Claude Code          " -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "⚙️ Đang gỡ bỏ biến môi trường User..." -ForegroundColor Yellow

[Environment]::SetEnvironmentVariable("ANTHROPIC_BASE_URL", $null, "User")
[Environment]::SetEnvironmentVariable("ANTHROPIC_API_KEY", $null, "User")
[Environment]::SetEnvironmentVariable("ANTHROPIC_MODEL", $null, "User")

$env:ANTHROPIC_BASE_URL = $null
$env:ANTHROPIC_API_KEY = $null
$env:ANTHROPIC_MODEL = $null

Write-Host ""
Write-Host "🎉 Gỡ bỏ cấu hình hoàn tất thành công!" -ForegroundColor Green
Write-Host "💡 Đã xóa biến môi trường ANTHROPIC_BASE_URL và ANTHROPIC_MODEL khỏi máy." -ForegroundColor Gray
Write-Host ""
