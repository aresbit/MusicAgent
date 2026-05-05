// Claudio FM inspired color palette
use gpui::Rgba;

pub struct Theme {
    // Background
    pub bg_dark: Rgba,
    pub bg_card: Rgba,
    pub bg_transcript: Rgba,

    // Header
    pub header_gradient_top: Rgba,
    pub header_gradient_bottom: Rgba,
    pub header_text: Rgba,
    pub status_live: Rgba,
    pub header_time: Rgba,

    // Text
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,

    // Transcript
    pub turn_who: Rgba,
    pub turn_time: Rgba,
    pub word_said: Rgba,
    pub word_current: Rgba,
    pub word_current_bg: Rgba,
    pub word_future: Rgba,

    // Player
    pub player_bar_bg: Rgba,
    pub player_bar_played: Rgba,
    pub player_btn_bg: Rgba,
    pub player_btn_fg: Rgba,

    // Accent fluid background (oklch approximations in RGB)
    pub fluid_blue: Rgba,
    pub fluid_violet: Rgba,
    pub fluid_accent: Rgba,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg_dark: hex(0x111111),
            bg_card: hex(0xFFFFFF),
            bg_transcript: hex(0xF3F3F1),

            header_gradient_top: hex(0x0C0C0E),
            header_gradient_bottom: hex(0x16161A),
            header_text: hex(0xFFFFFF),
            status_live: hex(0x29FFB8),
            header_time: rgba(255, 255, 255, 0.95),

            text_primary: hex(0x111111),
            text_secondary: hex(0x8A8A90),
            text_muted: hex(0xB5B5BA),

            turn_who: hex(0x6A6A70),
            turn_time: hex(0x8A8A90),
            word_said: hex(0x111111),
            word_current: hex(0x0F3A2A),
            word_current_bg: rgba(41, 255, 184, 0.28),
            word_future: hex(0xB5B5BA),

            player_bar_bg: hex(0xCFCFD1),
            player_bar_played: hex(0x111111),
            player_btn_bg: hex(0x111111),
            player_btn_fg: hex(0xFFFFFF),

            fluid_blue: rgba(74, 128, 255, 80),
            fluid_violet: rgba(180, 100, 220, 70),
            fluid_accent: rgba(200, 130, 240, 50),
        }
    }
}

const fn hex(val: u32) -> Rgba {
    let r = ((val >> 16) & 0xFF) as u8;
    let g = ((val >> 8) & 0xFF) as u8;
    let b = (val & 0xFF) as u8;
    Rgba { r, g, b, a: 255 }
}

const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba { r, g, b, a }
}
