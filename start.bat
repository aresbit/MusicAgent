@echo off
setlocal

set "ROOT=D:\yyscode\MusicAgent"
set "TTS_DIR=D:\yysdl\kiki-tts"
set "TTS_EXE=%TTS_DIR%\target\release\kitten-tts-server.exe"
set "TTS_MODEL=%TTS_DIR%\kitten-tts-mini"
set "TTS_LOG=%ROOT%\gpui-widget\tts-server.log"

echo [1/4] Stop existing TTS servers...
taskkill /F /IM kitten-tts-server.exe >nul 2>nul
timeout /t 1 /nobreak >nul
powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-NetTCPConnection -LocalPort 8080 -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess -Unique | ForEach-Object { Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }"

echo [2/4] Start TTS server...
if not exist "%TTS_EXE%" goto :err_no_exe
if not exist "%TTS_MODEL%" goto :err_no_model

echo.>>"%TTS_LOG%"
echo ===== START %DATE% %TIME% =====>>"%TTS_LOG%"
start "" /B "%TTS_EXE%" "%TTS_MODEL%" --host 127.0.0.1 --port 8080 1>>"%TTS_LOG%" 2>&1
timeout /t 1 /nobreak >nul
tasklist /FI "IMAGENAME eq kitten-tts-server.exe" | find /I "kitten-tts-server.exe" >nul
if errorlevel 1 goto :err_no_proc

echo [3/4] Wait for TTS health...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$ok=$false; $max=240; for($i=0;$i -lt $max;$i++){ try { $r=Invoke-WebRequest -Uri 'http://127.0.0.1:8080/health' -UseBasicParsing -TimeoutSec 1; if($r.StatusCode -eq 200){$ok=$true; break} } catch {}; if((($i -band 7) -eq 0)){ Write-Host ('  waiting... ' + [int]($i/4) + 's') }; Start-Sleep -Milliseconds 250 }; if(-not $ok){ exit 1 }"
if errorlevel 1 goto :err_health

echo [4/4] Run MusicAgent widget...
cd /d "%ROOT%\gpui-widget"
cargo run
goto :eof

:err_no_exe
echo ERROR: TTS exe not found: %TTS_EXE%
exit /b 1

:err_no_model
echo ERROR: TTS model dir not found: %TTS_MODEL%
exit /b 1

:err_no_proc
echo ERROR: TTS process did not start.
powershell -NoProfile -ExecutionPolicy Bypass -Command "if (Test-Path '%TTS_LOG%') { Get-Content '%TTS_LOG%' -Tail 80 }"
exit /b 1

:err_health
echo ERROR: TTS server did not become healthy.
echo See log: %TTS_LOG%
powershell -NoProfile -ExecutionPolicy Bypass -Command "if (Test-Path '%TTS_LOG%') { Get-Content '%TTS_LOG%' -Tail 80 } else { Write-Host 'log file not found' }"
exit /b 1
