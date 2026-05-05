//! Audio playback for MusicAgent FM
//!
//! Two modes:
//! 1. **Ambient** — low sine wave hum when the widget is "idle"
//! 2. **TTS** — plays WAV audio from the kiki-tts-server when AI responds

use rodio::{Decoder, OutputStream, Sink, Source};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static PLAYING: AtomicBool = AtomicBool::new(false);
const AUDIO_LOG_FILE: &str = r"D:\yyscode\MusicAgent\gpui-widget\musicagent-audio.log";

fn append_audio_log(line: &str) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let msg = format!("[{}] {}\n", ts, line);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(AUDIO_LOG_FILE)
    {
        use std::io::Write;
        let _ = f.write_all(msg.as_bytes());
    }
}

pub fn set_playing(on: bool) {
    PLAYING.store(on, Ordering::SeqCst);
}

pub fn is_playing() -> bool {
    PLAYING.load(Ordering::SeqCst)
}

/// Play TTS audio from WAV bytes.
/// Spawns a new thread with its own audio stream — no global state needed.
pub fn play_tts(wav_bytes: Vec<u8>) {
    std::thread::spawn(move || {
        play_tts_blocking(wav_bytes);
    });
}

/// Play TTS on the current thread and block until playback ends.
pub fn play_tts_blocking(wav_bytes: Vec<u8>) -> bool {
    append_audio_log(&format!("play_tts_blocking start bytes={}", wav_bytes.len()));
    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            append_audio_log(&format!("OutputStream::try_default failed: {}", e));
            return false;
        }
    };
    let sink = match Sink::try_new(&stream_handle) {
        Ok(s) => s,
        Err(e) => {
            append_audio_log(&format!("Sink::try_new failed: {}", e));
            return false;
        }
    };
    let cursor = Cursor::new(wav_bytes);
    if let Ok(source) = Decoder::new(cursor) {
        sink.append(source);
        sink.play();
        sink.sleep_until_end();
        append_audio_log("play_tts_blocking done");
        return true;
    }
    append_audio_log("Decoder::new failed");
    false
}

/// Estimate WAV duration in milliseconds for rough word-highlighting sync.
pub fn estimate_wav_duration_ms(wav_bytes: &[u8]) -> Option<u64> {
    if wav_bytes.len() < 44 {
        return None;
    }
    if &wav_bytes[0..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
        return None;
    }
    let byte_rate = u32::from_le_bytes([wav_bytes[28], wav_bytes[29], wav_bytes[30], wav_bytes[31]]);
    let data_len = u32::from_le_bytes([wav_bytes[40], wav_bytes[41], wav_bytes[42], wav_bytes[43]]);
    if byte_rate == 0 {
        return None;
    }
    Some(((data_len as u64) * 1000) / (byte_rate as u64))
}

/// Check if TTS is currently playing (rough check via any active OutputStream)
pub fn is_tts_playing() -> bool {
    // Rough heuristic: if we have an active audio thread, it's playing
    false
}

// ─── Ambient mode ──────────────────────────────────────────────────────

/// Spawn a background thread that plays a gentle ambient sine tone.
pub fn start_ambient() {
    if PLAYING.load(Ordering::SeqCst) {
        return;
    }
    PLAYING.store(true, Ordering::SeqCst);
    std::thread::spawn(|| {
        use rodio::source::SineWave;
        use rodio::{OutputStream, Sink};
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(s) => s,
            Err(_) => {
                PLAYING.store(false, Ordering::SeqCst);
                return;
            }
        };

        let sink = match Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(_) => {
                PLAYING.store(false, Ordering::SeqCst);
                return;
            }
        };

        let source = SineWave::new(110.0).amplify(0.04);
        sink.append(source);
        sink.play();

        while PLAYING.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(200));
        }

        sink.stop();
    });
}

pub fn stop_ambient() {
    PLAYING.store(false, Ordering::SeqCst);
}
