//! The app's design tokens: a monochrome scheme in dark and light, plus the roles the GPUI Kit
//! theme doesn't have (keycaps, travel and actuation colours). Colour values live only here.
//!
//! The kit theme is the source of the current mode: [`apply`] picks the mode (following the
//! system unless the user chose one) and writes the matching values into the kit theme, so kit
//! components and the app's own surfaces always agree.

use crate::config::Appearance;
use gpui::{rgb, rgba, rems, Hsla, Rems};

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
    pub slider_thumb: Hsla,
    /// The app logo tile.
    pub logo_top: Hsla,
    pub logo_bottom: Hsla,
    pub logo_mark: Hsla,
    pub logo_border: Hsla,
}

// The logo tile is the same in both modes: it matches the exe icon.
const LOGO_TOP: Hsla = Hsla { h: 0., s: 0., l: 0.18, a: 1. };
const LOGO_BOTTOM: Hsla = Hsla { h: 0., s: 0., l: 0.055, a: 1. };
const LOGO_MARK: Hsla = Hsla { h: 0., s: 0., l: 0.957, a: 1. };
const LOGO_BORDER: Hsla = Hsla { h: 0., s: 0., l: 1., a: 0.08 };

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}
fn ca(hex_rgba: u32) -> Hsla {
    rgba(hex_rgba).into()
}

impl Theme {
    pub fn for_mode(dark: bool) -> Self {
        if dark {
            Self::dark()
        } else {
            Self::light()
        }
    }

    pub fn dark() -> Self {
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
            slider_thumb: c(0xffffff),
            logo_top: LOGO_TOP,
            logo_bottom: LOGO_BOTTOM,
            logo_mark: LOGO_MARK,
            logo_border: LOGO_BORDER,
        }
    }

    pub fn light() -> Self {
        Theme {
            bg: c(0xf5f5f5),
            sidebar: c(0xededed),
            surface: c(0xffffff),
            surface_2: c(0xf2f2f2),
            surface_3: c(0xe6e6e6),
            hover: c(0xe9e9e9),
            border: c(0xe0e0e0),
            border_strong: c(0xcccccc),
            text: c(0x161616),
            text_2: c(0x474747),
            muted: c(0x6e6e6e),
            accent: c(0x222222),
            accent_strong: c(0x000000),
            rt: c(0x5c5c5c),
            actuation: c(0x7c3aed),
            on_accent: c(0xfafafa),
            accent_soft: ca(0x00000014),
            danger: c(0xcf2638),
            danger_soft: ca(0xcf263814),
            ok: c(0x3d3d3d),
            warn: c(0x5c5c5c),
            key: c(0xe4e4e4),
            key_top: c(0xfafafa),
            key_edge: c(0xc4c4c4),
            key_selected_top: c(0xd9d9d9),
            key_selected: c(0xcdcdcd),
            board_top: c(0xe2e2e2),
            board_bottom: c(0xd6d6d6),
            key_border: ca(0x00000012),
            track: c(0xdedede),
            slider_thumb: c(0xffffff),
            logo_top: LOGO_TOP,
            logo_bottom: LOGO_BOTTOM,
            logo_mark: LOGO_MARK,
            logo_border: LOGO_BORDER,
        }
    }
}

pub const UI_FONT: &str = "Inter";
/// Fixed-width digits for live values, so they don't shift as they change.
pub const NUM_FONT: &str = "JetBrains Mono";

/// A length from the design's pixel grid, as `rem`s, so it scales with the interface size.
/// The design was drawn at a 14 px base font; at that size `u(16.)` is 16 px.
pub fn u(design_px: f32) -> Rems {
    rems(design_px / crate::config::BASE_FONT)
}

/// Whether the app should be dark, for the user's choice and the window's appearance.
pub fn is_dark(choice: Appearance, window: &gpui::Window) -> bool {
    match choice {
        Appearance::Dark => true,
        Appearance::Light => false,
        Appearance::System => matches!(
            window.appearance(),
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        ),
    }
}

/// Sets the kit theme's mode, colours and base font size to match the app. Call after
/// `gpui::init`, and again when the appearance, the user's choice or the interface size changes.
pub fn apply(dark: bool, font_size: f32, cx: &mut gpui::App) {
    use gpui::component::{Theme as Kit, ThemeMode};
    let t = Theme::for_mode(dark);
    Kit::change(if dark { ThemeMode::Dark } else { ThemeMode::Light }, None, cx);
    Kit::update(cx, |k| {
        k.font_family = UI_FONT.into();
        k.font_size = gpui::px(font_size);
        k.radius = gpui::px(8.);
        k.shadow = true;
        k.background = t.bg;
        k.foreground = t.text;
        k.popover = t.surface;
        k.popover_foreground = t.text;
        k.border = t.border_strong;
        k.input = t.border_strong;
        k.ring = t.text_2;
        k.caret = t.text;
        k.selection = t.accent_soft;
        k.primary = t.accent;
        k.primary_hover = t.accent_strong;
        k.primary_active = t.accent_strong;
        k.primary_foreground = t.on_accent;
        k.secondary = t.surface_2;
        k.secondary_hover = t.hover;
        k.secondary_active = t.surface_3;
        k.secondary_foreground = t.text;
        k.danger = t.danger;
        k.accent = t.hover;
        k.accent_foreground = t.text;
        k.muted = t.surface_2;
        k.muted_foreground = t.muted;
        k.tab_bar = t.surface_2;
        k.tab_active = t.surface_3;
        k.slider_bar = t.track;
        k.slider_thumb = t.slider_thumb;
        k.switch = t.surface_3;
    });
}
