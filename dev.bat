@echo off
setlocal EnableExtensions

cd /d "%~dp0"
set "LOG=%~dp0dev-log.txt"
echo Dev log - %date% %time% > "%LOG%"

echo ========================================
echo   antbrowser - Dev Launcher
echo ========================================
echo.
echo Log: dev-log.txt
echo.

:: 1. Check wails
echo [1/4] Checking wails...
echo [1/4] Checking wails... >> "%LOG%"
where wails >nul 2>&1
if errorlevel 1 (
    echo [ERROR] wails not found on PATH
    echo [ERROR] wails not found on PATH >> "%LOG%"
    echo Run: go install github.com/wailsapp/wails/v2/cmd/wails@latest
    echo Run: go install github.com/wailsapp/wails/v2/cmd/wails@latest >> "%LOG%"
    pause
    exit /b 1
)
echo [OK] wails found
echo [OK] wails found >> "%LOG%"

:: 2. Generate bindings
echo [2/4] Generating Wails bindings...
echo [2/4] Generating Wails bindings... >> "%LOG%"
wails generate module >> "%LOG%" 2>&1
if errorlevel 1 (
    echo [ERROR] Failed to generate Wails bindings
    echo [ERROR] Failed to generate Wails bindings >> "%LOG%"
    pause
    exit /b 1
)
echo [OK] Bindings ready
echo [OK] Bindings ready >> "%LOG%"

:: 3. Build frontend
echo [3/4] Building frontend...
echo [3/4] Building frontend... >> "%LOG%"
cd /d "%~dp0frontend"
call npm run build >> "%LOG%" 2>&1
set "BUILD_EXIT=%errorlevel%"
cd /d "%~dp0"
if not "%BUILD_EXIT%"=="0" (
    echo [ERROR] Frontend build failed - see dev-log.txt
    echo [ERROR] Frontend build failed >> "%LOG%"
    pause
    exit /b 1
)
echo [OK] Frontend built
echo [OK] Frontend built >> "%LOG%"

:: 4. Start Wails dev
echo [4/4] Starting Wails dev...
echo [4/4] Starting Wails dev... >> "%LOG%"
echo.
echo Console will stay open for debugging.
echo Close the app or press Ctrl+C to stop.
echo.

wails dev -m -nogorebuild -noreload -s -skipbindings -assetdir frontend/dist

if errorlevel 1 (
    echo.
    echo [ERROR] Wails dev exited with error - see dev-log.txt
    echo [ERROR] Wails dev exited with error >> "%LOG%"
)
pause
