//! Audio playback for MusicAgent FM
//!
//! Two modes:
//! 1. Ambient: low sine wave hum.
//! 2. Music: MP3 playback.

use rodio::{Decoder, OutputStream, Sink, Source};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static AMBIENT_PLAYING: AtomicBool = AtomicBool::new(false);
static MUSIC_PLAYING: AtomicBool = AtomicBool::new(false);
static STOP_MUSIC_REQUESTED: AtomicBool = AtomicBool::new(false);

fn append_audio_log(line: &str) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let msg = format!("[{}] {}\n", ts, line);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crate::paths::log_file("audio"))
    {
        use std::io::Write;
        let _ = f.write_all(msg.as_bytes());
    }
}

pub fn set_playing(on: bool) {
    MUSIC_PLAYING.store(on, Ordering::SeqCst);
}

pub fn is_playing() -> bool {
    MUSIC_PLAYING.load(Ordering::SeqCst) || AMBIENT_PLAYING.load(Ordering::SeqCst)
}

pub fn is_music_playing() -> bool {
    MUSIC_PLAYING.load(Ordering::SeqCst)
}

pub fn stop_music() {
    STOP_MUSIC_REQUESTED.store(true, Ordering::SeqCst);
}

/// Play an MP3 file and block until playback ends or stop is requested.
pub fn play_mp3_file_blocking(path: &std::path::Path) -> bool {
    append_audio_log(&format!("play_mp3_file_blocking path={}", path.display()));
    STOP_MUSIC_REQUESTED.store(false, Ordering::SeqCst);
    MUSIC_PLAYING.store(true, Ordering::SeqCst);

    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            append_audio_log(&format!("OutputStream::try_default failed: {}", e));
            MUSIC_PLAYING.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let sink = match Sink::try_new(&stream_handle) {
        Ok(s) => s,
        Err(e) => {
            append_audio_log(&format!("Sink::try_new failed: {}", e));
            MUSIC_PLAYING.store(false, Ordering::SeqCst);
            return false;
        }
    };

    let ok = match std::fs::File::open(path) {
        Ok(file) => match Decoder::new(file) {
            Ok(source) => {
                sink.append(source);
                sink.play();
                while !sink.empty() {
                    if STOP_MUSIC_REQUESTED.load(Ordering::SeqCst) {
                        append_audio_log("play_mp3_file_blocking stop requested");
                        sink.stop();
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(80));
                }
                true
            }
            Err(e) => {
                append_audio_log(&format!("Decoder::new from file failed: {}", e));
                false
            }
        },
        Err(e) => {
            append_audio_log(&format!("File::open failed: {}", e));
            false
        }
    };

    MUSIC_PLAYING.store(false, Ordering::SeqCst);
    STOP_MUSIC_REQUESTED.store(false, Ordering::SeqCst);
    append_audio_log("play_mp3_file_blocking done");
    ok
}

pub fn start_ambient() {
    if AMBIENT_PLAYING.load(Ordering::SeqCst) {
        return;
    }
    AMBIENT_PLAYING.store(true, Ordering::SeqCst);
    std::thread::spawn(|| {
        use rodio::source::SineWave;
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(s) => s,
            Err(_) => {
                AMBIENT_PLAYING.store(false, Ordering::SeqCst);
                return;
            }
        };

        let sink = match Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(_) => {
                AMBIENT_PLAYING.store(false, Ordering::SeqCst);
                return;
            }
        };

        let source = SineWave::new(110.0).amplify(0.04);
        sink.append(source);
        sink.play();

        while AMBIENT_PLAYING.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(200));
        }

        sink.stop();
    });
}

pub fn stop_ambient() {
    AMBIENT_PLAYING.store(false, Ordering::SeqCst);
}

