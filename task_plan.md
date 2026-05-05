# Task Plan: MusicAgent — 多音频源架构设计

## Goal
将 MusicAgent 从单一的 AI TTS 电台，升级为支持**多种音乐源**的 AI 音乐助手：opencc TTS、在线电台、网易云、Spotify、YouTube、Bilibili、本地 MP3。

## Current Phase
Phase 2 — Architecture & Planning

## Phases

### Phase 1: Requirements & Discovery ✅
- [x] 明确用户需求（7 种音频源）
- [x] 识别技术约束（GPUI/smol、Windows、rodio）
- [x] 记录现有架构状态
- **Status:** completed

### Phase 2: Architecture & Planning
- [x] 设计音频源抽象层（Source Abstraction Layer）
- [x] 设计 Audio Manager 统一调度
- [x] 设计 AI 驱动播放接口（opencc Tool-Based Control）
- [x] 绘制组件架构图
- [ ] 定义每个音频源的集成方案
- [ ] 确定实现优先级
- **Status:** in_progress

### Phase 3: Core Implementation (Source Abstraction)
- [ ] 实现 `AudioSource` trait 和统一接口
- [ ] 实现 `AudioManager` 播放调度
- [ ] 实现本地 MP3 播放器
- [ ] 实现在线电台流播放
- [ ] 实现 TTS 播放集成
- **Status:** pending

### Phase 4: External Service Integration
- [ ] 网易云音乐 API 客户端
- [ ] Spotify Web API 客户端（OAuth 授权）
- [ ] YouTube/Bilibili 流提取 + 播放
- [ ] opencc Tool 接口对接（让 AI 控制播放）
- **Status:** pending

### Phase 5: Testing & Verification
- [ ] 验证所有音频源正常工作
- [ ] 验证 AI 语音指令控制播放
- [ ] 验证错误处理和降级策略
- **Status:** pending

## Architecture Overview

```
┌──────────────────────────────────────────────────────┐
│                    GPUI Widget UI                      │
│  ┌──────────────────┐  ┌──────────────────────────┐  │
│  │  Chat Panel       │  │  Player Controls          │  │
│  │  (opencc Stream)  │  │  ▶⏸⏭  Volume  Source    │  │
│  └────────┬─────────┘  └───────────┬──────────────┘  │
│           │                        │                  │
│           ▼                        ▼                  │
│  ┌───────────────────────────────────────────────┐   │
│  │              AudioManager                      │   │
│  │  ┌──────────┐ ┌──────────┐ ┌────────────────┐ │   │
│  │  │ TTS      │ │ Ambient  │ │ Music Playback  │ │   │
│  │  │ Playback │ │ Hum      │ │ (Queue + Mix)   │ │   │
│  │  └──────────┘ └──────────┘ └────────────────┘ │   │
│  └───────────────────┬───────────────────────────┘   │
│                      │                               │
│                      ▼                               │
│  ┌───────────────────────────────────────────────┐   │
│  │          Source Abstraction Layer              │   │
│  │                                                │   │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ │   │
│  │  │ Radio  │ │ NetEase│ │ Spotify│ │ YT/Bili│ │   │
│  │  │ Stream │ │ API    │ │ API    │ │ Extract│ │   │
│  │  └────────┘ └────────┘ └────────┘ └────────┘ │   │
│  │  ┌────────┐ ┌────────┐ ┌────────────────────┐ │   │
│  │  │ Local  │ │ TTS    │ │ AI Control Channel │ │   │
│  │  │ MP3    │ │ Server │ │ (opencc Commands)   │ │   │
│  │  └────────┘ └────────┘ └────────────────────┘ │   │
│  └───────────────────┬───────────────────────────┘   │
│                      │                               │
└──────────────────────┼───────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│              opencc CLI (AI Engine)                    │
│  ┌──────────────────────────────────────────────────┐ │
│  │  Music Agent Prompt + Tool Definitions             │ │
│  │  - play_radio(station)     - play_netease(song)  │ │
│  │  - play_spotify(track)     - play_youtube(url)   │ │
│  │  - play_bilibili(url)      - play_local(file)    │ │
│  │  - search_music(query, source)                    │ │
│  └──────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

## Audio Source Integration Matrix

| Source        | Protocol/API              | Auth Required | Stream Type  | Priority |
|---------------|---------------------------|:---:|:---:|:---:|
| TTS (kiki)    | HTTP REST (OpenAI compat) |  No | WAV/raw      | P0   |
| Local MP3     | File system               |  No | File → rodio | P0   |
| Online Radio  | Icecast/Shoutcast (MP3)   |  No | HTTP stream  | P1   |
| NetEase       | NeteaseCloudMusicApi      |  No | MP3 URL      | P1   |
| Spotify       | Web API + librespot       | OAuth | Ogg Vorbis | P2   |
| YouTube       | yt-dlp extract            |  No | MP4/AAC      | P2   |
| Bilibili      | bilibili-api              |  No | MP4/AAC      | P2   |

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Source Abstraction as Trait | 统一接口便于扩展，每个源实现 `fetch_stream() -> Result<AudioStream>` |
| AudioManager 集中调度 | 避免多源并发冲突，rodio OutputStream 只能有一个 |
| AI Control via opencc Tools | AI 通过 tool calls 控制播放，无需硬编码 UI 逻辑 |
| smol-first, no tokio | GPUI 运行在 smol executor，不能依赖 tokio |

## Errors Encountered
| Error | Resolution |
|-------|------------|
| espeak-ng 在 Windows 上 phonemization 失败 | 需要为 Windows 备选 phonemizer（如直接 Python phonemizer 子进程）|
