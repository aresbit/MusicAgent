@echo off
setlocal

set "ROOT=D:\yyscode\MusicAgent"
set "BACKEND_DIR=D:\yysdl\KittenTTS-FastAPI"
set "BACKEND_LOG=%ROOT%\gpui-widget\tts-fastapi.log"

REM Ensure cargo and uv are on PATH for double-click scenarios
set "PATH=%USERPROFILE%\.cargo\bin;%USERPROFILE%\.local\bin;D:\yysapp\uv;%PATH%"

set "KITTEN_TTS_DEVICE=cuda"
REM Force-set a clean model repo ID (trimming any pre-existing trailing spaces)
set "KITTEN_MODEL_REPO_ID=KittenML/kitten-tts-nano-0.8-fp32"
REM To use a different model, change the line above or set KITTEN_MODEL_REPO_ID_OVERRIDE
if not "%KITTEN_MODEL_REPO_ID_OVERRIDE%"=="" set "KITTEN_MODEL_REPO_ID=%KITTEN_MODEL_REPO_ID_OVERRIDE%"`r`nset "KITTEN_MODEL_REPO_ID=%KITTEN_MODEL_REPO_ID: =%"

REM Add cuDNN DLL directory to PATH for CUDA TTS acceleration
set "CUDNN_DIR=%BACKEND_DIR%\.venv\Lib\site-packages\nvidia\cudnn\bin"
if exist "%CUDNN_DIR%\cudnn64_9.dll" set "PATH=%CUDNN_DIR%;%PATH%"

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
echo [3.5/5] Check ONNX providers...
uv run python -c "import onnxruntime as ort; print('ONNX providers:', ort.get_available_providers())"

echo [4/5] Start FastAPI TTS backend...
echo.>>"%BACKEND_LOG%"
echo ===== START %DATE% %TIME% =====>>"%BACKEND_LOG%"
echo KITTEN_TTS_DEVICE=%KITTEN_TTS_DEVICE%>>"%BACKEND_LOG%"
echo KITTEN_MODEL_REPO_ID=%KITTEN_MODEL_REPO_ID%>>"%BACKEND_LOG%"
REM Use start /B to run TTS server in background (same console, but we redirect its output)
start "" /B cmd /c "cd /d ""%BACKEND_DIR%"" && set ""KITTEN_TTS_DEVICE=%KITTEN_TTS_DEVICE%"" && set ""KITTEN_MODEL_REPO_ID=%KITTEN_MODEL_REPO_ID%"" && uv run src/server.py" >>"%BACKEND_LOG%" 2>&1

echo [5/5] Wait for FastAPI readiness...
echo   Server log: %BACKEND_LOG%
echo.
REM Single-line PowerShell loop 閳?avoid ^ continuations that break %% escaping in batch.
REM %%60 is used because batch eats the first % in %%60 -> %60 for PowerShell modulo.
powershell -NoProfile -ExecutionPolicy Bypass -Command "$sec=0; while($sec -lt 180){ try{ $r=Invoke-WebRequest -Uri 'http://127.0.0.1:8005/health/ready' -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop; if($r.StatusCode -eq 200){ Write-Host ''; exit 0 } }catch{}; Write-Host ('  waiting... '+$sec+'s') -NoNewline; $log='%BACKEND_LOG%'; if(Test-Path $log){ $last=Get-Content $log -Tail 1 -ErrorAction SilentlyContinue; if($last){ Write-Host ('  '+$last) }else{ Write-Host '' } }else{ Write-Host '' }; Start-Sleep -Seconds 2; $sec=$sec+2 }; Write-Host '  Timed out after 3 minutes.'; exit 1"
if errorlevel 1 (
  echo ERROR: FastAPI TTS backend not ready.
  echo See log: %BACKEND_LOG%
  powershell -NoProfile -ExecutionPolicy Bypass -Command "if (Test-Path '%BACKEND_LOG%') { Get-Content '%BACKEND_LOG%' -Tail 120 }"
  exit /b 1
)

echo [5.5/5] TTS smoke test...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$body=@{model='tts-1';input='MusicAgent TTS smoke test.';voice='Bella';response_format='wav'} | ConvertTo-Json; try { $r=Invoke-WebRequest -Uri 'http://127.0.0.1:8005/v1/audio/speech' -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 12; if($r.StatusCode -ne 200){ exit 1 } } catch { Write-Host $_.Exception.Message; exit 1 }"
if errorlevel 1 (
  echo ERROR: FastAPI TTS smoke test failed.
  echo See log: %BACKEND_LOG%
  powershell -NoProfile -ExecutionPolicy Bypass -Command "if (Test-Path '%BACKEND_LOG%') { Get-Content '%BACKEND_LOG%' -Tail 120 }"
  exit /b 1
)

echo Backend ready. Launching widget...
cd /d "%ROOT%\gpui-widget"
cargo run

endlocal
