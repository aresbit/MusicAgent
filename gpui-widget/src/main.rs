// MusicAgent GPUI Widget - Claudio FM inspired private radio desktop widget
// Connects to opencc CLI.js for AI-powered music conversations

use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement;
use chrono::Local;
use std::fs::OpenOptions;
use std::time::Duration;

mod audio;
mod cli_client;
mod tts_client;

const APP_FLOW_LOG_FILE: &str = r"D:\yyscode\MusicAgent\gpui-widget\musicagent-flow.log";

fn append_flow_log(line: &str) {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let msg = format!("[{}] {}\n", ts, line);
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(APP_FLOW_LOG_FILE)
    {
        use std::io::Write;
        let _ = f.write_all(msg.as_bytes());
    }
}

fn build_tts_text(raw: &str) -> String {
    let mut s = raw.replace('*', " ").replace('`', " ");
    s = s.replace('#', " ").replace('_', " ");
    let compact = s.split_whitespace().collect::<Vec<_>>().join(" ");
    const MAX_CHARS: usize = 900;
    if compact.chars().count() > MAX_CHARS {
        compact.chars().take(MAX_CHARS).collect::<String>()
    } else {
        compact
    }
}

// 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?Pre-set Questions 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?
const PRESET_QUESTIONS: &[&str] = &[
    "Recommend some late-night jazz piano albums",
    "What makes Miles Davis's Kind of Blue so influential?",
    "Explain the Dorian mode with a musical example",
    "Tell me an interesting story about a classical composer",
];

// 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?Application Entry 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?
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
                        title: Some("MusicAgent FM".into()),
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

