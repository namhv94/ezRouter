# ==============================================================================
# ezRouter - Script Gỡ Bỏ Cấu Hình OpenAI Codex IDE & CLI
# Hỗ trợ: Windows PowerShell
# Chạy 1 bước: irm https://router.namhv.vip/uninstall-codex.ps1 | iex
# ==============================================================================

Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "     ⚡ ezRouter - Gỡ Bỏ Cấu Hình Codex (Uninstall)     " -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

$codexDir = Join-Path $HOME ".codex"
$configFile = Join-Path $codexDir "config.toml"

if (-not (Test-Path $configFile)) {
    Write-Host "ℹ️ Không tìm thấy file cấu hình tại: $configFile" -ForegroundColor Yellow
    Write-Host "Codex hiện đang ở trạng thái mặc định (chưa cấu hình ezRouter)."
    exit 0
}

# Kiểm tra nếu config.toml không chứa ezRouter thì giữ nguyên để tránh xóa nhầm cấu hình của user
$content = Get-Content $configFile -Raw -ErrorAction SilentlyContinue
if ($content -notmatch "(?i)ezrouter") {
    Write-Host "ℹ️ File cấu hình không chứa thiết lập ezRouter: $configFile" -ForegroundColor Yellow
    Write-Host "Codex hiện không trỏ về ezRouter proxy. Cấu hình được giữ nguyên."
    exit 0
}

$timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$uninstallBak = "$configFile.uninstall.$timestamp"
Copy-Item $configFile $uninstallBak -Force
Write-Host "📦 Đã sao lưu config hiện tại: $uninstallBak" -ForegroundColor Yellow

# Tìm bản sao lưu gần nhất không chứa ezRouter (nếu có)
$backups = Get-ChildItem -Path $codexDir -Filter "config.toml.bak*" -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending
$cleanBak = $null
if ($backups) {
    foreach ($bak in $backups) {
        $bakContent = Get-Content $bak.FullName -Raw -ErrorAction SilentlyContinue
        if ($bakContent -notmatch "(?i)ezrouter") {
            $cleanBak = $bak
            break
        }
    }
}

if ($cleanBak) {
    Write-Host "🔄 Đang khôi phục từ bản sao lưu sạch: $($cleanBak.FullName)..." -ForegroundColor Cyan
    Copy-Item $cleanBak.FullName $configFile -Force
    Write-Host "✅ Đã khôi phục file cấu hình gốc thành công!" -ForegroundColor Green
} else {
    Write-Host "🗑️ Đang gỡ bỏ cấu hình ezRouter khỏi config.toml..." -ForegroundColor Yellow
    Remove-Item $configFile -Force
    Write-Host "✅ Đã gỡ bỏ file cấu hình proxy thành công!" -ForegroundColor Green
}

Write-Host ""
Write-Host "🎉 Gỡ bỏ cấu hình hoàn tất!" -ForegroundColor Green
Write-Host "💡 Codex IDE Extension (VS Code / Cursor) và Codex CLI đã trở về trạng thái mặc định của OpenAI."
Write-Host ""
