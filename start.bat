@echo off
setlocal
set "ROOT=%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

echo [1/1] Launching widget...

if exist "%ROOT%musicagent-widget.exe" goto ROOT_EXE
if exist "%ROOT%gpui-widget\target\release\musicagent-widget.exe" goto REL_EXE
goto CARGO_RUN

:ROOT_EXE
echo   Using release binary: %ROOT%musicagent-widget.exe
start "" /B "%ROOT%musicagent-widget.exe"
goto END

:REL_EXE
echo   Using release binary: %ROOT%gpui-widget\target\release\musicagent-widget.exe
start "" /B "%ROOT%gpui-widget\target\release\musicagent-widget.exe"
goto END

:CARGO_RUN
cd /d "%ROOT%gpui-widget"
echo   Using cargo run (development mode)...
cargo run
goto END

:END
endlocal
