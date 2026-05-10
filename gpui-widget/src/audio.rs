//! Audio playback engine for MusicAgent
//!
//! Architecture:
//! - Dedicated worker thread runs a playback state machine with `rodio`.
//! - `Mutex<Inner>` + `Condvar` provides instant interrupt signaling
//!   (replacing the old 80 ms polling loop).
//! - A monotonically increasing **generation counter** (`gen`) ensures that
//!   when a new play/stop/skip bumps the counter, the worker thread detecting
//!   a mismatch knows its playback is stale and exits immediately.
//! - A `VecDeque<SongEntry>` queue provides playlist semantics: call
//!   `enqueue()` to append, `play_music()` to interrupt-and-play.
//! - `replay()` re-inserts the last finished song.

use rodio::{Decoder, OutputStream, Sink, Source};
use std::collections::VecDeque;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::Duration;

// ─── Public Types ──────────────────────────────────────────────────────────

/// A song entry with metadata and cached file path.
#[derive(Clone, Debug)]
pub struct SongEntry {
    pub title: String,
    pub url: String,
    pub file_path: PathBuf,
}

impl SongEntry {
    pub fn new(title: impl Into<String>, url: impl Into<String>, file_path: PathBuf) -> Self {
        SongEntry {
            title: title.into(),
            url: url.into(),
            file_path,
        }
    }
}

// ─── Engine Internals ──────────────────────────────────────────────────────

struct Inner {
    /// Monotonically increasing generation. Bumped on play/stop/skip.
    gen: u64,
    /// FIFO playlist queue.
    queue: VecDeque<SongEntry>,
    /// Currently playing song (None when idle).
    current: Option<SongEntry>,
    /// Last finished song, so replay() can re-insert it.
    last: Option<SongEntry>,
    /// Whether the worker is actively playing.
    playing: bool,
}

struct AudioEngine {
    inner: Mutex<Inner>,
    cvar: Condvar,
}

/// Global singleton.
///
/// # Safety
/// `AudioEngine` is **not** `Sync` (it holds a raw `Condvar`), but we only
/// reach it through this `static` — all access to mutable state goes through
/// `Mutex`, which **is** `Sync`.
static ENGINE: LazyLock<Arc<AudioEngine>> = LazyLock::new(|| {
    let engine = Arc::new(AudioEngine {
        inner: Mutex::new(Inner {
            gen: 0,
            queue: VecDeque::new(),
            current: None,
            last: None,
            playing: false,
        }),
        cvar: Condvar::new(),
    });

    // The worker keeps a weak ref so it does not prevent teardown.
    let weak = Arc::downgrade(&engine);
    std::thread::spawn(move || {
        while let Some(eng) = weak.upgrade() {
            AudioEngine::worker_loop(&eng);
        }
        log("AudioEngine worker exited");
    });

    engine
});

impl AudioEngine {
    /// Main event loop — runs on a dedicated thread.
    ///
    /// 1. Pops the front of the queue and plays it.
    /// 2. When the song finishes or is interrupted, returns to step 1.
    /// 3. If the queue is empty, blocks on the Condvar until a signal arrives.
    fn worker_loop(eng: &AudioEngine) {
        loop {
            let mut inner = eng.inner.lock().unwrap();

            if let Some(song) = inner.queue.pop_front() {
                let gen = inner.gen;
                inner.current = Some(song.clone());
                inner.playing = true;
                drop(inner);

                log(&format!("▶ Playing: {}", song.title));
                eng.play_song_to_completion(&song, gen);

                inner = eng.inner.lock().unwrap();
                inner.current = None;
                inner.playing = false;
                inner.last = Some(song);
                // Loop back to check for more songs in the queue.
                continue;
            }

            // Queue empty — park until signalled.
            inner = eng.cvar.wait(inner).unwrap();
        }
    }

