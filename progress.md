# Progress Log

## Session: 2026-05-06

### Current Status
- **Phase:** 2 — Architecture & Planning
- **Started:** 2026-05-06

### Actions Taken
- 使用 se-tool 初始化项目规划
- 设计多音频源架构 (Source Abstraction Layer)
- 设计 AudioManager 统一调度
- 调研 7 种音频源的集成方案
- 定义 AI Control Protocol (opencc Tools → 播放控制)
- 完成 task_plan.md 和 findings.md

### Implementation Priority
| Priority | Source | Effort | Dependencies |
|:--------:|--------|--------|:------------:|
| P0 | TTS (kiki) | 已有 | 修复 espeak-ng |
| P0 | Local MP3 | 低 | — |
| P1 | Online Radio | 低 | streaming crate |
| P1 | NetEase | 中 | NeteaseCloudMusicApi |
| P2 | Spotify | 高 | OAuth + librespot |
| P2 | YouTube | 中 | yt-dlp |
| P2 | Bilibili | 中 | bilibili-api |

### Architecture Diagram
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
