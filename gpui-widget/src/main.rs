// Melody FM GPUI Widget - Private AI Radio Desktop Widget
// Connects to opencc CLI.js for AI-powered music conversations

use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement;
use chrono::Local;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::time::{Duration, Instant};

mod audio;
mod cli_client;
mod lyrics;
mod music_player;
mod paths;

fn append_flow_log(line: &str) {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let msg = format!("[{}] {}\n", ts, line);
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(crate::paths::log_file("flow"))
    {
        use std::io::Write;
        let _ = f.write_all(msg.as_bytes());
    }
}

// ── Preset questions ───────────────────────────────────────────────────
const PRESET_QUESTIONS: &[&str] = &[
    "Recommend some late-night jazz piano albums",
    "What makes Miles Davis's Kind of Blue so influential?",
    "Explain the Dorian mode with a musical example",
    "Tell me an interesting story about a classical composer",
];

// ── Entry point ─────────────────────────────────────────────────────────
fn main() {
    Application::new().run(move |cx| {
        gpui_component::init(cx);

        cx.spawn(async move |cx| {
            let bounds = Bounds {
                origin: Point {
                    x: px(400.0),
                    y: px(100.0),
                },
                size: gpui::Size {
                    width: px(520.0),
                    height: px(860.0),
                },
            };

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(gpui::Size {
                        width: px(460.0),
                        height: px(760.0),
                    }),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Melody FM".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    kind: WindowKind::Normal,
                    focus: true,
                    show: true,
                    is_movable: true,
                    is_resizable: true,
                    is_minimizable: true,
                    display_id: None,
                    ..Default::default()
                },
                |window, cx| {
                    // Create text input state first (needs &mut Window)
                    let input_state = cx.new(|cx| InputState::new(window, cx));
                    // Create the app view, passing in input_state
                    let view = cx.new(|cx| MusicAgentApp::new(input_state.clone(), window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("Failed to open window");
        })
        .detach();
    });
}

// ── App state ──────────────────────────────────────────────────────────
struct MusicAgentApp {
    status: String,
    is_playing: bool,
    current_time: String,
    turns: Vec<Turn>,
    clock: String,
    current_turn_index: usize,
    current_word_index: usize,
    highlight_active: bool,
    /// Frequency spectrum (60 bars, low → high). Drives the big header bars.
    waveform_bars: Vec<f32>,
    /// Time-rolling intensity history (60 bars, oldest → newest). Drives the
    /// small player_bars row at the bottom — scrolls right→left.
    player_bars_history: Vec<f32>,
    is_asking: bool,
    fluid_hue: f32, // 0.0-360.0 hue for animated gradient background
    clear_input: bool,
    input_state: Entity<InputState>,
    _text_sub: Subscription,
    /// Per-queued-song first-seen timestamps for opacity ramp on insert.
    queue_first_seen: HashMap<String, Instant>,
    /// Parsed VTT for currently playing song (None if unavailable).
    current_lyrics: Option<lyrics::Lyrics>,
    /// URL of song whose lyrics we last parsed, so we know when to refresh.
    current_lyrics_url: Option<String>,
    /// Cached current lyric line text.
    current_lyric_text: Option<String>,
}

struct Turn {
    who: String,
    time: String,
    text: String,
}

impl MusicAgentApp {
    fn new(input_state: Entity<InputState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Subscribe to Enter key press on the input field
        let is = input_state.clone();
        let text_sub = cx.subscribe(&input_state, move |this: &mut MusicAgentApp, _emitter: Entity<InputState>, event: &InputEvent, cx: &mut Context<MusicAgentApp>| {
            if let InputEvent::PressEnter { .. } = event {
                let question = is.read(cx).value().to_string();
                if !question.is_empty() {
                    this.clear_input = true;
                    this.ask_question(&question, cx);
                }
            }
        });

        // Start the fluid hue animation timer
        let async_cx = cx.to_async();
        cx.spawn(
            |this: WeakEntity<Self>, _cx: &mut AsyncApp| async move {
            let cx = async_cx;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                this.update(&mut cx.clone(), |this, cx| {
                    this.fluid_hue = (this.fluid_hue + 0.5) % 360.0;
                    this.refresh_queue_anim();
                    this.refresh_lyrics();
                    let music_on = audio::is_music_playing();
                    if music_on {
                        this.is_playing = true;
                        if !this.highlight_active {
                            // Top: frequency spectrum (each bar = one band).
                            // Light UI-side smoothing on top of audio-thread
                            // smoothing keeps motion fluid.
                            let spectrum = audio::current_spectrum();
                            if spectrum.len() == this.waveform_bars.len() {
                                for (i, v) in this.waveform_bars.iter_mut().enumerate() {
                                    let target = spectrum[i].clamp(0.0, 1.0);
                                    *v = (*v * 0.35 + target * 0.65).max(0.06);
                                }
                            }
                            // Bottom: time-rolling history (each bar = one
                            // moment, oldest left → newest right). Copy
                            // verbatim — mixing would smear the time axis.
                            let history = audio::current_waveform_history();
                            if history.len() == this.player_bars_history.len() {
                                for (i, v) in this.player_bars_history.iter_mut().enumerate() {
                                    *v = history[i].clamp(0.0, 1.0).max(0.05);
                                }
                            }
                        }
                    } else {
                        if this.is_playing {
                            this.is_playing = false;
                            if this.status == "Playing..." || this.status == "Stopping..." {
                                this.status = "Idle".into();
                            }
                        }
                        if !this.highlight_active {
                            // Gentle decay to baseline when idle.
                            for v in this.waveform_bars.iter_mut() {
                                *v = (*v * 0.85).max(0.08);
                            }
                            for v in this.player_bars_history.iter_mut() {
                                *v = (*v * 0.9).max(0.05);
                            }
                        }
                    }
                    cx.notify();
                })
                .ok();
            }
        }).detach();

        Self {
            status: "Idle".into(),
            is_playing: false,
            current_time: "0:00".into(),
            turns: vec![
                Turn { who: "Ares".into(), time: "0:00".into(), text: "Welcome to Melody FM.".into() },
                Turn { who: "Ares".into(), time: "0:02".into(), text: "Your private AI radio station for music exploration.".into() },
                Turn { who: "Ares".into(), time: "0:05".into(), text: "Ask me anything about music \u{2014} theory, composition, analysis.".into() },
            ],
            clock: Local::now().format("%H:%M").to_string(),
            current_turn_index: 0,
            current_word_index: 0,
            highlight_active: false,
            waveform_bars: vec![0.08; 60],
            player_bars_history: vec![0.08; 60],
            is_asking: false,
            fluid_hue: 0.0,
            clear_input: false,
            input_state,
            _text_sub: text_sub,
            queue_first_seen: HashMap::new(),
            current_lyrics: None,
            current_lyrics_url: None,
            current_lyric_text: None,
        }
    }

    /// Reconcile the queue_first_seen map with the live queue, inserting
    /// `Instant::now()` for newly appeared songs and pruning departed ones.
    fn refresh_queue_anim(&mut self) {
        let snapshot = audio::queue_songs();
        let now = Instant::now();
        let mut next: HashMap<String, Instant> = HashMap::with_capacity(snapshot.len());
        for song in &snapshot {
            let key = song.url.clone();
            let ts = self.queue_first_seen.get(&key).copied().unwrap_or(now);
            next.insert(key, ts);
        }
        self.queue_first_seen = next;
    }

    /// Refresh cached lyrics when the playing song changes, and update the
    /// current visible lyric text against `audio::current_position()`.
    fn refresh_lyrics(&mut self) {
        let current = audio::current_song();
        let new_url = current.as_ref().map(|s| s.url.clone());
        if new_url != self.current_lyrics_url {
            self.current_lyrics_url = new_url.clone();
            self.current_lyrics = current
                .as_ref()
                .and_then(|s| s.lyrics_path.as_ref())
                .and_then(|p| lyrics::Lyrics::from_vtt_file(p));
            self.current_lyric_text = None;
        }
        let text = match (&self.current_lyrics, audio::current_position()) {
            (Some(lyr), Some(pos)) => lyr.current_line(pos).map(|l| l.text.clone()),
            _ => None,
        };
        if text != self.current_lyric_text {
            self.current_lyric_text = text;
        }
    }

    fn update_clock(&mut self) {
        self.clock = Local::now().format("%H:%M").to_string();
    }

        fn ask_question(&mut self, question: &str, cx: &mut Context<Self>) {
        if self.is_asking {
            return;
        }
        self.is_asking = true;
        self.status = "Asking...".into();
        cx.notify();

        let question_owned = question.to_string();
        let time_str = self.current_time.clone();

        self.turns.push(Turn {
            who: "You".into(),
            time: time_str,
            text: question_owned.clone(),
        });
        cx.notify();

        let async_cx = cx.to_async();
        cx.spawn(|this: WeakEntity<Self>, _cx: &mut AsyncApp| async move {
            let cx = async_cx;

            // Detect direct play/enqueue/skip/replay commands in Chinese/English.
            let raw = question_owned.trim();
            let lowered = raw.to_lowercase();
            let play_cn = "\u{64AD}\u{653E}"; // 播放
            let put_cn = "\u{653E}"; // 放
            let queue_cn = "\u{961F}\u{5217}"; // 队列
            let enqueue_cn = "\u{6392}\u{961F}"; // 排队
            let skip_cn = "\u{4E0B}\u{4E00}\u{9996}"; // 下一首
            let cut_cn = "\u{5207}\u{6B4C}"; // 切歌
            let replay_cn = "\u{91CD}\u{64AD}"; // 重播
            let replay2_cn = "\u{91CD}\u{65B0}\u{64AD}\u{653E}"; // 重新播放

            // ── Stop command ──
            if raw == "\u{505C}\u{6B62}" || raw == "\u{505C}" || lowered == "stop" {
                audio::stop_music();
                if let Some(v) = this.upgrade() {
                    v.update(&mut cx.clone(), |this, cx| {
                        this.is_asking = false;
                        this.status = "Stopped".into();
                        this.is_playing = false;
                        cx.notify();
                    }).ok();
                }
                return;
            }

            // ── Skip command ──
            if raw.contains(skip_cn) || raw.contains(cut_cn) || lowered.contains("skip") || lowered.contains("next ") {
                music_player::skip();
                if let Some(v) = this.upgrade() {
                    v.update(&mut cx.clone(), |this, cx| {
                        this.is_asking = false;
                        this.status = "Skipping...".into();
                        cx.notify();
                    }).ok();
                }
                return;
            }

            // ── Shuffle command ──
            let shuffle_cn = "\u{968F}\u{673A}"; // 随机
            let shuffle_cn2 = "\u{6D17}\u{724C}"; // 洗牌
            let shuffle_cn3 = "\u{6253}\u{4E71}"; // 打乱
            if raw.contains(shuffle_cn)
                || raw.contains(shuffle_cn2)
                || raw.contains(shuffle_cn3)
                || lowered.contains("shuffle")
            {
                let n = audio::shuffle_queue();
                if let Some(v) = this.upgrade() {
                    v.update(&mut cx.clone(), |this, cx| {
                        this.is_asking = false;
                        this.status = if n == 0 {
                            "Queue empty".into()
                        } else {
                            format!("Shuffled ({} songs)", n)
                        };
                        this.refresh_queue_anim();
                        cx.notify();
                    }).ok();
                }
                return;
            }

            // ── Replay command ──
            if raw.contains(replay_cn) || raw.contains(replay2_cn) || lowered.contains("replay") {
                music_player::replay();
                if let Some(v) = this.upgrade() {
                    v.update(&mut cx.clone(), |this, cx| {
                        this.is_asking = false;
                        this.status = "Replaying...".into();
                        this.is_playing = true;
                        cx.notify();
                    }).ok();
                }
                return;
            }

            // ── Play / Enqueue commands ──
            //
            // Routing rules — conservative on purpose, so that natural language
            // like "我失恋了 推荐几首中文歌曲播放" falls through to opencc CLI
            // instead of being mis-routed to a music search with empty query:
            //
            // * `播放<song>` / `play <song>` / `queue <song>` — keyword at start: explicit intent.
            // * `…播放<song>` — keyword anywhere, but only if there is non-empty
            //   text AFTER it; otherwise the keyword is treated as part of free
            //   prose and the message goes to opencc.
            // * `放` (single char) — too ambiguous (放心/释放/燃放); only accepted
            //   as a `strip_prefix` match, never as a substring fallback.
            let take_after = |keyword: &str| -> Option<String> {
                raw.find(keyword).and_then(|pos| {
                    let rest = raw[(pos + keyword.len())..].trim();
                    if rest.is_empty() { None } else { Some(rest.to_string()) }
                })
            };
            let (query_opt, is_enqueue) = if let Some(rest) = raw.strip_prefix(play_cn) {
                (Some(rest.trim().to_string()), false)
            } else if let Some(rest) = raw.strip_prefix(put_cn) {
                (Some(rest.trim().to_string()), false)
            } else if let Some(rest) = raw.strip_prefix(queue_cn) {
                (Some(rest.trim().to_string()), true)
            } else if let Some(rest) = raw.strip_prefix(enqueue_cn) {
                (Some(rest.trim().to_string()), true)
            } else if let Some(rest) = take_after(play_cn) {
                (Some(rest), false)
            } else if let Some(rest) = take_after(queue_cn) {
                (Some(rest), true)
            } else if let Some(rest) = take_after(enqueue_cn) {
                (Some(rest), true)
            } else if let Some(pos) = lowered.find("play ") {
                (Some(raw[(pos + 5)..].trim().to_string()), false)
            } else if let Some(pos) = lowered.find("queue ") {
                (Some(raw[(pos + 6)..].trim().to_string()), true)
            } else {
                (None, false)
            };

            if let Some(query) = query_opt {
                if !query.is_empty() {
                    // Split on ASCII / full-width punctuation so users can chain
                    // requests like: 播放 A, B、C; D
                    let cleaned = query
                        .trim()
                        .trim_start_matches('[')
                        .trim_end_matches(']');
                    let parts: Vec<String> = cleaned
                        .split(|c: char| matches!(
                            c,
                            ',' | '\u{FF0C}' | '\u{3001}' | ';' | '\u{FF1B}' | '\n'
                        ))
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    let mut summary_lines: Vec<String> = Vec::new();
                    let mut last_err: Option<String> = None;
                    let mut first_played_title: Option<String> = None;

                    for (idx, part) in parts.iter().enumerate() {
                        // First item with !is_enqueue triggers interruptive play;
                        // every subsequent item appends to the queue.
                        let use_enqueue = is_enqueue || idx > 0;
                        let result = if use_enqueue {
                            music_player::search_and_enqueue(part).await
                        } else {
                            music_player::search_and_play(part).await
                        };
                        match result {
                            Ok(msg) => {
                                if idx == 0 && !is_enqueue {
                                    first_played_title = Some(msg.clone());
                                }
                                summary_lines.push(msg);
                            }
                            Err(e) => {
                                last_err = Some(format!("{}: {}", part, e));
                                summary_lines.push(format!("⚠ {} — {}", part, e));
                            }
                        }
                    }

                    let combined_msg = if summary_lines.is_empty() {
                        "(no tracks)".to_string()
                    } else {
                        summary_lines.join("\n")
                    };
                    let any_success = first_played_title.is_some()
                        || summary_lines.iter().any(|l| !l.starts_with('⚠'));

                    if any_success {
                        let status_label: String = if is_enqueue {
                            "Queued".into()
                        } else if parts.len() > 1 {
                            format!("Playing (+{} queued)", parts.len().saturating_sub(1))
                        } else {
                            "Playing...".into()
                        };
                        if let Some(v) = this.upgrade() {
                            v.update(&mut cx.clone(), |this, cx| {
                                this.is_asking = false;
                                this.status = status_label;
                                this.is_playing = !is_enqueue;
                                this.turns.push(Turn {
                                    who: "Melody".into(),
                                    time: this.current_time.clone(),
                                    text: combined_msg.clone(),
                                });
                                this.update_clock();
                                cx.notify();
                            }).ok();
                        }
                    } else {
                        let err_msg = last_err
                            .unwrap_or_else(|| "Playback failed".into());
                        if let Some(v) = this.upgrade() {
                            v.update(&mut cx.clone(), |this, cx| {
                                this.is_asking = false;
                                this.status = "Error".into();
                                this.is_playing = false;
                                this.turns.push(Turn {
                                    who: "Melody".into(),
                                    time: this.current_time.clone(),
                                    text: format!("Playback failed: {}", err_msg),
                                });
                                this.update_clock();
                                cx.notify();
                            }).ok();
                        }
                    }
                    return;
                }
                // Empty query after a play/queue keyword (e.g. user typed just
                // "播放") — no song to search for. Fall through to opencc so
                // the AI can clarify intent rather than silently going Idle.
            }

            append_flow_log(&format!("ask start question_len={}", question_owned.len()));
            let reply = cli_client::send_message(&question_owned).await;
            let is_error = reply.starts_with("Error");
            append_flow_log(&format!("cli done is_error={} reply_len={}", is_error, reply.len()));
            let mut turn_idx: usize = 0;

            if let Some(view) = this.upgrade() {
                view.update(&mut cx.clone(), |this, cx| {
                    this.is_asking = false;
                    this.status = if is_error { "Error".into() } else { "Idle".into() };
                    this.is_playing = audio::is_music_playing();
                    this.turns.push(Turn {
                        who: "Ares".into(),
                        time: this.current_time.clone(),
                        text: reply.clone(),
                    });
                    turn_idx = this.turns.len().saturating_sub(1);
                    this.current_turn_index = turn_idx;
                    this.current_word_index = 0;
                    this.highlight_active = false;
                    this.update_clock();
                    cx.notify();
                }).ok();
            }

            if !is_error {
                if let Some(v) = this.upgrade() {
                    v.update(&mut cx.clone(), |this, cx| {
                        this.status = "Idle".into();
                        this.is_playing = false;
                        this.highlight_active = false;
                        this.waveform_bars = vec![0.08; 60];
                        cx.notify();
                    }).ok();
                }
            }
        }).detach();
    }
}

impl Render for MusicAgentApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Clear input field if flag is set (needs &mut Window, only available here)
        if self.clear_input {
            self.input_state.update(cx, |state, cx| state.set_value("", window, cx));
            self.clear_input = false;
        }

        let weak = cx.weak_entity();
        // Outer layout: solid dark bg behind a full-size card. The fluid
        // animated background is rendered INSIDE the card so the card's
        // `rounded(34) + overflow_hidden` clips it cleanly. Without this,
        // when the fluid hue cycled to navy-ish it blended with the card
        // and the rounded corners visually disappeared on one side.
        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(gpui::rgb(0x07080C))
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(self.render_card_with_weak(weak)),
            )
    }
}

