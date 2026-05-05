# TODO — MusicAgent Feature Development & Debug

## Priority Legend
- **P0**: Blocking — must fix before usable
- **P1**: Core feature — needed for MVP
- **P2**: Enhancement — nice to have
- **P3**: Polish — future

---

## P0 — Blocker: Fix TTS Phonemization on Windows

**Context**: `kiki-tts-server` at `D:\yysdl\kiki-tts` returns error when phonemizing text. The `espeak-ng` binary is found but produces empty stdout on Windows (MSVC CRT DLL issue).

**Files**:
- `D:\yysdl\kiki-tts\src\phonemize.rs` — `find_espeak_ng()` + `phonemize()` functions
- `D:\yysdl\kiki-tts\src\preprocess.rs` — calls phonemize

**Possible fixes** (pick one):
- [ ] Install espeak-ng properly with VC++ runtime (ensure `api-ms-win-crt-*.dll` available)
- [ ] Add a pure-Rust fallback phonemizer (e.g. `g2p` crate or `g2p_en` Rust port)
- [ ] Add Python phonemizer subprocess fallback (call Python `phonemizer` package)
- [ ] Add a simple lookup-table phonemizer for common English words
- [ ] Bundle `espeak-ng` data files and set `ESPEAK_DATA_PATH` explicitly

**Verification**: Run `kitten-tts-server`, then:
```bash
curl -X POST http://localhost:8080/v1/audio/speech \
  -H "Content-Type: application/json" \
  -d '{"model":"kitten-tts-mini","input":"Hello world","voice":"Bella","response_format":"wav"}' \
  --output test.wav
```
Should return a valid WAV file (>1KB), not an 85-byte JSON error.

---

## P1 — Audio Source Abstraction Layer

### P1.1 — Define `AudioSource` Trait

**Files**: New `gpui-widget/src/source/` module

```rust
/// gpui-widget/src/source/mod.rs
#[async_trait]
pub trait AudioSource: Send + Sync {
    fn kind(&self) -> AudioSourceKind;
    fn name(&self) -> &'static str;
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>>;
    async fn resolve(&self, id: &str) -> Result<AudioData>;
}

pub enum AudioSourceKind { Tts, Radio, NetEase, Spotify, YouTube, Bilibili, LocalMp3 }
pub struct SearchResult { title, artist, duration, source_kind, source_id }
pub struct AudioData { bytes: Vec<u8>, format: AudioFormat }
pub enum AudioFormat { Wav, Mp3, Ogg, Aac }
```

**Tasks**:
- [ ] Create `gpui-widget/src/source/mod.rs`
- [ ] Define `AudioSource` trait + supporting types
- [ ] Register module in `main.rs`

### P1.2 — Implement `AudioManager`

**Files**: New `gpui-widget/src/audio_manager.rs`

Central playback controller managing rodio `OutputStream` singleton.

```rust
pub struct AudioManager {
    tts_sink: Option<Sink>,
    music_sink: Option<Sink>,
    ambient_sink: Option<Sink>,
    queue: VecDeque<PlaylistItem>,
    current_source: AudioSourceKind,
    volume: f32,
}
```

**Tasks**:
- [ ] Create `AudioManager` struct with three independent sinks (TTS / music / ambient)
- [ ] Implement `play()`, `pause()`, `resume()`, `stop()`, `skip()`, `set_volume()`, `enqueue()`
- [ ] Wire ambient toggle (existing `start_ambient` / `stop_ambient`)
- [ ] Register as GPUI model (`cx.new(|_| AudioManager::new())`)
- [ ] Thread-safe access via `Entity<AudioManager>`

### P1.3 — Implement Local MP3 Playback

**Files**: New `gpui-widget/src/source/local.rs`

**Tasks**:
- [ ] Implement `AudioSource` for local file system
- [ ] File picker dialog (GPUI native or fallback path input)
- [ ] Directory scan with `walkdir` (optional)
- [ ] rodio `Decoder` from file path

