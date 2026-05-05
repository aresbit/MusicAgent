# Findings & Decisions — MusicAgent 多音频源架构

## Requirements
1. **7 种音频源**: opencc TTS、在线电台、网易云、Spotify、YouTube、Bilibili、本地 MP3
2. **AI 驱动**: opencc CLI 作为大脑，通过 tool calls 控制播放
3. **GPUI 桌面 Widget**: 现有 Rust GPUI 界面保持不变
4. **Windows 优先**: 所有源必须能在 Windows 上运行

## Research Findings

### 现有架构
- GPUI widget (main.rs + audio.rs) — smol executor, rodio 播放
- opencc CLI (packages/opencc/) — AI 引擎, DeepSeek API
- kiki-tts Server (D:\yysdl\kiki-tts) — TTS 服务, 但 Windows 上 espeak-ng 不可用

### 技术约束
| 约束 | 影响 |
|------|------|
| GPUI 使用 smol (非 tokio) | 所有异步代码必须兼容 smol |
| rodio OutputStream 唯一 | 需要 AudioManager 集中调度 |
| Windows 环境 | 许多 Linux 音频工具/库不可用 |
| 跨域/CORS | 浏览器 API 调用需 proxy |

### 各音频源集成方案

#### 1. 在线电台 (Radio)
- **协议**: Icecast/Shoutcast HTTP MP3 stream
- **Rust 库**: `streaming` (从 HTTP 流读取音频数据)
- **实现**: rodio 支持从 `Read` 读取 MP3，直接流式播放
- **电台列表**: 预配置 + opencc 动态推荐
- **示例**: 中国国际广播电台、CRI、HitFM 等

#### 2. 网易云音乐 (NetEase)
- **社区 API**: https://github.com/Binaryify/NeteaseCloudMusicApi (Node.js 服务)
- **接口**: `/search`, `/song/url`, `/lyric`
- **实现**: Rust HTTP 客户端 → 启动外部 NeteaseCloudMusicApi Node 服务
- **播放**: 获取 MP3 URL → 下载/流式传给 rodio
- **搜索**: 通过 opencc AI 调用 → 调用 API → 返回结果

#### 3. Spotify
- **Web API**: `GET /v1/search`, `GET /v1/tracks/{id}`
- **OAuth**: Device Code Flow (无需跳转浏览器)
- **流媒体**: librespot (Rust 库，非官方)
- **简化方案**: 先用 Web API 搜索，播放 30 秒预览片段 (preview_url)
- **完整方案**: 集成 librespot-rs 播放高质量流

#### 4. YouTube
- **提取**: yt-dlp (Python CLI) 或 youtube-dl-rs (Rust 库)
- **流程**: 搜索/输入 URL → 提取音频流 URL → rodio 播放
- **方案**: 调用 yt-dlp 子进程 (`smol::unblock`) 获取直接音频 URL
- **替代**: 使用 youtube-dl-rs (纯 Rust，但功能有限)

#### 5. Bilibili
- **API**: bilibili-api (社区维护的 Rust crate)
- **流程**: 搜索 BV 号/关键词 → 获取视频页 → 提取音频流
- **方案**: HTTP 请求模拟 bilibili API → 解析音频流地址
- **注意**: Bilibili 反爬，可能需要 cookie

#### 6. 本地 MP3
- **方案**: std::fs → rodio::Decoder
- **UI**: 文件选择器（GPUI 原生对话框）或拖拽
- **扫描**: 配置目录自动扫描（`walkdir` crate）

### AudioManager 设计
```
AudioManager {
    current_source: AudioSourceKind,
    tts_sink: Option<Sink>,
    music_sink: Option<Sink>,
    ambient_sink: Option<Sink>,
    queue: VecDeque<PlaylistItem>,
    
    fn play(source: AudioSourceKind, data: AudioData) -> Result<()>;
    fn pause() -> Result<()>;
    fn resume() -> Result<()>;
    fn stop() -> Result<()>;
    fn set_volume(level: f32) -> Result<()>;
    fn enqueue(item: PlaylistItem) -> Result<()>;
    fn skip() -> Result<PlaylistItem>;
}
```

### AudioSource Trait
```rust
#[async_trait]
pub trait AudioSource {
    fn kind(&self) -> AudioSourceKind;
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>>;
    async fn resolve(&self, id: &str) -> Result<AudioData>;
    fn name(&self) -> &'static str;
}
```

### AI Control Protocol
opencc AI 通过 Tool Calls 发送播放命令：
```json
{
  "tool": "play_radio",
  "args": { "station": "hitfm" }
}
{
  "tool": "search_music",
  "args": { "source": "netease", "query": "周杰伦 七里香" }
}
{
  "tool": "play_spotify",
  "args": { "track_id": "..." }
}
```

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| Source Abstraction Layer via Trait | 统一接口，新源只需实现 trait |
| AudioManager 单例调度 | rodio 只能有一个 OutputStream |
| 外部 API 服务用子进程启动 | 避免 Rust 绑定复杂度和 tokio 依赖 |
| yt-dlp 子进程提取流 | 最稳定的 YouTube 解决方案 |
| NeteaseCloudMusicApi 作为独立 Node 服务 | 社区维护最活跃，API 最全 |
| 不直接依赖 tokio | GPUI smol executor 不兼容 tokio |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| espeak-ng Windows DLL 缺失 | 备选: Python phonemizer 子进程 / g2p-en 纯 Rust 库 |
| rodio MP3 流式播放不稳定 | 可能需要先下载完整文件再播放 |
| Spotify OAuth 需要本地服务器 | 需要嵌入 HTTP server (tiny_http) |

## Resources
- NeteaseCloudMusicApi: https://github.com/Binaryify/NeteaseCloudMusicApi
- librespot: https://github.com/librespot-org/librespot
- yt-dlp: https://github.com/yt-dlp/yt-dlp
- rodio: https://github.com/RustAudio/rodio
- bilibili-api (Rust): https://crates.io/crates/bilibili