// ── Fluid background renderer ──────────────────────────────────────────
impl MusicAgentApp {
    fn render_fluid_background(&self) -> impl IntoElement {
        let hue = self.fluid_hue / 360.0; // Normalize to 0.0-1.0 for Hsla
        let hue2 = ((self.fluid_hue + 120.0) % 360.0) / 360.0;
        let hue3 = ((self.fluid_hue + 240.0) % 360.0) / 360.0;

        // Create smooth oklch-like gradient using three layered semi-transparent divs
        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .size_full()
            .child(
                // Layer 1: primary hue from top
                div()
                    .size_full()
                    .bg(gpui::Hsla {
                        h: hue,
                        s: 0.6,
                        l: 0.35,
                        a: 0.5,
                    }),
            )
            .child(
                // Layer 2: offset hue from bottom-right
                div()
                    .size_full()
                    .bg(gpui::Hsla {
                        h: hue2,
                        s: 0.7,
                        l: 0.3,
                        a: 0.5,
                    }),
            )
            .child(
                // Layer 3: third hue with blend
                div()
                    .size_full()
                    .bg(gpui::Hsla {
                        h: hue3,
                        s: 0.5,
                        l: 0.4,
                        a: 0.4,
                    }),
            )
    }
}

// ── Card layout ────────────────────────────────────────────────────────
impl MusicAgentApp {
    fn render_card_with_weak(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        // Card bg matches header bg so the rounded corners always look correct
        // regardless of what the outer container has. The animated fluid
        // gradient is no longer rendered — it was hidden by the opaque header
        // and body anyway, and the only place it ever showed was through the
        // rounded-corner cutouts (which is exactly the visual bug we fixed).
        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(34.0))
            .bg(gpui::rgb(0x0B1324))
            .child(self.render_header())
            .child(self.render_body_with_weak(weak))
    }
}

