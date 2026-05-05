//! TTS client for kiki-tts-server (OpenAI-compatible endpoint)
//!
//! Sends text to the local kiki-tts-server at localhost:8080
//! and returns WAV audio bytes for playback via rodio.

use anyhow::{Context, Result};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const TTS_ENDPOINT: &str = "http://127.0.0.1:8005/v1/audio/speech";
const TTS_HEALTH_ENDPOINT: &str = "http://127.0.0.1:8005/health/ready";
const TTS_LOG_FILE: &str = r"D:\yyscode\MusicAgent\gpui-widget\musicagent-tts.log";
static TTS_WARNED_UNHEALTHY: AtomicBool = AtomicBool::new(false);

/// Available kiki-tts voices
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

fn is_server_healthy() -> bool {
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
        let body = serde_json::json!({
            "model": "kitten-tts-mini",
            "input": text,
            "voice": voice,
            "response_format": "wav",
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

                    if audio_bytes.len() < 1024 {
                        append_tts_log(&format!("tts response too small: {} bytes", audio_bytes.len()));
                    }
                    return Ok(audio_bytes);
                }
                Err(e) => {
                    append_tts_log(&format!(
                        "tts send failed attempt={} err={} body_len={}",
                        attempt,
                        e,
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
