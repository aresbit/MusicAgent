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
use rustfft::{num_complex::Complex32, FftPlanner};
use std::collections::VecDeque;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant};

// ─── Spectrum (FFT) ────────────────────────────────────────────────────────

/// Number of output bars exposed to the UI.
pub const SPECTRUM_BARS: usize = 60;
/// Power-of-two FFT window. 1024 samples ≈ 23 ms @ 44.1 kHz — feels responsive.
const FFT_SIZE: usize = 1024;

/// Log-spaced frequency-bin magnitudes (length = SPECTRUM_BARS, range 0.0..=1.0).
/// Bar i represents a frequency band; left = bass, right = treble.
static SPECTRUM_FREQ: LazyLock<Mutex<Vec<f32>>> =
    LazyLock::new(|| Mutex::new(vec![0.0; SPECTRUM_BARS]));

/// Rolling time-domain peak intensity (length = SPECTRUM_BARS).
/// Index 0 = oldest sample (leftmost), last = newest (rightmost) — visualised
/// as a scrolling "EM wave" / oscilloscope trace.
static SPECTRUM_TIME: LazyLock<Mutex<Vec<f32>>> =
    LazyLock::new(|| Mutex::new(vec![0.0; SPECTRUM_BARS]));

/// Snapshot of the frequency-band spectrum.
pub fn current_spectrum() -> Vec<f32> {
    SPECTRUM_FREQ.lock().unwrap().clone()
}

/// Snapshot of the time-rolling intensity history.
pub fn current_waveform_history() -> Vec<f32> {
    SPECTRUM_TIME.lock().unwrap().clone()
}

fn clear_spectrum() {
    for store in [&*SPECTRUM_FREQ, &*SPECTRUM_TIME] {
        if let Ok(mut s) = store.lock() {
            for v in s.iter_mut() {
                *v = 0.0;
            }
        }
    }
}

/// Source wrapper that taps samples for a rolling-amplitude visualization.
///
/// The original implementation rendered a frequency spectrum (one bar per
/// frequency band) which felt static: bass bins on the left barely moved while
/// transients shoved the high-band bins on the right. The user expected an
/// oscilloscope-style waveform where new audio events enter on the right and
/// scroll leftward over time, similar to a radar trace or EM wave readout.
///
/// We now compute one "intensity" value per FFT window — the peak FFT bin
/// magnitude with dB normalization and a power-curve boost — and push it onto
/// the right end of a fixed-size ring buffer. The UI reads the buffer as-is:
/// index 0 is the oldest sample (leftmost), index N-1 is the newest
/// (rightmost). With ~21 publishes/sec the bars visibly scroll left across
/// the screen as music plays.
struct TapSource<S>
where
    S: Source<Item = i16>,
{
    inner: S,
    /// Local rolling buffer of mono samples (f32, range −1.0..=1.0).
    buf: Vec<f32>,
    /// Channel cursor for downmixing interleaved frames.
    chan_idx: u16,
    chan_acc: f32,
    /// Cached FFT instance — building it once is cheaper than per-window.
    planner: Arc<dyn rustfft::Fft<f32>>,
    /// Cached channel count (constant per source).
    channels: u16,
    /// Per-band smoothing state for the frequency spectrum (asymmetric
    /// attack/release for snappy visualisation).
    freq_smoothed: Vec<f32>,
    /// FFT window counter — only every Nth window publishes to the time-
    /// rolling buffer so the scroll speed stays ~21 Hz instead of ~86.
    window_counter: usize,
    /// Peak intensity accumulator across skipped time-buffer windows.
    pending_peak: f32,
}

