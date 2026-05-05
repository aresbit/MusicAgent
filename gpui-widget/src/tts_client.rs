//! TTS client for kiki-tts-server (OpenAI-compatible endpoint)
//!
//! Sends text to the local kiki-tts-server at localhost:8080
//! and returns WAV audio bytes for playback via rodio.

use anyhow::{Context, Result};
use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const TTS_ENDPOINT: &str = "http://127.0.0.1:8080/v1/audio/speech";
const TTS_HEALTH_ENDPOINT: &str = "http://127.0.0.1:8080/health";
const TTS_SERVER_EXE: &str = r"D:\yysdl\kiki-tts\target\release\kitten-tts-server.exe";
const TTS_MODEL_DIR: &str = r"D:\yysdl\kiki-tts\kitten-tts-mini";
const TTS_LOG_FILE: &str = r"D:\yyscode\MusicAgent\gpui-widget\musicagent-tts.log";
static TTS_BOOTED: AtomicBool = AtomicBool::new(false);

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

fn is_port_open() -> bool {
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}

fn ensure_tts_server() {
    if is_server_healthy() {
        return;
    }
    // If port is already occupied but health endpoint is not ready, avoid spawning
    // another server that would fail with Windows 10048.
    if is_port_open() {
        append_tts_log("port 8080 is in use; skip spawning duplicate server");
        for _ in 0..40 {
            if is_server_healthy() {
                append_tts_log("existing server became healthy");
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        append_tts_log("port occupied but health check failed");
        return;
    }

    // Ensure only one thread tries to spawn the server.
    let leader = TTS_BOOTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    if !leader {
        for _ in 0..40 {
            if is_server_healthy() {
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        return;
    }

    if !Path::new(TTS_SERVER_EXE).exists() {
        append_tts_log(&format!("tts server exe not found: {}", TTS_SERVER_EXE));
        TTS_BOOTED.store(false, Ordering::SeqCst);
        return;
    }
    if !Path::new(TTS_MODEL_DIR).exists() {
        append_tts_log(&format!("tts model dir not found: {}", TTS_MODEL_DIR));
        TTS_BOOTED.store(false, Ordering::SeqCst);
        return;
    }

    let spawn_res = Command::new(TTS_SERVER_EXE)
        .arg(TTS_MODEL_DIR)
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("8080")
        .spawn();
    match spawn_res {
        Ok(child) => append_tts_log(&format!("started tts server pid={}", child.id())),
        Err(e) => {
            append_tts_log(&format!("failed to start tts server: {}", e));
            TTS_BOOTED.store(false, Ordering::SeqCst);
            return;
        }
    }

    for i in 0..240 {
        if is_server_healthy() {
            append_tts_log("tts server healthy");
            return;
        }
        if i % 12 == 0 {
            append_tts_log("waiting for tts server health...");
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    append_tts_log("tts server failed to become healthy");
    TTS_BOOTED.store(false, Ordering::SeqCst);
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