// 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?App State 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?
struct MusicAgentApp {
    status: String,
    is_playing: bool,
    current_time: String,
    turns: Vec<Turn>,
    clock: String,
    current_turn_index: usize,
    current_word_index: usize,
    highlight_active: bool,
    is_asking: bool,
    fluid_hue: f32, // 0.0-360.0 hue for animated gradient background
    clear_input: bool,
    input_state: Entity<InputState>,
    _text_sub: Subscription,
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
                this.update(&mut cx.clone(), |this, _cx| {
                    this.fluid_hue = (this.fluid_hue + 0.5) % 360.0;
                })
                .ok();
            }
        }).detach();

        // Startup spoken intro: speak the three opening lines in sequence.
        smol::spawn(async {
            let intro_lines = [
                "Welcome to MusicAgent FM.",
                "Your private AI radio station for music exploration.",
                "Ask me anything about music theory, composition, analysis.",
            ];
            for (line_idx, line) in intro_lines.iter().enumerate() {
                let mut done = false;
                for attempt in 1..=3 {
                    append_flow_log(&format!(
                        "startup intro line={} tts attempt={}",
                        line_idx + 1,
                        attempt
                    ));
                    match tts_client::text_to_speech(line, "Bella").await {
                        Ok(wav) => {
                            append_flow_log(&format!(
                                "startup intro line={} tts bytes={}",
                                line_idx + 1,
                                wav.len()
                            ));
                            let _ = smol::unblock(move || audio::play_tts_blocking(wav)).await;
                            done = true;
                            break;
                        }
                        Err(e) => {
                            append_flow_log(&format!(
                                "startup intro line={} tts failed attempt={} err={}",
                                line_idx + 1,
                                attempt,
                                e
                            ));
                            smol::Timer::after(std::time::Duration::from_millis(1200)).await;
                        }
                    }
                }
                if !done {
                    append_flow_log(&format!(
                        "startup intro line={} gave up after retries",
                        line_idx + 1
                    ));
                }
            }
        })
        .detach();

        Self {
            status: "Idle".into(),
            is_playing: false,
            current_time: "0:00".into(),
            turns: vec![
                Turn { who: "Ares".into(), time: "0:00".into(), text: "Welcome to MusicAgent FM.".into() },
                Turn { who: "Ares".into(), time: "0:02".into(), text: "Your private AI radio station for music exploration.".into() },
                Turn { who: "Ares".into(), time: "0:05".into(), text: "Ask me anything about music \u{2014} theory, composition, analysis.".into() },
            ],
            clock: Local::now().format("%H:%M").to_string(),
            current_turn_index: 0,
            current_word_index: 0,
            highlight_active: false,
            is_asking: false,
            fluid_hue: 0.0,
            clear_input: false,
            input_state,
            _text_sub: text_sub,
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
            append_flow_log(&format!("ask start question_len={}", question_owned.len()));
            let reply = cli_client::send_message(&question_owned).await;
            let is_error = reply.starts_with("Error");
            append_flow_log(&format!("cli done is_error={} reply_len={}", is_error, reply.len()));
            let mut turn_idx: usize = 0;

            if let Some(view) = this.upgrade() {
                view.update(&mut cx.clone(), |this, cx| {
                    this.is_asking = false;
                    this.status = if is_error { "Error".into() } else { "Idle".into() };
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
                let tts_text = build_tts_text(&reply);
                let word_count = tts_text.split_whitespace().count().max(1);
                append_flow_log(&format!("tts request start words={}", word_count));

                // Always hook reply -> TTS, even if view temporarily unavailable.
                match tts_client::text_to_speech(&tts_text, "Bella").await {
                    Ok(wav) => {
                        append_flow_log(&format!("tts response bytes={}", wav.len()));
                        let total_ms = audio::estimate_wav_duration_ms(&wav)
                            .unwrap_or((word_count as u64) * 420);
                        let step_ms = (total_ms / word_count as u64).max(120);
                        append_flow_log(&format!("tts duration_ms={} step_ms={}", total_ms, step_ms));

                        if let Some(v) = this.upgrade() {
                            v.update(&mut cx.clone(), |this, cx| {
                                this.status = "Speaking...".into();
                                this.current_turn_index = turn_idx;
                                this.current_word_index = 0;
                                this.highlight_active = true;
                                cx.notify();
                            }).ok();
                        }

                        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        let done_for_player = done.clone();
                        smol::spawn(async move {
                            let _ = smol::unblock(move || audio::play_tts_blocking(wav)).await;
                            done_for_player.store(true, std::sync::atomic::Ordering::SeqCst);
                        }).detach();

                        let mut idx = 0usize;
                        while !done.load(std::sync::atomic::Ordering::SeqCst) {
                            if let Some(v) = this.upgrade() {
                                v.update(&mut cx.clone(), |this, cx| {
                                    this.current_turn_index = turn_idx;
                                    this.current_word_index = idx.min(word_count.saturating_sub(1));
                                    this.highlight_active = true;
                                    cx.notify();
                                }).ok();
                            }
                            idx = idx.saturating_add(1);
                            smol::Timer::after(std::time::Duration::from_millis(step_ms)).await;
                        }

                        if let Some(v) = this.upgrade() {
                            v.update(&mut cx.clone(), |this, cx| {
                                this.status = "Idle".into();
                                this.highlight_active = false;
                                cx.notify();
                            }).ok();
                        }
                        append_flow_log("tts playback done");
                    }
                    Err(e) => {
                        append_flow_log(&format!("tts failed: {}", e));
                        eprintln!("TTS failed: {}", e);
                    }
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
        // Main layout: fluid gradient background behind a centered card
        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .child(self.render_fluid_background())
            .child(
                // Centered card overlay
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

// 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?Fluid Gradient Background 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜?
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

// 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?Card Layout 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑?
impl MusicAgentApp {
    fn render_card_with_weak(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(34.0))
            .bg(gpui::rgb(0x0A0C12))
            .child(self.render_header())
            .child(self.render_body_with_weak(weak))
    }
}

// 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?Header 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜?
impl MusicAgentApp {
    fn render_header(&self) -> impl IntoElement {
        div()
            .h(px(250.0))
            .flex()
            .flex_col()
            .justify_between()
            .px(px(26.0))
            .pt(px(20.0))
            .bg(gpui::rgb(0x090B11))
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
                                    .child("Claudio")
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
                    .h(px(110.0))
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
        let mut row = div().flex().items_end().gap(px(1.0)).w_full();
        for i in 0..count {
            let n1 = (i as f64 * 0.6).sin() * 0.5 + 0.5;
            let n2 = (i as f64 * 0.23 + 1.3).sin() * 0.5 + 0.5;
            let env = 0.5 + 0.5 * ((i as f64 / count as f64) * std::f64::consts::PI).sin();
            let h = 4.0 + (n1 * 0.6 + n2 * 0.4) * env * 60.0;
            row = row.child(
                div()
                    .w(px(3.0))
                    .h(px(h as f32))
                    .rounded(px(1.5))
                    .bg(gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.84 }),
            );
        }
        row
    }
}

// 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸?Body 闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕闂佸啿鍘滈崑鎾绘煃閸忓浜鹃梺鍐插帨閸嬫捇鏌嶉崗澶婁壕

impl MusicAgentApp {
    fn render_body_with_weak(&self, weak: WeakEntity<MusicAgentApp>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .overflow_hidden()
            .bg(gpui::rgb(0xF7F7F8))
            .rounded_t(px(34.0))
            .mt(px(-8.0))
            .child(self.render_meta())
            .child(self.render_transcript())
            .child(self.render_input_area(weak.clone()))
            .child(self.render_question_buttons(weak.clone()))
            .child(self.render_player(weak))
    }

    fn render_meta(&self) -> impl IntoElement {
        div()
            .px(px(28.0))
            .pt(px(12.0))
            .pb(px(6.0))
            .child(
                div()
                    .child(
                        div()
                            .text_size(px(50.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(px(50.0))
                            .text_color(gpui::rgb(0x131418))
                            .child("Monday Night Exhale")
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_baseline()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .text_color(gpui::rgb(0x6C6E76))
                                    .child("If — Bread")
                            )
                    )
            )
    }

    fn render_transcript(&self) -> impl IntoElement {
        div()
            .flex_1()
            .min_h(px(260.0))
            .min_h(px(0.0))
            .mx(px(12.0))
            .rounded(px(22.0))
            .bg(gpui::rgb(0xF1F1F2))
            .overflow_y_scrollbar()
            .child(
                div()
                    .p(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .pb(px(28.0))
                    .bg(gpui::Rgba { r: 0.95, g: 0.95, b: 0.96, a: 1.0 })
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
                } else if is_past {
                    div()
                        .text_color(gpui::rgb(0x111111))
                        .child(word.to_string())
                } else {
                    div()
                        .text_color(gpui::rgb(0xB5B5BA))
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
                    .bg(gpui::rgb(0x111111))
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
                    .text_color(gpui::rgb(0x111111))
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
            .child(self.render_play_button(weak))
    }

    fn render_player_bars(&self) -> impl IntoElement {
        let mut container = div().flex().flex_1().items_center().gap(px(2.0));
        for i in 0..56 {
            let n1 = (i as f64 * 0.6).sin() * 0.5 + 0.5;
            let n2 = (i as f64 * 0.23 + 1.3).sin() * 0.5 + 0.5;
            let env = 0.5 + 0.5 * ((i as f64 / 56.0) * std::f64::consts::PI).sin();
            let h = 6.0 + (n1 * 0.6 + n2 * 0.4) * env * 18.0;
            container = container.child(
                div()
                    .flex_1()
                    .h(px(h as f32))
                    .rounded_full()
                    .bg(gpui::rgb(0xCFCFD1))
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
            .bg(gpui::rgb(0x111111))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(
                div()
                    .text_color(gpui::rgb(0xFFFFFF))
                    .text_size(px(14.0))
                    .child(if is_playing { "\u{23F8}" } else { "\u{25B6}" })
            )
            .on_click(move |_event, _window, cx| {
                weak.update(cx, |this, _cx| {
                    this.is_playing = !this.is_playing;
                    if this.is_playing {
                        audio::start_ambient();
                    } else {
                        audio::stop_ambient();
                    }
                }).ok();
            })
    }
}