impl<S> TapSource<S>
where
    S: Source<Item = i16>,
{
    fn new(inner: S) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let channels = inner.channels().max(1);
        TapSource {
            inner,
            buf: Vec::with_capacity(FFT_SIZE),
            chan_idx: 0,
            chan_acc: 0.0,
            planner: fft,
            channels,
            freq_smoothed: vec![0.0; SPECTRUM_BARS],
            window_counter: 0,
            pending_peak: 0.0,
        }
    }

    fn push_sample(&mut self, sample: i16) {
        // Downmix interleaved channels into a single mono sample.
        self.chan_acc += sample as f32 / 32768.0;
        self.chan_idx += 1;
        if self.chan_idx >= self.channels {
            let mono = self.chan_acc / self.channels as f32;
            self.chan_idx = 0;
            self.chan_acc = 0.0;
            self.buf.push(mono);
            if self.buf.len() >= FFT_SIZE {
                self.process_window();
                // Slide window by half-length so consecutive windows overlap
                // 50% — same as the original spectrum implementation.
                let drop_n = FFT_SIZE / 2;
                self.buf.drain(..drop_n);
            }
        }
    }

    /// Run FFT on the current buffered window and update BOTH visualisations:
    ///  - SPECTRUM_FREQ: log-spaced per-band magnitudes with smoothing.
    ///  - SPECTRUM_TIME: one rolling intensity sample per ~4 windows.
    fn process_window(&mut self) {
        let mut data: Vec<Complex32> = Vec::with_capacity(FFT_SIZE);
        for (i, &s) in self.buf.iter().take(FFT_SIZE).enumerate() {
            let w = 0.5 - 0.5
                * ((2.0 * std::f32::consts::PI * i as f32) / (FFT_SIZE as f32 - 1.0)).cos();
            data.push(Complex32::new(s * w, 0.0));
        }
        self.planner.process(&mut data);

        let half = FFT_SIZE / 2;
        let mags: Vec<f32> = data[..half].iter().map(|c| c.norm()).collect();

        // ── Frequency spectrum: log-spaced bins, dB scaling, smoothing. ──
        let min_bin = 1.0f32;
        let max_bin = half as f32;
        let mut bars = vec![0.0f32; SPECTRUM_BARS];
        for b in 0..SPECTRUM_BARS {
            let lo_f = min_bin * (max_bin / min_bin).powf(b as f32 / SPECTRUM_BARS as f32);
            let hi_f = min_bin * (max_bin / min_bin).powf((b + 1) as f32 / SPECTRUM_BARS as f32);
            let lo = (lo_f as usize).max(1);
            let hi = (hi_f as usize).max(lo + 1).min(half);
            let mut bin_peak = 0.0f32;
            for &m in &mags[lo..hi] {
                if m > bin_peak {
                    bin_peak = m;
                }
            }
            let db = 20.0 * (bin_peak + 1e-6).log10();
            // -55..-5 dB → 0..1, then sqrt() lift for visual punch.
            let norm = ((db + 55.0) / 50.0).clamp(0.0, 1.0);
            bars[b] = norm.sqrt();
        }
        // Asymmetric attack/release smoothing per bar.
        for (i, target) in bars.iter().enumerate() {
            let prev = self.freq_smoothed[i];
            let next = if *target > prev {
                prev + (*target - prev) * 0.55
            } else {
                prev + (*target - prev) * 0.20
            };
            self.freq_smoothed[i] = next.clamp(0.0, 1.0);
        }
        if let Ok(mut out) = SPECTRUM_FREQ.lock() {
            out.clone_from(&self.freq_smoothed);
        }

        // ── Time-rolling intensity: one bar per ~4 FFT windows. ──
        let mut wave_peak = 0.0f32;
        for &m in &mags[1..half] {
            if m > wave_peak {
                wave_peak = m;
            }
        }
        let db = 20.0 * (wave_peak + 1e-6).log10();
        let intensity = ((db + 45.0) / 40.0).clamp(0.0, 1.0).sqrt();

        if intensity > self.pending_peak {
            self.pending_peak = intensity;
        }
        self.window_counter += 1;
        const PUBLISH_EVERY: usize = 4;
        if self.window_counter % PUBLISH_EVERY == 0 {
            let value = self.pending_peak;
            self.pending_peak = 0.0;
            if let Ok(mut out) = SPECTRUM_TIME.lock() {
                if !out.is_empty() {
                    out.rotate_left(1);
                    if let Some(last) = out.last_mut() {
                        *last = value;
                    }
                }
            }
        }
    }
}

impl<S> Iterator for TapSource<S>
where
    S: Source<Item = i16>,
{
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        let s = self.inner.next()?;
        self.push_sample(s);
        Some(s)
    }
}

impl<S> Source for TapSource<S>
where
    S: Source<Item = i16>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

// ─── Public Types ──────────────────────────────────────────────────────────

