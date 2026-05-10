//! Audio playback for MusicAgent FM
//!
//! Two modes:
//! 1. Ambient: low sine wave hum.
//! 2. Music/TTS: WAV or MP3 playback.

use rodio::{Decoder, OutputStream, Sink, Source};
use std::io::Cursor;
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

/// Play TTS audio from WAV bytes.
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

pub fn is_tts_playing() -> bool {
    false
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

pub struct FftAnimation {
    pub frame_interval_ms: u64,
    pub frames: Vec<Vec<f32>>,
}

pub fn build_fft_animation(wav_bytes: &[u8], bars: usize) -> Option<FftAnimation> {
    if bars == 0 {
        return None;
    }
    let mut reader = hound::WavReader::new(Cursor::new(wav_bytes)).ok()?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return None;
    }

    let channels = spec.channels.max(1) as usize;
    let mut mono = Vec::<f32>::new();
    let mut acc = 0.0f32;
    let mut ch = 0usize;
    for s in reader.samples::<i16>() {
        let v = s.ok()? as f32 / i16::MAX as f32;
        acc += v;
        ch += 1;
        if ch == channels {
            mono.push(acc / channels as f32);
            acc = 0.0;
            ch = 0;
        }
    }
    if mono.len() < 2048 {
        return None;
    }

    let fft_size = 1024usize;
    let hop_size = ((spec.sample_rate as f32) * 0.05).max(256.0) as usize;
    let frame_interval_ms = ((hop_size as f64) * 1000.0 / spec.sample_rate as f64) as u64;
    let usable_bins = fft_size / 2;

    let mut frames = Vec::new();
    let mut pos = 0usize;
    while pos + fft_size <= mono.len() {
        let mut windowed = vec![0.0f32; fft_size];
        for i in 0..fft_size {
            let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (fft_size as f32)).cos();
            windowed[i] = mono[pos + i] * w;
        }

        let mut bars_out = vec![0.0f32; bars];
        for (bi, out) in bars_out.iter_mut().enumerate() {
            let start = bi * usable_bins / bars;
            let end = ((bi + 1) * usable_bins / bars).max(start + 1);
            let k = (start + end) / 2;
            let mut re = 0.0f32;
            let mut im = 0.0f32;
            let twopi_k = 2.0 * std::f32::consts::PI * (k as f32) / (fft_size as f32);
            for (n, x) in windowed.iter().enumerate() {
                let a = twopi_k * (n as f32);
                re += *x * a.cos();
                im -= *x * a.sin();
            }
            let avg = re * re + im * im;
            *out = (avg.sqrt() * 6.0).clamp(0.0, 1.0);
        }
        frames.push(bars_out);
        pos += hop_size;
    }

    if frames.is_empty() {
        None
    } else {
        Some(FftAnimation {
            frame_interval_ms: frame_interval_ms.max(20),
            frames,
        })
    }
}
