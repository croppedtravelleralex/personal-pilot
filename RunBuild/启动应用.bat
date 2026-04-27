@echo off
cd /d "%~dp0.."

if exist "build\bin\personal-pilot.exe" (
    start "" "build\bin\personal-pilot.exe"
    exit /b 0
)

echo [INFO] exe not found, starting dev mode...
call RunBuild\dev.bat stable --no-pause
