@echo off
REM ==============================================================================
REM ezRouter - Script Cấu hình Tự động Tích hợp Claude Code CLI (Batch)
REM ==============================================================================

setlocal EnableDelayedExpansion

set ROUTER_URL=%~1
if "%ROUTER_URL%"=="" set ROUTER_URL=https://router.namhv.vip

set API_KEY=%~2
if "%API_KEY%"=="" set API_KEY=namhv94

set MODEL=%~3
if "%MODEL%"=="" set MODEL=cx/gpt-6.1-sol

echo ======================================================
echo      ezRouter - Tich Hop Claude Code CLI (1 Buoc)
echo ======================================================
echo.

setx ANTHROPIC_BASE_URL "%ROUTER_URL%" >nul
setx ANTHROPIC_AUTH_TOKEN "%API_KEY%" >nul
setx ANTHROPIC_API_KEY "%API_KEY%" >nul
setx ANTHROPIC_MODEL "%MODEL%" >nul
setx ANTHROPIC_SMALL_FAST_MODEL "%MODEL%" >nul
setx CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT "1" >nul

echo [OK] Tich hop hoan tat!
echo Da luu ANTHROPIC_BASE_URL=%ROUTER_URL%
echo Hay mo cua so Command Prompt / PowerShell moi va chay: claude
echo.
