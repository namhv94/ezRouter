@echo off
rem ==============================================================================
rem ezRouter - Script Go Bo Cau Hinh OpenAI Codex IDE & CLI
rem Ho tro: Windows Command Prompt
rem ==============================================================================

setlocal enabledelayedexpansion

set "CODEX_DIR=%USERPROFILE%\.codex"
set "CONFIG_FILE=%CODEX_DIR%\config.toml"

echo ======================================================
echo      ezRouter - Go Bo Cau Hinh Codex (Uninstall)
echo ======================================================
echo.

if not exist "%CONFIG_FILE%" (
    echo [INFO] Khong tim thay file cau hinh tai: %CONFIG_FILE%
    echo Codex hien dang o trang thai mac dinh.
    exit /b 0
)

findstr /i "ezrouter" "%CONFIG_FILE%" >nul 2>&1
if errorlevel 1 (
    echo [INFO] File cau hinh khong chua thiet lap ezRouter: %CONFIG_FILE%
    echo Codex hien khong tro ve ezRouter proxy. Cau hinh duoc giu nguyen.
    exit /b 0
)

echo Dang sao luu file hien tai sang %CONFIG_FILE%.uninstall.bak...
copy /y "%CONFIG_FILE%" "%CONFIG_FILE%.uninstall.bak" >nul 2>&1

set "CLEAN_BAK="
if exist "%CONFIG_FILE%.bak" (
    findstr /i "ezrouter" "%CONFIG_FILE%.bak" >nul 2>&1
    if errorlevel 1 (
        set "CLEAN_BAK=%CONFIG_FILE%.bak"
    )
)

if not defined CLEAN_BAK (
    for /f "delims=" %%F in ('dir /b /o-d "%CONFIG_FILE%.bak*" 2^>nul') do (
        if not defined CLEAN_BAK (
            findstr /i "ezrouter" "%CODEX_DIR%\%%F" >nul 2>&1
            if errorlevel 1 (
                set "CLEAN_BAK=%CODEX_DIR%\%%F"
            )
        )
    )
)

if defined CLEAN_BAK (
    echo Dang khoi phuc tu %CLEAN_BAK%...
    copy /y "%CLEAN_BAK%" "%CONFIG_FILE%" >nul 2>&1
    echo [OK] Da khoi phuc file cau hinh goc thanh cong!
) else (
    echo Dang go bo cau hinh ezRouter khoi config.toml...
    del /f "%CONFIG_FILE%" >nul 2>&1
    echo [OK] Da go bo file cau hinh proxy thanh cong!
)

echo.
echo [OK] Go bo cau hinh hoan tat!
echo Codex IDE Extension va Codex CLI da tro ve mac dinh cua OpenAI.
echo.
