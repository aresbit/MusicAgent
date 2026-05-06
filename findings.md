# Findings — Melody MP3 搜索下载播放

## 核心需求
用户输入歌曲名 → 自动搜索 MP3 → 下载 → 播放（无需 API Key、无需复杂集成）

## 搜索源对比

| 源 | 稳定性 | 中文歌曲覆盖 | 需要 API Key | 实现难度 |
|----|--------|-------------|:-----------:|:-------:|
| **yt-dlp (YouTube)** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 否 | 低 |
| 网易云 API | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 否 | 中 |
| Spotify API | ⭐⭐⭐⭐ | ⭐⭐ | 是 | 高 |
| Bilibili API | ⭐⭐⭐ | ⭐⭐⭐⭐ | 否 | 中 |

**结论**: yt-dlp 是最优选择 — 免费、稳定、中文覆盖全、无需注册。

## yt-dlp 可行性验证
```bash
# 搜索歌曲
yt-dlp "ytsearch:周杰伦 七里香" --default-search "ytsearch" --flat-playlist

# 提取最佳音频
yt-dlp -f bestaudio --extract-audio --audio-format mp3 -o "output.%(ext)s" <URL>

# 获取直接音频 URL（不下载，直接获取流链接）
yt-dlp -f bestaudio -g <URL>
```

## 依赖需求
- 系统需要安装 Python + `pip install yt-dlp`
- rodio Cargo.toml 需启用 mp3 feature: `rodio = { version = "0.20", features = ["mp3"] }`

## 实现注意事项
1. yt-dlp 子进程通过 `smol::unblock` 调用，不阻塞 GPUI
2. 下载的 MP3 缓存到 `gpui-widget/music_cache/`
3. 播放通过 rodio `Sink` + `Decoder`（已有 TTS 一样的机制）
4. 文件播放和 TTS 播放需要共享同一个 OutputStream

## 用户输入触发方式
```
输入: "播放周杰伦七里香"
→ 检测 "播放" 前缀
→ 提取查询: "周杰伦七里香"
→ music_player::search_and_play("周杰伦七里香")
→ yt-dlp 搜索 → 下载 → 播放
```

## 已知限制
- 首次使用需安装 yt-dlp
- YouTube 可能被屏蔽（需代理）
- 下载需要几秒时间，缓存后可提速
- yt-dlp 版本更新可能影响参数兼容性
