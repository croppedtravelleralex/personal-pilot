@echo off
cd /d "%~dp0"

:: 如果已构建 exe，直接用 start 启动（无黑窗）
if exist "build\bin\personal-pilot.exe" (
    start "" "build\bin\personal-pilot.exe"
    exit /b 0
)

:: 未构建则用稳定模式启动开发环境
echo 未找到已构建 exe，正在启动开发模式...
call bat\dev.bat stable --no-pause
