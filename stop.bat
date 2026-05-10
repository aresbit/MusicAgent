@echo off
setlocal

echo [1/1] Stopping widget process...
taskkill /F /IM musicagent-widget.exe 2>nul && echo   - killed musicagent-widget.exe || echo   - (no musicagent-widget.exe found)

echo.
echo All clear.

endlocal
