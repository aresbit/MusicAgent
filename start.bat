@echo off
setlocal

set "ROOT=%~dp0"

REM Ensure cargo is on PATH for double-click scenarios
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

echo [1/1] Launching widget...

REM Try release binary first, then fall back to cargo run (development)
if exist "%~dp0musicagent-widget.exe" (
    echo   Using release binary: %~dp0musicagent-widget.exe
    start "" /B "%~dp0musicagent-widget.exe"
) else if exist "%~dp0gpui-widget\target\release\musicagent-widget.exe" (
    echo   Using release binary: %~dp0gpui-widget\target\release\musicagent-widget.exe
    start "" /B "%~dp0gpui-widget\target\release\musicagent-widget.exe"
) else (
    cd /d "%~dp0gpui-widget"
    echo   Using cargo run (development mode)...
    cargo run
)

endlocal
