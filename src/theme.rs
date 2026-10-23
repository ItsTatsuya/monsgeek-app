//! The design system, in a single monochrome colour scheme.

use gpui::{rgb, rgba, Hsla};

#[derive(Clone, Copy)]
pub struct Theme {
    /// Window background.
    pub bg: Hsla,
    pub sidebar: Hsla,
    pub surface: Hsla,
    pub surface_2: Hsla,
    pub surface_3: Hsla,
    pub hover: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub text: Hsla,
    pub text_2: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub accent_strong: Hsla,
    pub rt: Hsla,
    /// The actuation point, wherever it is drawn.
    pub actuation: Hsla,
    pub actuation_soft: Hsla,
    /// Text on an accent fill.
    pub on_accent: Hsla,
    pub accent_soft: Hsla,
    pub danger: Hsla,
    pub danger_soft: Hsla,
    pub ok: Hsla,
    pub warn: Hsla,
    pub key: Hsla,
    pub key_top: Hsla,
    pub key_edge: Hsla,
    pub key_selected_top: Hsla,
    pub key_selected: Hsla,
    pub board_top: Hsla,
    pub board_bottom: Hsla,
    pub key_border: Hsla,
    pub track: Hsla,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}
fn ca(hex_rgba: u32) -> Hsla {
    rgba(hex_rgba).into()
}

impl Theme {
    pub fn new() -> Self {
        Theme {
        bg: c(0x0c0c0c),
        sidebar: c(0x121212),
        surface: c(0x161616),
        surface_2: c(0x1d1d1d),
        surface_3: c(0x252525),
        hover: c(0x252525),
        border: c(0x282828),
        border_strong: c(0x373737),
        text: c(0xededed),
        text_2: c(0xb7b7b7),
        muted: c(0x808080),
        accent: c(0xdedede),
        accent_strong: c(0xf5f5f5),
        rt: c(0xbbbbbb),
        actuation: c(0xa78bfa),
        actuation_soft: ca(0xa78bfa24),
        on_accent: c(0x111111),
        accent_soft: ca(0xffffff29),
        danger: c(0xff5d6c),
        danger_soft: ca(0xff5d6c1f),
        ok: c(0xc9c9c9),
        warn: c(0xa9a9a9),
        key: c(0x1e1e1e),
        key_top: c(0x282828),
        key_edge: c(0x131313),
        key_selected_top: c(0x383838),
        key_selected: c(0x2d2d2d),
        board_top: c(0x191919),
        board_bottom: c(0x111111),
        key_border: ca(0xffffff0a),
        track: c(0x252525),
        }
    }
}

pub const UI_FONT: &str = "Inter";
/// Fixed-width digits for live values, so they don't shift as they change.
pub const NUM_FONT: &str = "JetBrains Mono";
