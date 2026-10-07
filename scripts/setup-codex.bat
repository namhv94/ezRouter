@echo off
rem ==============================================================================
rem ezRouter - Script Cấu hình Tự động Tích hợp OpenAI Codex IDE & CLI
rem Hỗ trợ: Windows Command Prompt
rem ==============================================================================

setlocal enabledelayedexpansion

set "ROUTER_URL=%~1"
if "%ROUTER_URL%"=="" set "ROUTER_URL=https://router.namhv.vip/v1"

set "API_KEY=%~2"
if "%API_KEY%"=="" set "API_KEY=ag-proxy-key"

set "MODEL=%~3"
if "%MODEL%"=="" set "MODEL=cx/gpt-5.6-sol"

rem Strip trailing slash if present
if "%ROUTER_URL:~-1%"=="/" set "ROUTER_URL=%ROUTER_URL:~0,-1%"

set "CODEX_DIR=%USERPROFILE%\.codex"
if not exist "%CODEX_DIR%" mkdir "%CODEX_DIR%"
set "CONFIG_FILE=%CODEX_DIR%\config.toml"

echo ======================================================
echo      ezRouter - Tich Hop OpenAI Codex (1 Buoc)
echo ======================================================
echo.
echo Dang ghi cau hinh Codex...
echo    - Base URL : %ROUTER_URL%
echo    - Model    : %MODEL%
echo    - Config   : %CONFIG_FILE%

if exist "%CONFIG_FILE%" (
    copy "%CONFIG_FILE%" "%CONFIG_FILE%.bak" >nul 2>&1
    echo    - Da sao luu config cu sang %CONFIG_FILE%.bak
)

(
echo # ==============================================================================
echo # Cau hinh ezRouter cho OpenAI Codex IDE ^& Codex CLI
echo # Tu dong tao boi ezRouter One-Step Installer
echo # ==============================================================================
echo.
echo model = "%MODEL%"
echo model_provider = "ezrouter"
echo.
echo [model_providers.ezrouter]
echo name = "ezRouter"
echo base_url = "%ROUTER_URL%"
echo wire_api = "responses"
echo supports_websockets = false
echo experimental_bearer_token = "%API_KEY%"
echo requires_openai_auth = false
echo request_max_retries = 4
echo stream_max_retries = 10
echo stream_idle_timeout_ms = 300000
) > "%CONFIG_FILE%"

echo.
echo [OK] Tich hop hoan tat thanh cong!
echo Codex IDE Extension va Codex CLI da san sang su dung.
echo.
echo Kiem tra hoat dong:
echo    codex -m %MODEL% "Xin chao!"
echo.
