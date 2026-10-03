@echo off
chcp 65001 >nul
title ezRouter - Huong dan tich hop Antigravity IDE

echo ======================================================
echo    ezRouter - Huong Dan Tich Hop Antigravity IDE
echo ======================================================
echo.

set "DEFAULT_URL=http://127.0.0.1:20229/v1"
set "DEFAULT_KEY=ag-proxy-key"

set /p USER_URL="Nhap ezRouter Base URL [Mac dinh: %DEFAULT_URL%]: "
if "%USER_URL%"=="" (set "ROUTER_URL=%DEFAULT_URL%") else (set "ROUTER_URL=%USER_URL%")

set /p USER_KEY="Nhap ezRouter API Key [Mac dinh: %DEFAULT_KEY%]: "
if "%USER_KEY%"=="" (set "API_KEY=%DEFAULT_KEY%") else (set "API_KEY=%USER_KEY%")

echo.
echo Chon phuong thuc tich hop:
echo   [1] Cach 1 (Khuyen dung): Tu dong cau hinh Extension Continue trong Antigravity IDE
echo   [2] Cach 2 (Nang cao): Huong dan Proxy / Settings cho Antigravity IDE
echo   [3] Kiem tra ket noi: Test goi API toi ezRouter
echo   [0] Thoat
echo.

set /p CHOICE="Vui long chon [1/2/3/0]: "

if "%CHOICE%"=="1" goto opt_continue
if "%CHOICE%"=="2" goto opt_proxy
if "%CHOICE%"=="3" goto opt_test
if "%CHOICE%"=="0" goto opt_exit
goto opt_invalid

:opt_continue
echo.
echo ==^> [Cach 1] Dang cau hinh Extension Continue cho Antigravity IDE...
set "CONTINUE_DIR=%USERPROFILE%\.continue"
if not exist "%CONTINUE_DIR%" mkdir "%CONTINUE_DIR%"
set "CONFIG_FILE=%CONTINUE_DIR%\config.json"

if exist "%CONFIG_FILE%" (
    copy /y "%CONFIG_FILE%" "%CONFIG_FILE%.bak" >nul
    echo Da backup file config cu sang %CONFIG_FILE%.bak
)

(
echo {
echo   "models": [
echo     {
echo       "title": "Gemini 3.8 Flash High (ezRouter)",
echo       "provider": "openai",
echo       "model": "ag/gemini-3.8-flash-high",
echo       "apiBase": "%ROUTER_URL%",
echo       "apiKey": "%API_KEY%"
echo     },
echo     {
echo       "title": "Claude Opus 5.5 (ezRouter)",
echo       "provider": "openai",
echo       "model": "claude-opus-5.5",
echo       "apiBase": "%ROUTER_URL%",
echo       "apiKey": "%API_KEY%"
echo     },
echo     {
echo       "title": "GPT-5 Codex (ezRouter)",
echo       "provider": "openai",
echo       "model": "cx/gpt-5",
echo       "apiBase": "%ROUTER_URL%",
echo       "apiKey": "%API_KEY%"
echo     },
echo     {
echo       "title": "Gemini 3.0 Pro (ezRouter)",
echo       "provider": "openai",
echo       "model": "ag/gemini-3.0-pro",
echo       "apiBase": "%ROUTER_URL%",
echo       "apiKey": "%API_KEY%"
echo     }
echo   ],
echo   "tabAutocompleteModel": {
echo     "title": "Gemini 2.5 Flash Autocomplete",
echo     "provider": "openai",
echo     "model": "ag/gemini-2.5-flash",
echo     "apiBase": "%ROUTER_URL%",
echo     "apiKey": "%API_KEY%"
echo   },
echo   "embeddingsProvider": {
echo     "provider": "openai",
echo     "model": "text-embedding-3-small",
echo     "apiBase": "%ROUTER_URL%",
echo     "apiKey": "%API_KEY%"
echo   },
echo   "allowAnonymousTelemetry": false
echo }
) > "%CONFIG_FILE%"

echo.
echo [OK] Da tao file cau hinh thanh cong tai:
echo      %CONFIG_FILE%
echo.
echo Cac buoc tiep theo trong Antigravity IDE:
echo   1. Mo Antigravity IDE.
echo   2. Bam to hop phim Ctrl+Shift+X de mo Extensions.
echo   3. Tim kiem "Continue" va bam Install.
echo   4. Mo sidebar Continue (Ctrl+L), chon model "Gemini 3.8 Flash High (ezRouter)" va dung ngay!
echo.
pause
exit /b 0

:opt_proxy
echo.
echo ==^> [Cach 2] Huong dan cau hinh Proxy / Settings cho Antigravity IDE...
echo.
echo Thu muc cau hinh Antigravity tren Windows:
echo   %%APPDATA%%\Antigravity\User\settings.json
echo.
echo Them noi dung sau vao file settings.json:
echo {
echo   "http.proxy": "http://127.0.0.1:20229",
echo   "http.proxyStrictSSL": false
echo }
echo.
echo Neu chay qua cmd hoac PowerShell truoc khi mo IDE:
echo   set HTTP_PROXY=http://127.0.0.1:20229
echo   set HTTPS_PROXY=http://127.0.0.1:20229
echo   set NODE_TLS_REJECT_UNAUTHORIZED=0
echo.
pause
exit /b 0

:opt_test
echo.
echo ==^> [Kiem tra ket noi] Dang kiem tra health tai %ROUTER_URL%...
curl -s "%ROUTER_URL%/models" -H "Authorization: Bearer %API_KEY%"
echo.
echo [OK] Kiem tra xong!
pause
exit /b 0

:opt_exit
exit /b 0

:opt_invalid
echo Lua chon khong hop le.
pause
exit /b 1
