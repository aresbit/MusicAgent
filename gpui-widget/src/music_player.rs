//! 搜索、下载、播放模块
//!
//! 数据源:
//! - YouTube (yt-dlp ytsearch)
//! - Bilibili (yt-dlp bilisearch)
//!
//! 播放由 `audio.rs` 中的状态机引擎管理：
//! - 可打断（新播放请求立即中断当前播放）
//! - 播放队列（多首歌顺序播放）
//! - 重播（replay 最后一首）

use crate::audio::SongEntry;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn music_log(line: &str) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let msg = format!("[{}] {}\n", ts, line);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crate::paths::log_file("music"))
    {
        use std::io::Write;
        let _ = f.write_all(msg.as_bytes());
    }
}

/// Sanitize the inherited env on a per-Command basis before invoking yt-dlp.
///
/// Two concrete problems we've seen in the wild:
/// 1. `FTP_PROXY=ftp://…` — urllib (used by yt-dlp) does not support the `ftp`
///    proxy scheme and aborts the whole HTTP request with
///    "Unsupported proxy type: ftp". We unset it on the child.
/// 2. `NO_PROXY` historically gets clobbered by this app. Instead of mutating
///    the *process* env, scope all overrides to the child Command, so the rest
///    of the app keeps the user's original settings.
fn sanitize_proxy_env(cmd: &mut Command) {
    for k in ["FTP_PROXY", "ftp_proxy", "ALL_PROXY_FTP", "all_proxy_ftp"] {
        cmd.env_remove(k);
    }
    // Make sure local addrs bypass any proxy we *do* honour.
    let no_proxy = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .unwrap_or_default();
    let mut parts: Vec<String> = no_proxy
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for needed in ["127.0.0.1", "localhost"] {
        if !parts.iter().any(|p| p == needed) {
            parts.push(needed.to_string());
        }
    }
    let merged = parts.join(",");
    cmd.env("NO_PROXY", &merged);
    cmd.env("no_proxy", &merged);
}

// ─── Search Results ────────────────────────────────────────────────────────

/// 单个搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub duration: u64,
    pub source: String, // "youtube" 或 "bilibili"
}

// ─── YouTube ───────────────────────────────────────────────────────────────

