@echo off
setlocal

echo [1/3] Stopping TTS server processes...
taskkill /F /IM kitten-tts-server.exe 2>nul && echo   - killed kitten-tts-server.exe || echo   - (no kitten-tts-server.exe found)

echo [2/3] Freeing port 8005...
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Get-NetTCPConnection -LocalPort 8005 -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess -Unique | ForEach-Object { Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue; Write-Host ('  - killed PID ' + $_) }"
timeout /t 1 /nobreak >nul

echo [3/3] Checking port 8005 is free...
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$p=Get-NetTCPConnection -LocalPort 8005 -ErrorAction SilentlyContinue; if($p){ Write-Host '  WARNING: port 8005 still in use by PID ' + $p.OwningProcess; exit 1 }else{ Write-Host '  OK: port 8005 is free.' }"
if errorlevel 1 (
  echo.
  echo Port 8005 still in use. Try: taskkill /F /IM python.exe
  exit /b 1
)

echo.
echo All clear.

endlocal
