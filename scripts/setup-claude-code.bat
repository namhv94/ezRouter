@echo off
REM ==============================================================================
REM ezRouter - Script Cấu hình Tự động Tích hợp Claude Code CLI (Batch)
REM ==============================================================================

setlocal EnableDelayedExpansion

set ROUTER_URL=%~1
if "%ROUTER_URL%"=="" set ROUTER_URL=https://router.namhv.vip

set API_KEY=%~2
if "%API_KEY%"=="" set API_KEY=ag-proxy-key

set MODEL=%~3
if "%MODEL%"=="" set MODEL=ag/claude-sonnet-5-5-high

echo ======================================================
echo      ezRouter - Tich Hop Claude Code CLI (1 Buoc)
echo ======================================================
echo.

setx ANTHROPIC_BASE_URL "%ROUTER_URL%" >nul
setx ANTHROPIC_API_KEY "%API_KEY%" >nul
setx ANTHROPIC_MODEL "%MODEL%" >nul

echo [OK] Tich hop hoan tat!
echo Da luu ANTHROPIC_BASE_URL=%ROUTER_URL%
echo Hay mo cua so Command Prompt / PowerShell moi va chay: claude
echo.