### P1.4 — Implement TTS Source Wrapper

**Files**: New `gpui-widget/src/source/tts.rs`

Refactor existing `tts_client.rs` into the `AudioSource` trait.

**Tasks**:
- [ ] Wrap `text_to_speech()` as `AudioSource::resolve()`
- [ ] Integrate with `AudioManager` so TTS playback goes through the manager

---

## P1 — AI-Driven Playback Control

### P1.5 — opencc Tool Definitions

**Files**: `packages/opencc/dist/music-agent-prompt.txt`

Add tool definitions to the system prompt so the AI can send playback commands:

```
## Tools

You have tools to control music playback:

### play_radio
Play an online radio stream.
Parameters: station (string) — station name or URL

### search_music
Search for music from a source.
Parameters: source (string: "netease"|"spotify"|"youtube"|"bilibili"), query (string)

### play_music
Play a specific track.
Parameters: source_id (string), track_id (string)

### play_local
Play a local MP3 file.
Parameters: path (string) — file path

### control_playback
Control current playback.
Parameters: action ("play"|"pause"|"stop"|"skip"|"next"), volume (float, optional)
```

**Tasks**:
- [ ] Add tool definitions to `music-agent-prompt.txt`
- [ ] Ensure opencc CLI supports these tool call formats

### P1.6 — Parse AI Tool Calls in Widget

**Files**: `gpui-widget/src/main.rs`

**Tasks**:
- [ ] Parse AI response for tool call patterns (e.g. `{"tool": "play_radio", "args": {...}}`)
- [ ] Route parsed commands to `AudioManager`
- [ ] Strip tool call JSON from chat display (show natural language only)
- [ ] Handle tool call errors gracefully

---

## P2 — External Music Sources

### P2.1 — Online Radio Stream

**Files**: New `gpui-widget/src/source/radio.rs`

**Tasks**:
- [ ] Implement `AudioSource` for Icecast/Shoutcast HTTP MP3 streams
- [ ] Use rodio's `Decoder::new` with a streaming HTTP reader (`ureq` stream)
- [ ] Add preset radio station list (e.g. HitFM, CRI, BBC World Service)
- [ ] Test with known working Icecast URLs

**Dependencies**: `ureq` 3 (already in Cargo.toml)

### P2.2 — NetEase Cloud Music (网易云)

**Files**: New `gpui-widget/src/source/netease.rs`

**Tasks**:
- [ ] Clone/setup NeteaseCloudMusicApi Node.js service
- [ ] Implement Rust HTTP client calling the API service
- [ ] Search songs via `/search` endpoint
- [ ] Get song URL via `/song/url/v1` endpoint
- [ ] Cache song URLs to avoid repeated API calls

**Dependencies**:
- External: https://github.com/Binaryify/NeteaseCloudMusicApi (Node.js)
- Start command: `npm start` in the API service directory

### P2.3 — YouTube Audio Extraction

**Files**: New `gpui-widget/src/source/youtube.rs`

**Tasks**:
- [ ] Implement `yt-dlp` subprocess call to extract audio URL
- [ ] Pipe extracted audio URL to rodio (or download then play)
- [ ] Search YouTube via yt-dlp or public API
- [ ] Handle playlists vs single tracks

**Dependencies**: `yt-dlp` (Python CLI, must be installed on system)

### P2.4 — Bilibili Audio Extraction

**Files**: New `gpui-widget/src/source/bilibili.rs`

**Tasks**:
- [ ] Implement HTTP client for bilibili video API
- [ ] Extract audio stream URL from video page
- [ ] Support BV number and search queries
- [ ] Handle cookie-based authentication if needed

### P2.5 — Spotify Integration

**Files**: New `gpui-widget/src/source/spotify.rs`