/// 使用 yt-dlp 搜索 YouTube
async fn search_youtube(query: &str) -> Result<Vec<SearchResult>> {
    let query = query.to_string();
    smol::unblock(move || -> Result<Vec<SearchResult>> {
        music_log(&format!("yt search start query='{}'", query));

        let mut cmd = Command::new("yt-dlp");
        sanitize_proxy_env(&mut cmd);
        let output = cmd
            .args([
                "--flat-playlist",
                "--dump-single-json",
                "--no-warnings",
                "--default-search",
                "ytsearch",
                &format!("ytsearch5:{}", query),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("yt-dlp not found — install with: pip install yt-dlp")?;

        if !output.stderr.is_empty() {
            let s = String::from_utf8_lossy(&output.stderr);
            for line in s.lines().take(3) {
                music_log(&format!("yt stderr: {}", line));
            }
        }
        let results = parse_ytdlp_json(&output.stdout, "youtube")?;
        music_log(&format!("yt search found {} results", results.len()));
        Ok(results)
    })
    .await
}

// ─── Bilibili ──────────────────────────────────────────────────────────────

/// 使用 yt-dlp 的 bilisearch 搜索 Bilibili
///
/// yt-dlp 内置 bilibili extractor，使用 `bilisearch5:N` 格式。
async fn search_bilibili(query: &str) -> Result<Vec<SearchResult>> {
    let query = query.to_string();
    smol::unblock(move || -> Result<Vec<SearchResult>> {
        music_log(&format!("bili search start query='{}'", query));

        let mut cmd = Command::new("yt-dlp");
        sanitize_proxy_env(&mut cmd);
        let output = cmd
            .args([
                "--flat-playlist",
                "--dump-single-json",
                "--no-warnings",
                "--default-search",
                "bilisearch",
                &format!("bilisearch5:{}", query),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("yt-dlp not found — install with: pip install yt-dlp")?;

        if !output.stderr.is_empty() {
            let s = String::from_utf8_lossy(&output.stderr);
            for line in s.lines().take(3) {
                music_log(&format!("bili stderr: {}", line));
            }
        }
        let results = parse_ytdlp_json(&output.stdout, "bilibili")?;
        music_log(&format!("bili search found {} results", results.len()));
        Ok(results)
    })
    .await
}

// ─── Shared Parser ─────────────────────────────────────────────────────────

/// 从 yt-dlp `--dump-single-json` 的输出中解析搜索列表
fn parse_ytdlp_json(raw: &[u8], source: &str) -> Result<Vec<SearchResult>> {
    let json_str = String::from_utf8_lossy(raw);
    let val: serde_json::Value =
        serde_json::from_str(&json_str).context("Failed to parse yt-dlp JSON output")?;

    let entries: &[serde_json::Value] = val
        .get("entries")
        .and_then(|e| e.as_array())
        .map_or(&[], |v| v.as_slice());

    let mut results = Vec::new();
    for entry in entries {
        let title = match entry.get("title").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => continue,
        };
        let url = entry
            .get("url")
            .or_else(|| entry.get("id"))
            .and_then(|u| u.as_str())
            .unwrap_or(title);
        let duration = entry.get("duration").and_then(|d| d.as_u64()).unwrap_or(0);
        let webpage_url = entry
            .get("webpage_url")
            .and_then(|u| u.as_str())
            .unwrap_or(url);

        results.push(SearchResult {
            title: title.to_string(),
            url: webpage_url.to_string(),
            duration,
            source: source.to_string(),
        });
    }

    Ok(results)
}

// ─── Public search ─────────────────────────────────────────────────────────

/// 同时从 **YouTube** 和 **Bilibili** 搜索，B 站结果排在后面。
pub async fn search_music(query: &str) -> Result<Vec<SearchResult>> {
    // 并行搜索两个源
    let (yt, bili) = tokio::join!(search_youtube(query), search_bilibili(query));

    let mut all: Vec<SearchResult> = match yt {
        Ok(r) => r,
        Err(e) => {
            music_log(&format!("yt search failed: {}", e));
            Vec::new()
        }
    };

    match bili {
        Ok(r) => all.extend(r),
        Err(e) => music_log(&format!("bili search failed: {}", e)),
    }

    if all.is_empty() {
        anyhow::bail!("No results from YouTube or Bilibili for: {}", query);
    }

    Ok(all)
}

// ─── Download ──────────────────────────────────────────────────────────────

/// Audio file + (best-effort) lyrics VTT path.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub audio_path: PathBuf,
    pub lyrics_path: Option<PathBuf>,
}

/// 下载音频到本地缓存，返回 MP3 文件路径以及（如有）VTT 字幕路径。
///
/// 字幕走 yt-dlp `--write-subs --write-auto-subs --convert-subs vtt`，
/// 失败不会让整个下载失败（lyrics 是可选）。
pub async fn download_audio(url: &str, cache_dir: &Path) -> Result<DownloadResult> {
    let url = url.to_string();
    let cache_dir = cache_dir.to_path_buf();

    smol::unblock(move || -> Result<DownloadResult> {
        std::fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;

        // deterministic hash-based filename
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();

        let output_template = cache_dir.join(format!("{}.%(ext)s", hash));
        let output_path = cache_dir.join(format!("{}.mp3", hash));

        let audio_path = if output_path.exists() {
            music_log(&format!("download cache hit path={}", output_path.display()));
            output_path.clone()
        } else {
            music_log(&format!("download start url='{}'", url));

            let mut cmd = Command::new("yt-dlp");
            sanitize_proxy_env(&mut cmd);
            let status = cmd
                .args([
                    "--extractor-args",
                    "youtube:player_client=android",
                    "--extract-audio",
                    "--audio-format",
                    "mp3",
                    "--audio-quality",
                    "0",
                    // Best-effort subtitle / auto-caption download.
                    "--write-subs",
                    "--write-auto-subs",
                    "--sub-langs",
                    "zh-Hans,zh-CN,zh,en,en-US,ja",
                    "--convert-subs",
                    "vtt",
                    "--no-warnings",
                    "-o",
                    output_template.to_str().unwrap(),
                    &url,
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .status()
                .context("Failed to run yt-dlp download")?;

            if !status.success() {
                anyhow::bail!("yt-dlp download failed with exit code: {:?}", status.code());
            }

            // yt-dlp may produce .mp3 or other extension — find the real file
            let actual = find_audio_file(&cache_dir, hash).unwrap_or(output_path);
            music_log(&format!("download done path={}", actual.display()));
            actual
        };

        let lyrics_path = find_lyrics_file(&cache_dir, hash);
        if let Some(ref p) = lyrics_path {
            music_log(&format!("lyrics vtt={}", p.display()));
        }
        Ok(DownloadResult { audio_path, lyrics_path })
    })
    .await
}

fn find_audio_file(dir: &Path, hash: u64) -> Option<PathBuf> {
    let prefix = format!("{}", hash);
    for entry in std::fs::read_dir(dir).ok()? {
        if let Ok(entry) = entry {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix) && name_str.ends_with(".mp3") {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Find a VTT subtitle file `{hash}.<lang>.vtt`, preferring zh* then en* then any.
fn find_lyrics_file(dir: &Path, hash: u64) -> Option<PathBuf> {
    let prefix = format!("{}.", hash);
    let mut zh: Option<PathBuf> = None;
    let mut en: Option<PathBuf> = None;
    let mut other: Option<PathBuf> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_string();
        if !(name_str.starts_with(&prefix) && name_str.ends_with(".vtt")) {
            continue;
        }
        let lang_part = &name_str[prefix.len()..name_str.len() - 4]; // strip ".vtt"
        let lower = lang_part.to_ascii_lowercase();
        if lower.starts_with("zh") {
            zh = Some(entry.path());
        } else if lower.starts_with("en") {
            en.get_or_insert(entry.path());
        } else {
            other.get_or_insert(entry.path());
        }
    }
    zh.or(en).or(other)
}

// ─── High-level actions ────────────────────────────────────────────────────

/// **搜索并立即播放**（中断当前播放）。
///
/// 流程: 搜索 → 下载 → `audio::play_music()`（可打断）。
pub async fn search_and_play(query: &str) -> Result<String> {
    music_log(&format!("search_and_play query='{}'", query));

    let results = search_music(query).await?;
    let first = &results[0];

    music_log(&format!(
        "selected [{}] title='{}' url='{}'",
        first.source, first.title, first.url
    ));

    let cache_dir = crate::paths::music_cache_dir();
    let dl = download_audio(&first.url, &cache_dir).await?;

    let song = SongEntry::new(&first.title, &first.url, dl.audio_path)
        .with_lyrics(dl.lyrics_path);
    crate::audio::play_music(song);

    Ok(format!("\u{1F3B5} Now playing: {}", first.title))
}

/// **搜索并加入播放列表队列**。
///
/// 流程: 搜索 → 下载 → `audio::enqueue()`（追加到末尾）。
pub async fn search_and_enqueue(query: &str) -> Result<String> {
    music_log(&format!("search_and_enqueue query='{}'", query));

    let results = search_music(query).await?;
    let first = &results[0];

    let cache_dir = crate::paths::music_cache_dir();
    let dl = download_audio(&first.url, &cache_dir).await?;

    let song = SongEntry::new(&first.title, &first.url, dl.audio_path)
        .with_lyrics(dl.lyrics_path);
    crate::audio::enqueue(song);

    Ok(format!("\u{1F3B5} Added to queue: {} ({} in queue)", first.title, crate::audio::queue_len()))
}

/// 重播上一首
pub fn replay() {
    crate::audio::replay();
}

/// 跳到下一首（结合队列使用）
pub fn skip() {
    crate::audio::next_track();
}
