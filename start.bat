@echo off
setlocal

set "ROOT=D:\yyscode\MusicAgent"
set "BACKEND_DIR=D:\yysdl\KittenTTS-FastAPI"
set "BACKEND_LOG=%ROOT%\gpui-widget\tts-fastapi.log"

echo [1/5] Stop existing TTS backends...
taskkill /F /IM kitten-tts-server.exe >nul 2>nul
powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-NetTCPConnection -LocalPort 8005 -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess -Unique | ForEach-Object { Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }"
timeout /t 1 /nobreak >nul

echo [2/5] Check FastAPI backend directory...
if not exist "%BACKEND_DIR%\pyproject.toml" (
  echo ERROR: backend not found: %BACKEND_DIR%
  exit /b 1
)

echo [3/5] Ensure backend deps (uv sync)...
cd /d "%BACKEND_DIR%"
uv sync >nul 2>nul
if errorlevel 1 (
  echo ERROR: uv sync failed in %BACKEND_DIR%
  exit /b 1
)

echo [4/5] Start FastAPI TTS backend...
echo.>>"%BACKEND_LOG%"
echo ===== START %DATE% %TIME% =====>>"%BACKEND_LOG%"
start "" /B cmd /c "cd /d %BACKEND_DIR% && uv run src/server.py" 1>>"%BACKEND_LOG%" 2>&1

echo [5/5] Wait for FastAPI readiness...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$ok=$false; $max=240; for($i=0;$i -lt $max;$i++){ try { $r=Invoke-WebRequest -Uri 'http://127.0.0.1:8005/health/ready' -UseBasicParsing -TimeoutSec 1; if($r.StatusCode -eq 200){$ok=$true; break} } catch {}; if((($i -band 7) -eq 0)){ Write-Host ('  waiting... ' + [int]($i/4) + 's') }; Start-Sleep -Milliseconds 250 }; if(-not $ok){ exit 1 }"
if errorlevel 1 (
  echo ERROR: FastAPI TTS backend not ready.
  echo See log: %BACKEND_LOG%
  powershell -NoProfile -ExecutionPolicy Bypass -Command "if (Test-Path '%BACKEND_LOG%') { Get-Content '%BACKEND_LOG%' -Tail 120 }"
  exit /b 1
)

echo Backend ready. Launching widget...
cd /d "%ROOT%\gpui-widget"
cargo run

endlocal