// ── Header ─────────────────────────────────────────────────────────────
impl MusicAgentApp {
    fn render_header(&self) -> impl IntoElement {
        div()
            .h(px(208.0))
            .flex()
            .flex_col()
            .justify_between()
            .px(px(26.0))
            .pt(px(20.0))
            .bg(gpui::rgb(0x0B1324))
            .child(self.render_header_top())
            .child(self.render_waveform_area())
    }

    fn render_header_top(&self) -> impl IntoElement {
        let status_text = if self.highlight_active {
            "Speaking...".to_string()
        } else {
            self.status.clone()
        };
        div()
            .flex()
            .justify_between()
            .items_start()
            .child(
                div()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(self.render_avatar())
                            .child(
                                div()
                                    .text_color(gpui::rgb(0xFFFFFF))
                                    .text_size(px(31.0))
                                    .font_weight(FontWeight::THIN)
                                    .child("Melody")
                            )
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .ml(px(38.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(self.render_status_dot())
                            .child(
                                div()
                                    .text_color(gpui::rgb(0x1DE59A))
                                    .text_size(px(13.0))
                                    .child(status_text)
                            )
                    )
            )
            .child(
                div()
                    .text_color(gpui::rgba(0xFFFFFFF2))
                    .text_size(px(15.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.clock.clone())
            )
    }

    fn render_avatar(&self) -> impl IntoElement {
        div()
            .w(px(24.0))
            .h(px(24.0))
            .rounded_full()
            .bg(gpui::rgb(0x4A80FF))
            .flex()
            .items_center()
            .justify_center()
            .child(div().text_color(gpui::rgb(0xFFFFFF)).text_size(px(12.0)).child("A"))
    }

    fn render_status_dot(&self) -> impl IntoElement {
        div()
            .w(px(6.0))
            .h(px(6.0))
            .rounded_full()
            .bg(if self.is_asking {
                gpui::rgb(0xFFAA00)
            } else {
                gpui::rgb(0x29FFB8)
            })
    }

    fn render_waveform_area(&self) -> impl IntoElement {
        // Placeholder for audio waveform canvas
        div()
                    .h(px(82.0))
            .w_full()
            .flex()
            .items_end()
            .px(px(24.0))
            .child(
                // Render some static waveform bars
                self.render_wave_bars(60),
            )
    }

    fn render_wave_bars(&self, count: usize) -> impl IntoElement {
        // Frequency-spectrum bars. flex_1 makes the 60 bars fill the panel
        // width evenly (instead of clumping on the left as they did with a
        // fixed px width).
        let mut row = div().flex().items_end().gap(px(2.0)).w_full();
        for i in 0..count {
            let a = self.waveform_bars.get(i).copied().unwrap_or(0.08).clamp(0.0, 1.0);
            let h = 6.0 + a * 72.0;
            row = row.child(
                div()
                    .flex_1()
                    .min_w(px(2.0))
                    .h(px(h as f32))
                    .rounded(px(1.5))
                    .bg(gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.9 }),
            );
        }
        row
    }
}

// ── Body ───────────────────────────────────────────────────────────────
impl MusicAgentApp {
    fn render_body_with_weak(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        // Body has square top corners on purpose. gpui 0.2's Blade renderer
        // on Linux shows an asymmetric clip-path bug here: top-right starts
        // sharp on first paint and only "snaps" to rounded after the view
        // has been re-rendered enough times (e.g. once playback animation
        // kicks in). Tried rounded_t(R), rounded_tl+tr, with/without mt(-N) —
        // all reproduce. Rather than ship a UI that looks broken on launch,
        // we accept square corners. The outer card still has rounded(34) so
        // the overall window silhouette stays soft.
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .overflow_hidden()
            .bg(gpui::rgb(0xFFFFFF))
            .child(self.render_meta())
            .child(self.render_lyric_line())
            .child(self.render_queue_list(weak.clone()))
            .child(self.render_history_list(weak.clone()))
            .child(self.render_transcript())
            .child(self.render_input_area(weak.clone()))
            .child(self.render_question_buttons(weak.clone()))
            .child(self.render_player(weak))
    }