**Tasks**:
- [ ] Register Spotify App and get Client ID/Secret
- [ ] Implement OAuth Device Code flow (no browser redirect needed)
- [ ] Implement search via Spotify Web API
- [ ] Play 30-second preview clips via `preview_url`
- [ ] (Optional) Integrate `librespot` for full track playback

**Dependencies**: Spotify Developer account, `ureq` for API calls

---

## P3 — Polish & Fixes

### P3.1 — kiki-tts Server Auto-Start

**Files**: `gpui-widget/src/main.rs`, `gpui-widget/src/tts_client.rs`

**Tasks**:
- [ ] Auto-launch `kitten-tts-server` subprocess on widget startup
- [ ] Wait for server readiness (`GET /health` or similar)
- [ ] Graceful shutdown on widget close
- [ ] Show server status in UI

### P3.2 — Error Handling & Recovery

**Tasks**:
- [ ] Handle TTS server not running → show clear error in UI
- [ ] Handle API key missing → show setup instructions in UI
- [ ] Handle network timeout for external sources
- [ ] Fallback chain: primary source → fallback → clear error message

### P3.3 — UI Enhancements

**Tasks**:
- [ ] Real-time waveform visualization during playback (via rodio samples)
- [ ] Volume slider in player controls
- [ ] Source selector dropdown (radio / netease / spotify / local)
- [ ] Current track info display (title, artist, progress bar)
- [ ] Loading spinner during TTS/cli requests
- [ ] Scroll-to-bottom on new transcript messages

### P3.4 — Audio Quality & Playback

**Tasks**:
- [ ] Duck ambient volume when TTS is speaking
- [ ] Crossfade between tracks
- [ ] Playlist queue management UI
- [ ] Equalizer / tone controls (optional)

---

## Project Structure Reference

```
D:\yyscode\MusicAgent\
├── CHANGELOG.md             ← this file
├── TODO.md                  ← this file
├── package.json             ← workspace config (opencc only)
├── .claude/
│   └── CLAUDE.md            ← project instructions
├── gpui-widget/
│   ├── Cargo.toml           ← Rust dependencies
│   └── src/
│       ├── main.rs          ← GPUI widget UI + app state
│       ├── audio.rs         ← rodio playback (ambient + TTS)
│       ├── audio_manager.rs ← [TODO] unified playback controller
│       ├── cli_client.rs    ← opencc CLI communication
│       ├── tts_client.rs    ← kiki-tts HTTP client
│       ├── theme.rs         ← color palette
│       └── source/          ← [TODO] audio source implementations
│           ├── mod.rs       ← AudioSource trait
│           ├── local.rs     ← local MP3
│           ├── radio.rs     ← online radio
│           ├── netease.rs   ← 网易云音乐
│           ├── spotify.rs   ← Spotify
│           ├── youtube.rs   ← YouTube
│           └── bilibili.rs  ← 哔哩哔哩
├── packages/
│   └── opencc/
│       ├── dist/
│       │   ├── cli.js                  ← AI engine entry
│       │   └── music-agent-prompt.txt  ← system prompt
│       └── opencc.bat                 ← Windows launcher
└── (external) D:\yysdl\kiki-tts\      ← TTS engine
    ├── Cargo.toml
    └── src/
        ├── phonemize.rs    ← [BUG] espeak-ng Windows issue
        ├── preprocess.rs
        └── bin/kitten-tts-server/   ← TTS HTTP server
```

## Build & Run Commands

```bash
# Build GPUI widget
cd D:\yyscode\MusicAgent\gpui-widget
cargo build

# Build kiki-tts server
cd D:\yysdl\kiki-tts
cargo build --bin kitten-tts-server --release

# Start TTS server
cd D:\yysdl\kiki-tts
cargo run --bin kitten-tts-server --release -- --model-path /path/to/kitten-tts-mini

# Start widget
cd D:\yyscode\MusicAgent\gpui-widget
cargo run

# Build opencc CLI
cd D:\yyscode\MusicAgent\packages\opencc
bun run build
```
