# Changelog

## [0.1.0] — 2026-05-06

### Added

#### GPUI Desktop Widget (`gpui-widget/`)
- Full GPUI 0.2 desktop widget with Claudio FM-inspired design
- Fluid animated gradient background (HSL hue rotation, 50ms tick)
- Chat-style transcript panel with word-by-word highlight animation
- Input field (gpui-component `InputState`) with Enter-to-send
- Preset question buttons (4 music-themed questions)
- Play/pause toggle button with ambient audio integration
- Card layout with header (avatar, clock, status dot, waveform bars) and body (metadata, transcript, input, player)
- Player progress bar (56-bar visualizer)

#### Audio Subsystem (`gpui-widget/src/audio.rs`)
- **Ambient mode**: 110 Hz sine wave at 0.04 amplitude via `rodio`
- **TTS playback**: Spawns thread with `OutputStream` + `Sink`, decodes WAV from `Cursor`
- Thread-safe atomic playing-state flag
- No global state (rodio `OutputStream` is not `Send`, so each playback creates its own)

#### TTS Client (`gpui-widget/src/tts_client.rs`)
- OpenAI-compatible HTTP client for `kiki-tts-server` at `127.0.0.1:8080`
- 8 built-in voices: Bella, Jasper, Luna, Bruno, Rosie, Hugo, Kiki, Leo
- Uses `ureq` 3 within `smol::unblock` (GPUI runs on smol executor)
- Request payload: `model: kitten-tts-mini`, `response_format: wav`

#### CLI Client (`gpui-widget/src/cli_client.rs`)
- `std::process::Command` + `smol::unblock` calling `opencc` CLI in `-p` (pipe) mode
- 120-second timeout via `child.try_wait()` polling loop
- Auto-loads `ANTHROPIC_API_KEY` / `DEEPSEEK_API_KEY` from `~/.bashrc`
- Maps `ANTHROPIC_AUTH_TOKEN` → `ANTHROPIC_API_KEY` for DeepSeek proxy support

#### AI Engine (`packages/opencc/`)
- MusicAgent system prompt (English, late-night radio DJ persona)
- Music theory, analysis, discovery, history, production capabilities
- Conversational, warm output style with sensory language
- Integrated via `opencc.bat` auto-loader

#### Theme System (`gpui-widget/src/theme.rs`)
- Dark theme with `Theme::dark()` struct
- Full color palette: backgrounds, headers, text states, transcript, player, fluid accents
- Uses `Rgba` with `hex()` and `rgba()` const constructors

#### Project Cleanup
- Removed all Electron-related code (~1.5 GB+):
  - Electron build config, dependencies, scripts from `package.json`
  - Electron-specific entries from `.gitignore`
  - Unused workspace packages removed

### Fixed
- **rodio overflow panic**: Removed `take_duration(Duration::from_secs(u64::MAX))` — sample count overflows on multiplication with sample rate
- **tokio runtime panic**: Replaced `tokio::process::Command` and `tokio::time::timeout` with `std::process::Command` + `smol::unblock` + polling loop (GPUI runs on smol, not tokio)
- **ureq 3 API**: Replaced `AgentBuilder` / `.set()` / `.timeout()` / `.send_json()` with `ureq::post(url).header(...).send(body_string)`
- **InputState::set_value in callback**: Moved input clearing to `render()` where `&mut Window` is available (subscribe callback doesn't provide it)
- **on_click missing `.id()`**: Added `.id("play-btn")` for stateful interactive element

### Known Issues
- **espeak-ng phonemization broken on Windows**: kiki-tts-server returns `{"error": "espeak-ng failed: "}` wrapped as 85-byte pseudo-WAV. The espeak-ng binary is found but produces empty stdout from native Windows context (DLL resolution / data path issue)
