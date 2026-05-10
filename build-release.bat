@echo off
setlocal enabledelayedexpansion

set "ROOT=%~dp0"
set "OPENCC_DIR=%ROOT%packages\opencc"
set "GPUI_DIR=%ROOT%gpui-widget"
set "RELEASE_DIR=%ROOT%release"

echo ==========================================
echo  MusicAgent 1.0 Release Build
echo ==========================================
echo.

REM ---- Step 1: Build opencc CLI ----
echo [1/3] Building opencc CLI...
cd /d "%OPENCC_DIR%"

echo   Running bun install...
call bun install >nul 2>nul
if errorlevel 1 (
    echo ERROR: bun install failed in %OPENCC_DIR%
    exit /b 1
)

echo   Running bun run build...
call bun run build >nul 2>nul
if errorlevel 1 (
    echo ERROR: opencc build failed
    exit /b 1
)

if not exist "%OPENCC_DIR%\dist\cli.js" (
    echo ERROR: opencc dist/cli.js not found after build
    exit /b 1
)
echo   OK: opencc CLI built successfully
echo.

REM ---- Step 2: Build Rust widget ----
echo [2/3] Building GPUI widget (release mode)...
cd /d "%GPUI_DIR%"

cargo build --release
if errorlevel 1 (
    echo ERROR: Rust build failed
    exit /b 1
)

set "RELEASE_EXE=%GPUI_DIR%\target\release\musicagent-widget.exe"
if not exist "!RELEASE_EXE!" (
    echo ERROR: Release binary not found at !RELEASE_EXE!
    exit /b 1
)
echo   OK: musicagent-widget.exe built successfully
echo.

REM ---- Step 3: Stage release artifacts ----
echo [3/3] Staging release artifacts...

if exist "%RELEASE_DIR%" (
    rmdir /s /q "%RELEASE_DIR%"
)
mkdir "%RELEASE_DIR%"
mkdir "%RELEASE_DIR%\packages\opencc\dist"

REM Copy Rust binary
copy "%RELEASE_EXE%" "%RELEASE_DIR%\musicagent-widget.exe" >nul
echo   - musicagent-widget.exe

REM Copy batch scripts
copy "%ROOT%start.bat" "%RELEASE_DIR%\start.bat" >nul
copy "%ROOT%stop.bat" "%RELEASE_DIR%\stop.bat" >nul
echo   - start.bat, stop.bat

REM Copy root package.json (version info)
copy "%ROOT%package.json" "%RELEASE_DIR%\package.json" >nul
echo   - package.json

REM Copy opencc CLI (bundled dist only, node_modules not needed)
copy "%OPENCC_DIR%\dist\cli.js" "%RELEASE_DIR%\packages\opencc\dist\cli.js" >nul
copy "%OPENCC_DIR%\package.json" "%RELEASE_DIR%\packages\opencc\package.json" >nul
copy "%OPENCC_DIR%\opencc.bat" "%RELEASE_DIR%\packages\opencc\opencc.bat" >nul
copy "%OPENCC_DIR%\music-agent-prompt.txt" "%RELEASE_DIR%\packages\opencc\music-agent-prompt.txt" >nul
echo   - packages/opencc/

REM Create gpui-widget placeholder dir
mkdir "%RELEASE_DIR%\gpui-widget" >nul 2>nul
echo   - gpui-widget/

echo.
echo ==========================================
echo  Release staged in: %RELEASE_DIR%
echo.
echo  To create zip:
echo    cd /d "%ROOT%"
echo    powershell Compress-Archive -Path release\* -DestinationPath MusicAgent-v1.0.0.zip
echo ==========================================

REM ---- Step 4: Create zip archive (optional) ----
echo.
echo [4/3] Creating zip archive...
cd /d "%ROOT%"
if exist "MusicAgent-v1.0.0.zip" del "MusicAgent-v1.0.0.zip"
powershell -NoProfile -Command "Compress-Archive -Path '%RELEASE_DIR%\*' -DestinationPath 'MusicAgent-v1.0.0.zip'" >nul
if errorlevel 1 (
    echo WARNING: zip creation failed
) else (
    for %%F in (MusicAgent-v1.0.0.zip) do echo   OK: MusicAgent-v1.0.0.zip (%%~zF bytes)
)
echo.

echo ==========================================
echo  Build complete!
echo.
echo  Binary:           %RELEASE_DIR%\musicagent-widget.exe
echo  Archive:          %ROOT%MusicAgent-v1.0.0.zip
echo.
echo  To run: cd %RELEASE_DIR% ^&^& start.bat
echo ==========================================

endlocal
