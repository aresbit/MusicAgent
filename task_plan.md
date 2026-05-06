# Task Plan: Melody FM — MP3 搜索下载播放功能

## Goal
实现最核心的功能：用户输入歌名/歌手 → 自动搜索 MP3 → 下载 → 播放。

## 整体流程

```
用户输入 "播放周杰伦七里香"
  → music_player::search_and_play(query)
    → yt-dlp search YouTube (最佳免费音乐源)
    → 提取最佳音频流 / 下载 MP3
    → 缓存到本地
    → rodio 播放
  → UI 更新: 封面/歌名/进度条/波形
```

## 实现方案

### 方案选择：yt-dlp + rodio
- **搜索源**: YouTube（yt-dlp 子进程，最稳定免费方案）
- **下载格式**: 最佳音频质量 → 转换为 MP3/WAV
- **播放**: rodio（已有，无需新增依赖）

### 模块设计

```
gpui-widget/src/
├── main.rs           # 修改：添加音乐播放入口
├── audio.rs          # 增强：play_mp3() 文件播放
├── music_player.rs   # 新增：搜索、下载、播放编排
├── tts_client.rs     # 不变
└── cli_client.rs     # 不变
```

### music_player.rs 设计

```rust
// 搜索结构体
struct MusicSearchResult {
    title: String,
    url: String,       // yt-dlp 提取的音频直链
    duration: u64,     // 秒
}

// 核心函数
async fn search_music(query: &str) -> Result<Vec<MusicSearchResult>>
async fn download_audio(url: &str, cache_dir: &Path) -> Result<PathBuf>
async fn play_music_file(path: &PathBuf) -> Result<()>
```

### 用户输入处理
两种路径触发音乐播放：

1. **快捷命令**: 输入框以 "播放"/"放"/"play" 开头 → 直接调用 music_player
2. **AI 解释**: 发给 opencc → AI 回复 → 解析出歌曲请求 → 调用 music_player

### 依赖
- `yt-dlp` 需要安装在系统 PATH 中（`pip install yt-dlp`）
- rodio 已支持 MP3 解码（需 feature `mp3`）

## 阶段

### Phase 1: 基础模块
- [ ] 创建 `music_player.rs`（search + download）
- [ ] 扩展现有 `audio.rs`（play_mp3_file + play_music_stream）
- [ ] 集成到 `main.rs`（检测 "播放" 前缀命令）
- [ ] 验证 yt-dlp 调用 + 音频播放完整链路

### Phase 2: UI 增强
- [ ] 显示正在播放的信息（歌名、歌手）
- [ ] 播放进度条
- [ ] 波形显示
- [ ] 播放/暂停控制

### Phase 3: 缓存与体验
- [ ] 本地缓存（避免重复下载）
- [ ] 播放队列（搜索 → 加入播放列表）
- [ ] AI 集成（让 AI 回复包含播放指令）

## 技术决策
| 决策 | 原因 |
|------|------|
| yt-dlp 作为搜索源 | 免费、稳定、中文歌曲覆盖全 |
| smol::unblock 调用 yt-dlp | 避免 tokio 依赖，兼容 GPUI |
| MP3 文件先下载再播放 | rodio 流式播放不稳定 |
| 缓存目录 `gpui-widget/music_cache/` | 统一管理，可手动清理 |
