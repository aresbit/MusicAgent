// MusicAgent GPUI Widget — Claudio FM inspired private radio desktop widget
// Connects to opencc CLI.js for AI-powered music conversations

use gpui::*;
use gpui_component::*;
use gpui_component::scroll::ScrollableElement;
use chrono::Local;

// ─── Application Entry ───────────────────────────────────────────────────────

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
                    width: px(440.0),
                    height: px(780.0),
                },
            };

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("MusicAgent FM".into()),
                        appears_transparent: true,
                        traffic_light_position: None,
                    }),
                    kind: WindowKind::Normal,
                    focus: true,
                    show: true,
                    is_movable: true,
                    is_resizable: false,
                    is_minimizable: true,
                    display_id: None,
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|_| MusicAgentApp::new());
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("Failed to open window");
        })
        .detach();
    });
}

// ─── App State ───────────────────────────────────────────────────────────────

struct MusicAgentApp {
    status: String,
    is_playing: bool,
    current_time: String,
    turns: Vec<Turn>,
    clock: String,
}

struct Turn {
    who: String,
    time: String,
    text: String,
}

impl MusicAgentApp {
    fn new() -> Self {
        Self {
            status: "Idle".into(),
            is_playing: false,
            current_time: "0:00".into(),
            turns: vec![
                Turn { who: "Ares".into(), time: "0:00".into(), text: "Welcome to MusicAgent FM.".into() },
                Turn { who: "Ares".into(), time: "0:02".into(), text: "Your private AI radio station for music exploration.".into() },
                Turn { who: "Ares".into(), time: "0:05".into(), text: "Ask me anything about music — theory, composition, analysis.".into() },
            ],
            clock: Local::now().format("%H:%M").to_string(),
        }
    }
}

impl Render for MusicAgentApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Main layout: centered card with rounded corners on transparent background
        div().size_full().flex().flex_col().items_center().justify_center().overflow_hidden().child(
            self.render_card(),
        )
    }
}

// ─── Card Layout ─────────────────────────────────────────────────────────────

impl MusicAgentApp {
    fn render_card(&self) -> impl IntoElement {
        div()
            .w(px(440.0))
            .h(px(780.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(28.0))
            .bg(gpui::rgb(0xFFFFFF))
            .child(self.render_header())
            .child(self.render_body())
    }
}

// ─── Header ──────────────────────────────────────────────────────────────────

impl MusicAgentApp {
    fn render_header(&self) -> impl IntoElement {
        div()
            .h(px(232.0))
            .flex()
            .flex_col()
            .justify_between()
            .px(px(24.0))
            .pt(px(22.0))
            .bg(gpui::rgb(0x0C0C0E))
            .child(self.render_header_top())
            .child(self.render_waveform_area())
    }

    fn render_header_top(&self) -> impl IntoElement {
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
                                    .text_size(px(32.0))
                                    .child("MusicAgent")
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
                                    .text_color(gpui::rgb(0x29FFB8))
                                    .text_size(px(13.0))
                                    .child(self.status.clone())
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
            .w(px(28.0))
            .h(px(28.0))
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
            .bg(gpui::rgb(0x29FFB8))
    }

    fn render_waveform_area(&self) -> impl IntoElement {
        // Placeholder for audio waveform canvas
        div()
            .h(px(100.0))
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
                    .bg(gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.25 }),
            );
        }
        row
    }
}

// ─── Body ────────────────────────────────────────────────────────────────────

impl MusicAgentApp {
    fn render_body(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .bg(gpui::rgb(0xFFFFFF))
            .child(self.render_meta())
            .child(self.render_transcript())
            .child(self.render_player())
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
                            .text_size(px(22.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(gpui::rgb(0x111111))
                            .child("MusicAgent FM")
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_baseline()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(gpui::rgb(0x8A8A90))
                                    .child("Private AI Radio Station")
                            )
                    )
            )
    }

    fn render_transcript(&self) -> impl IntoElement {
        div()
            .flex_1()
            .mx(px(16.0))
            .rounded(px(18.0))
            .bg(gpui::rgb(0xF3F3F1))
            .overflow_y_scrollbar()
            .child(
                div()
                    .p(px(20.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(self.render_turns())
            )
    }

    fn render_turns(&self) -> impl IntoElement {
        let mut container = div().flex().flex_col();

        for turn in &self.turns {
            container = container
                .child(
                    div()
                        .mb(px(4.0))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(gpui::rgb(0x8A8A90))
                                .mb(px(2.0))
                                .child(format!("{} • {}", turn.who, turn.time))
                        )
                        .child(
                            div()
                                .text_size(px(15.0))
                                .text_color(gpui::rgb(0x111111))
                                .child(turn.text.clone())
                        )
                );
        }

        container
    }

    fn render_player(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(14.0))
            .px(px(18.0))
            .py(px(14.0))
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
            .child(self.render_play_button())
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

    fn render_play_button(&self) -> impl IntoElement {
        div()
            .w(px(36.0))
            .h(px(36.0))
            .rounded_full()
            .bg(gpui::rgb(0x111111))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_color(gpui::rgb(0xFFFFFF))
                    .text_size(px(14.0))
                    .child(if self.is_playing { "\u{23F8}" } else { "\u{25B6}" })
            )
    }
}
