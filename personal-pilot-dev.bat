@echo off
setlocal EnableExtensions

cd /d "%~dp0"

set "LOG=%~dp0personal-pilot-dev.log"
> "%LOG%" echo PersonalPilot Tauri dev - %date% %time%

echo ========================================
echo   PersonalPilot - Tauri Dev Launcher
echo ========================================
echo.
echo This starts Tauri 2 dev mode.
echo Tauri will build the Go sidecar and start Vite via beforeDevCommand.
echo Log: personal-pilot-dev.log
echo.

where npm >nul 2>&1
if errorlevel 1 (
    echo [ERROR] npm not found on PATH
    >> "%LOG%" echo [ERROR] npm not found on PATH
    pause
    exit /b 1
)

echo [1/1] Starting Tauri dev...
>> "%LOG%" echo [INFO] npm run tauri:dev -- %*
call npm run tauri:dev -- %*
set "EXIT_CODE=%errorlevel%"
>> "%LOG%" echo [INFO] exit code %EXIT_CODE%

if not "%EXIT_CODE%"=="0" (
    echo.
    echo [ERROR] Tauri dev exited with code %EXIT_CODE%
    echo See personal-pilot-dev.log for launcher details.
    pause
)

exit /b %EXIT_CODE%