    fn render_meta(&self) -> impl IntoElement {
        let queue_len = audio::queue_len();
        let label = match audio::current_song() {
            Some(song) => {
                if queue_len > 0 {
                    format!("♫ {} (+{} queued)", song.title, queue_len)
                } else {
                    format!("♫ {}", song.title)
                }
            }
            None => {
                if queue_len > 0 {
                    format!("⏳ {} songs in queue — waiting...", queue_len)
                } else {
                    return div().h(px(0.0));
                }
            }
        };
        div()
            .h(px(24.0))
            .flex()
            .items_center()
            .px(px(16.0))
            .text_size(px(11.0))
            .text_color(gpui::rgb(0x888888))
            .child(label)
    }

    fn render_history_list(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        let history = audio::history_songs();
        // Hide history when actively playing or queued — it would just be noise
        // there; the user wants it visible after they stop / when idle.
        let busy = audio::is_music_playing() || audio::queue_len() > 0;
        if busy || history.is_empty() {
            return div().h(px(0.0)).into_any_element();
        }
        const VISIBLE: usize = 8;
        let mut list = div()
            .mx(px(16.0))
            .mb(px(4.0))
            .py(px(6.0))
            .px(px(10.0))
            .rounded(px(8.0))
            .bg(gpui::rgb(0xEEF2FF))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_size(px(10.5))
                    .text_color(gpui::rgb(0x4F46E5))
                    .mb(px(2.0))
                    .child("\u{23EE} \u{6700}\u{8FD1}\u{64AD}\u{653E} \u{00B7} \u{70B9}\u{51FB}\u{91CD}\u{64AD}"), // ⏮ 最近播放 · 点击重播
            );
        for (i, song) in history.iter().take(VISIBLE).enumerate() {
            let row_weak = weak.clone();
            let title = song.title.clone();
            list = list.child(
                div()
                    .id(ElementId::Name(format!("hist_{}", i).into()))
                    .cursor_pointer()
                    .px(px(4.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(gpui::rgb(0x9CA3AF))
                            .w(px(16.0))
                            .child(format!("{}", i + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.0))
                            .text_color(gpui::rgb(0x333333))
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(gpui::rgb(0x4F46E5))
                            .child("\u{25B6}"), // ▶
                    )
                    .on_click(move |_event, _window, cx| {
                        row_weak
                            .update(cx, |this, cx| {
                                if audio::play_from_history(i) {
                                    this.status = "Playing...".into();
                                    this.is_playing = true;
                                }
                                cx.notify();
                            })
                            .ok();
                    }),
            );
        }
        list.into_any_element()
    }

