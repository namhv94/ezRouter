# ==============================================================================
# ezRouter - Script Cấu hình Tự động Tích hợp OpenAI Codex IDE & CLI
# Hỗ trợ: Windows PowerShell
# Chạy 1 bước: irm https://router.namhv.vip/setup-codex.ps1 | iex
# ==============================================================================

param(
    [string]$RouterUrl = "https://router.namhv.vip/v1",
    [string]$ApiKey = "ag-proxy-key",
    [string]$Model = "cx/gpt-5.6-sol"
)

$RouterUrl = $RouterUrl.TrimEnd('/')

Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "     ⚡ ezRouter - Tích Hợp OpenAI Codex (1 Bước)      " -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

$codexDir = Join-Path $HOME ".codex"
if (-not (Test-Path $codexDir)) {
    New-Item -ItemType Directory -Path $codexDir -Force | Out-Null
}

$configFile = Join-Path $codexDir "config.toml"

Write-Host "⚙️ Đang ghi cấu hình Codex..." -ForegroundColor Yellow
Write-Host "   - Base URL : $RouterUrl" -ForegroundColor Green
Write-Host "   - Model    : $Model" -ForegroundColor Green
Write-Host "   - Config   : $configFile" -ForegroundColor Green

if (Test-Path $configFile) {
    $timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $backupPath = "$configFile.bak.$timestamp"
    Copy-Item $configFile $backupPath -Force
    Write-Host "   - Đã sao lưu config cũ: $backupPath"
}

$configContent = @"
# ==============================================================================
# Cấu hình ezRouter cho OpenAI Codex IDE & Codex CLI
# Tự động tạo bởi ezRouter One-Step Installer
# ==============================================================================

model = "$Model"
model_provider = "ezrouter"

[model_providers.ezrouter]
name = "ezRouter"
base_url = "$RouterUrl"
wire_api = "responses"
supports_websockets = false
experimental_bearer_token = "$ApiKey"
requires_openai_auth = false
request_max_retries = 4
stream_max_retries = 10
stream_idle_timeout_ms = 300000
"@

Set-Content -Path $configFile -Value $configContent -Encoding UTF8

Write-Host ""
Write-Host "✅ Tích hợp hoàn tất thành công!" -ForegroundColor Green
Write-Host "💡 Codex IDE Extension (VS Code / Cursor) và Codex CLI đã sẵn sàng sử dụng."
Write-Host ""
Write-Host "👉 Cách kiểm tra hoạt động:" -ForegroundColor Cyan
Write-Host "   codex -m $Model `"Xin chào!`""
Write-Host ""
