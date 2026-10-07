# ==============================================================================
# ezRouter - Script Cấu hình Tự động Tích hợp Claude Code CLI (PowerShell)
# Hỗ trợ: Windows PowerShell / PowerShell 7+
# Chạy 1 bước: iwr -useb https://router.namhv.vip/setup-claude-code.ps1 | iex
# ==============================================================================

param(
    [string]$RouterUrl = "https://router.namhv.vip",
    [string]$ApiKey = "namhv94",
    [string]$Model = "cx/gpt-6.1-sol"
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
[Environment]::SetEnvironmentVariable("ANTHROPIC_AUTH_TOKEN", $ApiKey, "User")
[Environment]::SetEnvironmentVariable("ANTHROPIC_API_KEY", $ApiKey, "User")
[Environment]::SetEnvironmentVariable("ANTHROPIC_MODEL", $Model, "User")
[Environment]::SetEnvironmentVariable("ANTHROPIC_SMALL_FAST_MODEL", $Model, "User")
[Environment]::SetEnvironmentVariable("CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT", "1", "User")

# Set cho process hiện tại
$env:ANTHROPIC_BASE_URL = $CleanUrl
$env:ANTHROPIC_AUTH_TOKEN = $ApiKey
$env:ANTHROPIC_API_KEY = $ApiKey
$env:ANTHROPIC_MODEL = $Model
$env:ANTHROPIC_SMALL_FAST_MODEL = $Model
$env:CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT = "1"

# Cập nhật ~/.claude/settings.json
$ClaudeDir = Join-Path $HOME ".claude"
if (-not (Test-Path $ClaudeDir)) {
    New-Item -ItemType Directory -Path $ClaudeDir | Out-Null
}
$SettingsFile = Join-Path $ClaudeDir "settings.json"
$Settings = @{ env = @{} }
if (Test-Path $SettingsFile) {
    try {
        $Settings = Get-Content $SettingsFile -Raw | ConvertFrom-Json
        if (-not $Settings.env) { $Settings | Add-Member -MemberType NoteProperty -Name "env" -Value @{} }
    } catch {}
}
$Settings.env.ANTHROPIC_BASE_URL = $CleanUrl
$Settings.env.ANTHROPIC_AUTH_TOKEN = $ApiKey
$Settings.env.ANTHROPIC_API_KEY = $ApiKey
$Settings.env.ANTHROPIC_MODEL = $Model
$Settings.env.ANTHROPIC_SMALL_FAST_MODEL = $Model
$Settings.env.CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT = "1"

$Settings | ConvertTo-Json -Depth 5 | Set-Content $SettingsFile

Write-Host ""
Write-Host "✅ Tích hợp hoàn tất thành công!" -ForegroundColor Green
Write-Host "💡 Đã lưu cấu hình vào User Environment và $SettingsFile." -ForegroundColor Gray
Write-Host ""
Write-Host "👉 Hãy mở terminal mới hoặc chạy:" -ForegroundColor Cyan
Write-Host "   claude" -ForegroundColor White
Write-Host ""