/// A song entry with metadata and cached file path.
#[derive(Clone, Debug)]
pub struct SongEntry {
    pub title: String,
    pub url: String,
    pub file_path: PathBuf,
    /// Optional VTT lyrics file path (yt-dlp may produce this).
    pub lyrics_path: Option<PathBuf>,
}

impl SongEntry {
    pub fn new(title: impl Into<String>, url: impl Into<String>, file_path: PathBuf) -> Self {
        SongEntry {
            title: title.into(),
            url: url.into(),
            file_path,
            lyrics_path: None,
        }
    }

    /// Builder-style: attach an optional VTT lyrics file path.
    pub fn with_lyrics(mut self, lyrics_path: Option<PathBuf>) -> Self {
        self.lyrics_path = lyrics_path;
        self
    }
}

// ─── Engine Internals ──────────────────────────────────────────────────────

/// Cap on the play-history ring buffer.
const MAX_HISTORY: usize = 20;

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
    /// Wall-clock instant the current track started playing (for lyrics sync).
    play_started_at: Option<Instant>,
    /// Recently played tracks (newest first), deduplicated by url.
    history: VecDeque<SongEntry>,
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
            play_started_at: None,
            history: VecDeque::new(),
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
                inner.play_started_at = Some(Instant::now());
                drop(inner);

                log(&format!("▶ Playing: {}", song.title));
                eng.play_song_to_completion(&song, gen);

                inner = eng.inner.lock().unwrap();
                inner.current = None;
                inner.playing = false;
                inner.play_started_at = None;
                inner.last = Some(song.clone());
                // Push to history (newest-first, dedup by url, cap MAX_HISTORY).
                inner.history.retain(|s| s.url != song.url);
                inner.history.push_front(song);
                while inner.history.len() > MAX_HISTORY {
                    inner.history.pop_back();
                }
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

        // Wrap the decoder so we can tap PCM samples for real-time FFT.
        let tapped = TapSource::new(source);
        sink.append(tapped);
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
        clear_spectrum();
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

/// Elapsed time since the current track started playing.
///
/// Returns None when idle. Approximate — assumes no gaps from CPU stalls or
/// PCM buffer underruns, which is more than precise enough for line-level
/// lyric sync (cues are typically multiple seconds long).
pub fn current_position() -> Option<Duration> {
    let inner = ENGINE.inner.lock().unwrap();
    if inner.playing {
        inner.play_started_at.map(|t| t.elapsed())
    } else {
        None
    }
}

/// Remove the song at the given queue index (does NOT affect the currently
/// playing track). Returns true on success.
pub fn remove_from_queue(index: usize) -> bool {
    let eng = &**ENGINE;
    let mut inner = eng.inner.lock().unwrap();
    if index < inner.queue.len() {
        inner.queue.remove(index);
        true
    } else {
        false
    }
}

/// Move the queued song at `index` to the front of the queue so it plays next.
/// No-op if index == 0 or out of range. Does NOT interrupt current playback.
pub fn pin_to_top(index: usize) -> bool {
    let eng = &**ENGINE;
    let mut inner = eng.inner.lock().unwrap();
    if index == 0 || index >= inner.queue.len() {
        return false;
    }
    if let Some(song) = inner.queue.remove(index) {
        inner.queue.push_front(song);
        true
    } else {
        false
    }
}

/// Snapshot of recently played tracks (newest first, cap = MAX_HISTORY).
pub fn history_songs() -> Vec<SongEntry> {
    ENGINE.inner.lock().unwrap().history.iter().cloned().collect()
}

/// Play a specific entry from the history (interrupts current playback).
/// Returns true if the index was valid.
pub fn play_from_history(index: usize) -> bool {
    let song = {
        let inner = ENGINE.inner.lock().unwrap();
        inner.history.get(index).cloned()
    };
    match song {
        Some(s) => {
            play_music(s);
            true
        }
        None => false,
    }
}

/// Randomly shuffle the queue (Fisher-Yates). Currently playing track is
/// untouched. Returns the new queue length.
pub fn shuffle_queue() -> usize {
    use rand::seq::SliceRandom;
    let eng = &**ENGINE;
    let mut inner = eng.inner.lock().unwrap();
    let mut v: Vec<SongEntry> = inner.queue.drain(..).collect();
    v.shuffle(&mut rand::thread_rng());
    inner.queue.extend(v);
    inner.queue.len()
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
