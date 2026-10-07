@echo off
REM ==============================================================================
REM ezRouter - Script Go Bo Cau Hinh Claude Code CLI (Batch)
REM ==============================================================================

echo ======================================================
echo      ezRouter - Go Bo Cau Hinh Claude Code
echo ======================================================
echo.

REG delete "HKCU\Environment" /F /V ANTHROPIC_BASE_URL >nul 2>&1
REG delete "HKCU\Environment" /F /V ANTHROPIC_API_KEY >nul 2>&1
REG delete "HKCU\Environment" /F /V ANTHROPIC_MODEL >nul 2>&1

echo [OK] Da go bo cau hinh Claude Code thanh cong!
echo.
