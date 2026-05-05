@echo off
REM Windows wrapper script for OpenCC (Claude Code CLI) - MusicAgent Edition
REM This script allows running OpenCC on Windows without Unix shell
REM MusicAgent: 私人 AI 电台主播，专注音乐探索与推荐

REM Get the directory where this batch file is located
set "SCRIPT_DIR=%~dp0"

REM Define MusicAgent system prompt file path (before setlocal to persist)
set "MUSIC_AGENT_PROMPT=%SCRIPT_DIR%music-agent-prompt.txt"

REM Check if user already passed --system-prompt or --system-prompt-file
setlocal enabledelayedexpansion
set "HAS_SYSTEM_PROMPT_FLAG=0"
for %%a in (%*) do (
    set "arg=%%a"
    if "!arg!"=="--system-prompt" set "HAS_SYSTEM_PROMPT_FLAG=1"
    if "!arg!"=="--system-prompt-file" set "HAS_SYSTEM_PROMPT_FLAG=1"
)
endlocal & set "HAS_SYSTEM_PROMPT_FLAG=%HAS_SYSTEM_PROMPT_FLAG%"

IF "%HAS_SYSTEM_PROMPT_FLAG%"=="0" (
    IF EXIST "%MUSIC_AGENT_PROMPT%" (
        REM Auto-load MusicAgent system prompt
        bun run "%SCRIPT_DIR%dist\cli.js" --dangerously-skip-permissions --system-prompt-file "%MUSIC_AGENT_PROMPT%" %*
    ) ELSE (
        REM Fallback to default behavior if prompt file is missing
        bun run "%SCRIPT_DIR%dist\cli.js" --dangerously-skip-permissions %*
    )
) ELSE (
    REM User provided their own system prompt, respect it
    bun run "%SCRIPT_DIR%dist\cli.js" --dangerously-skip-permissions %*
)
