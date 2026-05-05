//! TTS client for local KittenTTS FastAPI backend (OpenAI-compatible endpoint).

use anyhow::{Context, Result};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const TTS_ENDPOINT: &str = "http://127.0.0.1:8005/tts";
const TTS_HEALTH_ENDPOINT: &str = "http://127.0.0.1:8005/health/ready";
const TTS_LOG_FILE: &str = r"D:\yyscode\MusicAgent\gpui-widget\musicagent-tts.log";
static TTS_WARNED_UNHEALTHY: AtomicBool = AtomicBool::new(false);

/// Available KittenTTS voices
pub const VOICES: &[&str] = &[
    "Bella", "Jasper", "Luna", "Bruno", "Rosie", "Hugo", "Kiki", "Leo",
];

fn append_tts_log(line: &str) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let msg = format!("[{}] {}\n", ts, line);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(TTS_LOG_FILE)
    {
        use std::io::Write;
        let _ = f.write_all(msg.as_bytes());
    }
}

fn force_local_no_proxy() {
    // Prevent local loopback calls from being routed through corporate/system proxies.
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    std::env::set_var("no_proxy", "127.0.0.1,localhost");
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("HTTPS_PROXY");
    std::env::remove_var("http_proxy");
    std::env::remove_var("https_proxy");
}

fn is_server_healthy() -> bool {
    force_local_no_proxy();
    match ureq::get(TTS_HEALTH_ENDPOINT)
        .header("Connection", "close")
        .call()
    {
        Ok(resp) => resp.status() == 200,
        Err(_) => false,
    }
}

fn ensure_tts_server() {
    if is_server_healthy() {
        TTS_WARNED_UNHEALTHY.store(false, Ordering::SeqCst);
        return;
    }
    if !TTS_WARNED_UNHEALTHY.swap(true, Ordering::SeqCst) {
        append_tts_log("FastAPI TTS backend is not ready at /health/ready. Start it via start.bat");
    }
}

/// Convert text to speech WAV bytes via kiki-tts-server.
///
/// Uses smol::unblock since ureq is synchronous.
pub async fn text_to_speech(text: &str, voice: &str) -> Result<Vec<u8>> {
    let text = text.to_string();
    let voice = voice.to_string();

    smol::unblock(move || -> Result<Vec<u8>> {
        force_local_no_proxy();
        let body = serde_json::json!({
            "text": text,
            "voice": voice,
            "output_format": "wav",
            "split_text": true,
            "chunk_size": 160,
            "speed": 0.94,
            "text_options": {
                "profile": "narration",
                "normalize_pause_punctuation": true,
                "pause_strength": "medium",
                "normalize_markdown": true,
                "strip_non_speakable_symbols": true
            }
        });
        let json_body = serde_json::to_string(&body)
            .context("Failed to serialize TTS request")?;

        let mut last_err: Option<anyhow::Error> = None;
        for attempt in 1..=4 {
            ensure_tts_server();
            if !is_server_healthy() {
                append_tts_log(&format!(
                    "tts send skipped attempt={} because server is not healthy yet",
                    attempt
                ));
                last_err = Some(anyhow::anyhow!("TTS server is not healthy yet"));
                std::thread::sleep(Duration::from_millis(1800));
                continue;
            }
            append_tts_log(&format!("tts send attempt={}", attempt));
            match ureq::post(TTS_ENDPOINT)
                .header("Content-Type", "application/json")
                .header("Connection", "close")
                .send(json_body.clone())
            {
                Ok(mut resp) => {
                    let mut audio_bytes = Vec::new();
                    resp.body_mut()
                        .as_reader()
                        .read_to_end(&mut audio_bytes)
                        .context("Failed to read TTS response")?;
                    append_tts_log(&format!(
                        "tts send success attempt={} bytes={}",
                        attempt,
                        audio_bytes.len()
                    ));

                    if audio_bytes.len() < 1024 {
                        append_tts_log(&format!("tts response too small: {} bytes", audio_bytes.len()));
                    }
                    return Ok(audio_bytes);
                }
                Err(e) => {
                    let err_detail = match &e {
                        ureq::Error::StatusCode(code) => format!(" status={}", code),
                        _ => String::new(),
                    };
                    append_tts_log(&format!(
                        "tts send failed attempt={} err={}{} body_len={}",
                        attempt,
                        e,
                        err_detail,
                        json_body.len()
                    ));
                    last_err = Some(anyhow::anyhow!("Failed to send TTS request: {}", e));
                    std::thread::sleep(Duration::from_millis(1500));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Failed to send TTS request")))
    })
    .await
}