    fn render_lyric_line(&self) -> impl IntoElement {
        match &self.current_lyric_text {
            Some(text) if !text.is_empty() => div()
                .mx(px(16.0))
                .mb(px(2.0))
                .px(px(4.0))
                .text_size(px(13.0))
                .text_color(gpui::rgb(0x0F172A))
                .child(format!("♪ {}", text))
                .into_any_element(),
            _ => div().h(px(0.0)).into_any_element(),
        }
    }

    fn render_queue_list(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        let songs = audio::queue_songs();
        if songs.is_empty() {
            return div().h(px(0.0)).into_any_element();
        }
        const FADE_MS: u128 = 320;
        let now = Instant::now();
        let mut list = div()
            .mx(px(16.0))
            .mb(px(4.0))
            .py(px(6.0))
            .px(px(10.0))
            .rounded(px(8.0))
            .bg(gpui::rgb(0xF5F5F7))
            .flex()
            .flex_col()
            .gap(px(2.0));
        for (i, song) in songs.iter().enumerate() {
            let prefix = if i == 0 { "▸" } else { " " };
            let label = format!("{} {}", prefix, song.title);

            // Opacity ramp: 0 → 1 over FADE_MS for newly inserted items.
            let opacity = self
                .queue_first_seen
                .get(&song.url)
                .map(|seen| {
                    let elapsed = now.duration_since(*seen).as_millis();
                    (elapsed as f32 / FADE_MS as f32).clamp(0.15, 1.0)
                })
                .unwrap_or(1.0);

            // Pin (move-to-top) — only meaningful for index > 0.
            let pin_weak = weak.clone();
            let pin_button = div()
                .id(ElementId::Name(format!("pin_{}", i).into()))
                .cursor_pointer()
                .px(px(6.0))
                .py(px(1.0))
                .rounded(px(4.0))
                .text_size(px(11.0))
                .text_color(gpui::rgb(0x4A80FF))
                .child("▲")
                .on_click(move |_event, _window, cx| {
                    pin_weak
                        .update(cx, |this, cx| {
                            audio::pin_to_top(i);
                            this.refresh_queue_anim();
                            cx.notify();
                        })
                        .ok();
                });

            let remove_weak = weak.clone();
            let remove_button = div()
                .id(ElementId::Name(format!("rm_{}", i).into()))
                .cursor_pointer()
                .px(px(6.0))
                .py(px(1.0))
                .rounded(px(4.0))
                .text_size(px(11.0))
                .text_color(gpui::rgb(0xC0392B))
                .child("✕")
                .on_click(move |_event, _window, cx| {
                    remove_weak
                        .update(cx, |this, cx| {
                            audio::remove_from_queue(i);
                            this.refresh_queue_anim();
                            cx.notify();
                        })
                        .ok();
                });

            let row = div()
                .flex()
                .items_center()
                .justify_between()
                .opacity(opacity)
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .text_color(gpui::rgb(0x555555))
                        .child(label),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.0))
                        .when(i > 0, |el| el.child(pin_button))
                        .child(remove_button),
                );