    /// Block until the song finishes or `gen` changes (interrupt signal).
    fn play_song_to_completion(&self, song: &SongEntry, my_gen: u64) {
        // ---- rodio setup ----
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => {
                log(&format!("OutputStream::try_default failed: {}", e));
                return;
            }
        };
        let sink = match Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(e) => {
                log(&format!("Sink::try_new failed: {}", e));
                return;
            }
        };
        let file = match File::open(&song.file_path) {
            Ok(f) => f,
            Err(e) => {
                log(&format!("File::open failed: {}", e));
                return;
            }
        };
        let source = match Decoder::new(file) {
            Ok(s) => s,
            Err(e) => {
                log(&format!("Decoder::new failed: {}", e));
                return;
            }
        };

        sink.append(source);
        sink.play();

        // ---- Wait loop (Condvar-based, NOT polling) ----
        let mut inner = self.inner.lock().unwrap();
        while !sink.empty() {
            if inner.gen != my_gen {
                log(&format!("⏹ Interrupted: {}", song.title));
                break;
            }
            // Park with a 100 ms timeout so we periodically re-check
            // `sink.empty()`.  On stop/play/skip the Condvar is notified
            // externally, which wakes us *immediately*.
            inner = self
                .cvar
                .wait_timeout(inner, Duration::from_millis(100))
                .unwrap()
                .0;
        }

        sink.stop();
        log(&format!("■ Finished: {}", song.title));
    }
}

// ─── Public API ────────────────────────────────────────────────────────────

/// Force initialisation of the engine (lazy — happens automatically).
pub fn init() {
    LazyLock::force(&ENGINE);
}

/// Play a song **immediately**, interrupting whatever is currently playing.
///
/// The queue is cleared before inserting the new song.
pub fn play_music(song: SongEntry) {
    let eng = &**ENGINE;
    let mut inner = eng.inner.lock().unwrap();
    inner.gen += 1; // ← this is the "interrupt" signal
    inner.queue.clear();
    inner.queue.push_back(song);
    eng.cvar.notify_one();
}

/// Append a song to the end of the playlist queue.
pub fn enqueue(song: SongEntry) {
    let eng = &**ENGINE;
    let mut inner = eng.inner.lock().unwrap();
    inner.queue.push_back(song);
    eng.cvar.notify_one();
}

/// Prepend a song to the front of the queue (play next).
pub fn play_next(song: SongEntry) {
    let eng = &**ENGINE;
    let mut inner = eng.inner.lock().unwrap();
    inner.queue.push_front(song);
    eng.cvar.notify_one();
}

/// Stop playback immediately and clear the queue.
pub fn stop_music() {
    let eng = &**ENGINE;
    let mut inner = eng.inner.lock().unwrap();
    if inner.playing || !inner.queue.is_empty() {
        inner.gen += 1;
        inner.queue.clear();
        eng.cvar.notify_one();
    }
}

/// Skip to the next track (interrupts current, keeps rest of queue intact).
pub fn next_track() {
    let eng = &**ENGINE;
    let mut inner = eng.inner.lock().unwrap();
    if inner.playing || !inner.queue.is_empty() {
        inner.gen += 1;
        eng.cvar.notify_one();
    }
}

/// Replay the last finished song (interrupts current, ignores queue).
pub fn replay() {
    let eng = &**ENGINE;
    let mut inner = eng.inner.lock().unwrap();
    let last_song = inner.last.clone();
    if let Some(last) = last_song {
        inner.gen += 1;
        inner.queue.clear();
        inner.queue.push_back(last);
        eng.cvar.notify_one();
    }
}

/// Returns `true` if music is actively playing.
pub fn is_music_playing() -> bool {
    ENGINE.inner.lock().unwrap().playing
}

/// Returns `true` if music is actively playing (alias).
pub fn is_playing() -> bool {
    is_music_playing()
}

/// Currently playing song, if any.
pub fn current_song() -> Option<SongEntry> {
    ENGINE.inner.lock().unwrap().current.clone()
}

/// Number of songs remaining in the playlist queue.
pub fn queue_len() -> usize {
    ENGINE.inner.lock().unwrap().queue.len()
}

/// Clear the queue (does **not** stop current playback).
pub fn clear_queue() {
    ENGINE.inner.lock().unwrap().queue.clear();
}

/// Return a snapshot of all queued songs (does NOT include currently playing).
pub fn queue_songs() -> Vec<SongEntry> {
    ENGINE.inner.lock().unwrap().queue.iter().cloned().collect()
}

/// The most recently finished song.
pub fn last_song() -> Option<SongEntry> {
    ENGINE.inner.lock().unwrap().last.clone()
}

// ─── Internal Helpers ──────────────────────────────────────────────────────

fn log(line: &str) {
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
