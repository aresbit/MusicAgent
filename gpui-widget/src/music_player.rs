//! MP3 搜索、下载、播放模块
//!
//! 使用 yt-dlp 子进程搜索 YouTube 并下载音频，
//! 使用 rodio 播放本地文件。

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn append_music_log(line: &str) {
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

fn force_local_no_proxy() {
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    std::env::set_var("no_proxy", "127.0.0.1,localhost");
}

/// 搜索歌曲，返回标题和音频 URL 列表
pub async fn search_music(query: &str) -> Result<Vec<SearchResult>> {
    let query = query.to_string();
    smol::unblock(move || -> Result<Vec<SearchResult>> {
        force_local_no_proxy();
        append_music_log(&format!("search start query='{}'", query));

        // yt-dlp 搜索：最多返回 5 个结果
        let output = Command::new("yt-dlp")
            .args([
                "--flat-playlist",
                "--dump-single-json",
                "--no-warnings",
                "--default-search",
                "ytsearch5",
                &format!("ytsearch5:{}", query),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to run yt-dlp. Is it installed? Try: pip install yt-dlp")?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        append_music_log(&format!(
            "search stderr='{}'",
            stderr.chars().take(200).collect::<String>()
        ));

        // yt-dlp --flat-playlist --dump-single-json 输出 JSON
        // 如果搜索失败，尝试解析 JSON 中的 entries
        let stdout = String::from_utf8_lossy(&output.stdout);
        let results = parse_ytdlp_search_results(&stdout, &query)?;
        append_music_log(&format!("search found {} results", results.len()));
        Ok(results)
    })
    .await
}

/// 从 yt-dlp JSON 输出中解析搜索结果
fn parse_ytdlp_search_results(json_str: &str, query: &str) -> Result<Vec<SearchResult>> {
    // 尝试解析 JSON
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(entries) = val.get("entries").and_then(|e| e.as_array()) {
            let mut results = Vec::new();
            for entry in entries {
                if let (Some(title), Some(url)) = (
                    entry.get("title").and_then(|t| t.as_str()),
                    entry
                        .get("url")
                        .or_else(|| entry.get("id"))
                        .and_then(|u| u.as_str()),
                ) {
                    let duration = entry.get("duration").and_then(|d| d.as_u64()).unwrap_or(0);
                    let webpage_url = entry
                        .get("webpage_url")
                        .and_then(|u| u.as_str())
                        .unwrap_or(url);
                    results.push(SearchResult {
                        title: title.to_string(),
                        url: webpage_url.to_string(),
                        duration,
                    });
                }
            }
            if !results.is_empty() {
                return Ok(results);
            }
        }
    }

    // 降级：直接构造 ytsearch URL（fallback）
    Ok(vec![SearchResult {
        title: query.to_string(),
        url: format!("ytsearch:{}", query),
        duration: 0,
    }])
}

/// 搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub duration: u64,
}

/// 下载音频到本地缓存
pub async fn download_audio(url: &str, cache_dir: &Path) -> Result<PathBuf> {
    let url = url.to_string();
    let cache_dir = cache_dir.to_path_buf();

    smol::unblock(move || -> Result<PathBuf> {
        force_local_no_proxy();
        std::fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;

        // 使用 URL 的 hash 作为文件名避免特殊字符问题
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();

        let output_template = cache_dir.join(format!("{}.%(ext)s", hash));
        let output_path = cache_dir.join(format!("{}.mp3", hash));

        // 如果已经缓存过了，直接返回
        if output_path.exists() {
            append_music_log(&format!("download cache hit path={}", output_path.display()));
            return Ok(output_path);
        }

        append_music_log(&format!("download start url='{}'", url));

        let status = Command::new("yt-dlp")
            .args([
                "--extractor-args",
                "youtube:player_client=android",
                "--extract-audio",
                "--audio-format",
                "mp3",
                "--audio-quality",
                "0",
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

        // 查找实际输出的 mp3 文件（yt-dlp 可能输出 .mp3 或其他扩展名）
        let actual = find_mp3_in_dir(&cache_dir, hash).unwrap_or(output_path.clone());
        append_music_log(&format!("download done path={}", actual.display()));
        Ok(actual)
    })
    .await
}

fn find_mp3_in_dir(dir: &Path, hash: u64) -> Option<PathBuf> {
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

/// 搜索并播放（对外主入口）
pub async fn search_and_play(query: &str) -> Result<String> {
    append_music_log(&format!("search_and_play query='{}'", query));

    // 1. 搜索
    let results = search_music(query).await?;
    if results.is_empty() {
        anyhow::bail!("No results found for: {}", query);
    }

    let first = &results[0];
    append_music_log(&format!(
        "selected result: title='{}' url='{}'",
        first.title, first.url
    ));

    // 2. 下载
    let cache_dir = crate::paths::music_cache_dir();
    let file_path = download_audio(&first.url, &cache_dir).await?;

    // 3. 播放（异步，后台播放）
    let title = first.title.clone();
    smol::spawn(async move {
        append_music_log(&format!("playback start path={}", file_path.display()));
        let _ = smol::unblock(move || crate::audio::play_mp3_file_blocking(&file_path)).await;
        append_music_log("playback done");
    })
    .detach();

    Ok(format!("\u{1F3B5} Now playing: {}", title))
}