            list = list.child(row);
        }
        list.into_any_element()
    }

    fn render_transcript(&self) -> impl IntoElement {
        div()
            .flex_1()
            .min_h(px(260.0))
            .mx(px(12.0))
            .rounded(px(22.0))
            .bg(gpui::rgb(0xFFFFFF))
            .overflow_y_scrollbar()
            .child(
                div()
                    .p(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .pb(px(28.0))
                    .bg(gpui::rgb(0xFFFFFF))
                    .child(self.render_turns())
            )
    }

    fn render_turns(&self) -> impl IntoElement {
        let mut container = div().flex().flex_col();

        for (turn_idx, turn) in self.turns.iter().enumerate() {
            let words: Vec<&str> = turn.text.split_whitespace().collect();

            let mut turn_word_container = div()
                .mb(px(4.0))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(gpui::rgb(0x8A8A90))
                        .mb(px(2.0))
                        .child(format!("{} \u{2022} {}", turn.who, turn.time))
                );

            let mut word_row = div().flex().flex_wrap().gap(px(4.0));

            for (word_idx, word) in words.iter().enumerate() {
                let highlight_this_turn = self.highlight_active && turn_idx == self.current_turn_index;
                let is_past = highlight_this_turn && word_idx < self.current_word_index;
                let is_current = highlight_this_turn && word_idx == self.current_word_index;
                let _is_future = !is_past && !is_current;

                let word_el = if is_current {
                    div()
                        .px(px(4.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .bg(gpui::Rgba {
                            r: 41.0 / 255.0,
                            g: 255.0 / 255.0,
                            b: 184.0 / 255.0,
                            a: 0.28,
                        })
                        .text_color(gpui::rgb(0x0F3A2A))
                        .child(word.to_string())
                } else if is_past || !highlight_this_turn {
                    div()
                        .text_color(gpui::rgb(0x0F172A))
                        .child(word.to_string())
                } else {
                    div()
                        .text_color(gpui::rgb(0x6B7280))
                        .child(word.to_string())
                };

                word_row = word_row.child(word_el);
            }

            turn_word_container = turn_word_container.child(
                div()
                    .text_size(px(15.0))
                    .child(word_row),
            );

            container = container.child(turn_word_container);
        }

        container
    }

        fn render_input_area(&self, _weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        let input = Input::new(&self.input_state);

        div()
            .px(px(12.0))
            .py(px(6.0))
            .child(input)
    }

    fn render_question_buttons(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        div()
            .px(px(12.0))
            .pt(px(6.0))
            .pb(px(4.0))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.0))
                    .child(self.render_preset_buttons(weak))
            )
    }

        fn render_preset_buttons(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        let mut container = div().flex().flex_wrap().gap(px(6.0));

        for question in PRESET_QUESTIONS {
            let q = *question;
            let is_disabled = self.is_asking;
            let weak_for_click = weak.clone();

            container = container.child(
                div()
                    .id(ElementId::Name(format!("preset_{}", q).into()))
                    .flex_shrink()
                    .px(px(12.0))
                    .py(px(5.0))
                    .rounded(px(14.0))
                    .bg(gpui::rgb(0x0F172A))
                    .cursor_pointer()
                    .when(self.is_asking, |el| el.opacity(0.4))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(gpui::rgb(0xFFFFFF))
                            .child(q.to_string())
                    )
                    .on_click(move |_event, _window, cx| {
                        if !is_disabled {
                            weak_for_click.update(cx, |this, cx| {
                                this.ask_question(q, cx);
                            }).ok();
                        }
                    }),
            );
        }

        container
    }

    fn render_player(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(14.0))
            .px(px(18.0))
            .pt(px(8.0))
            .pb(px(10.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(gpui::rgb(0x0F172A))
                    .child(self.current_time.clone())
            )
            .child(
                div()
                    .flex_1()
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(self.render_player_bars())
            )
            .child(self.render_shuffle_button(weak.clone()))
            .child(self.render_play_button(weak))
    }

    fn render_shuffle_button(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        let queue_has_items = audio::queue_len() >= 2;
        let mut btn = div()
            .id("shuffle-btn")
            .w(px(32.0))
            .h(px(32.0))
            .rounded_full()
            .bg(gpui::rgb(0xE5E7EB))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_color(gpui::rgb(if queue_has_items { 0x0F172A } else { 0x9CA3AF }))
                    .text_size(px(14.0))
                    .child("\u{1F500}"), // 🔀
            );
        if queue_has_items {
            btn = btn.cursor_pointer().on_click(move |_event, _window, cx| {
                weak.update(cx, |this, cx| {
                    let n = audio::shuffle_queue();
                    this.refresh_queue_anim();
                    this.status = format!("Shuffled ({} songs)", n);
                    cx.notify();
                })
                .ok();
            });
        } else {
            btn = btn.opacity(0.55);
        }
        btn
    }

    fn render_player_bars(&self) -> impl IntoElement {
        // The small bottom row visualises the time-rolling history (new audio
        // events enter on the right, old ones scroll off to the left) —
        // visually distinct from the header's frequency spectrum.
        let mut container = div().flex().flex_1().items_center().gap(px(2.0));
        for i in 0..56 {
            let src = i * 60 / 56;
            let a = self
                .player_bars_history
                .get(src)
                .copied()
                .unwrap_or(0.08)
                .clamp(0.0, 1.0);
            let h = 4.0 + a * 22.0;
            container = container.child(
                div()
                    .flex_1()
                    .h(px(h as f32))
                    .rounded_full()
                    .bg(gpui::rgb(0xCFCFD1)),
            );
        }
        container
    }

    fn render_play_button(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        let is_playing = self.is_playing;
        div()
            .id("play-btn")
            .w(px(36.0))
            .h(px(36.0))
            .rounded_full()
            .bg(gpui::rgb(0x0F172A))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(
                div()
                    .text_color(gpui::rgb(0xFFFFFF))
                    .text_size(px(14.0))
                    .child(if is_playing { "\u{25A0}" } else { "\u{25B6}" })
            )
            .on_click(move |_event, _window, cx| {
                weak.update(cx, |this, _cx| {
                    if this.is_playing {
                        audio::stop_music();
                        this.status = "Stopping...".into();
                    }
                    // else: no music playing — do nothing (no ambient hum)
                }).ok();
            })
    }
}

