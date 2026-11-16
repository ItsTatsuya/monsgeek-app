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
    /// A selected item's own surface: the chosen segment, the current page in the sidebar.
    pub raised: Hsla,
    /// The sunken track a segmented control sits in.
    pub well: Hsla,
    /// Ordinary (non-primary) button fill, and its hover.
    pub control: Hsla,
    pub control_hover: Hsla,
    /// Switch track when on and off, and the thumb in both states.
    pub switch_on: Hsla,
    pub switch_off: Hsla,
    pub switch_thumb: Hsla,
    /// Keyboard focus ring.
    pub focus: Hsla,
    /// Soft ring drawn round a slider thumb while it is hovered or dragged.
    pub halo: Hsla,
    /// The small drop shadow under a slider thumb.
    pub thumb_shadow: Hsla,
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

    /// Neutral greys with a faint cool cast, stepped so each layer reads against the one below.
    pub fn dark() -> Self {
        Theme {
            bg: c(0x0a0a0c),
            sidebar: c(0x0e0e11),
            surface: c(0x131316),
            surface_2: c(0x19191d),
            surface_3: c(0x222227),
            hover: c(0x2a2a30),
            border: c(0x222226),
            border_strong: c(0x313137),
            text: c(0xeeeef0),
            text_2: c(0xb9b9c0),
            muted: c(0x8a8a93),
            accent: c(0xe8e8ec),
            accent_strong: c(0xffffff),
            rt: c(0xb9b9c0),
            actuation: c(0xa78bfa),
            on_accent: c(0x0e0e11),
            accent_soft: ca(0xffffff24),
            danger: c(0xff6b78),
            danger_soft: ca(0xff6b781f),
            ok: c(0xcfcfd4),
            warn: c(0xa9a9b0),
            key: c(0x1c1c20),
            key_top: c(0x27272c),
            key_edge: c(0x111114),
            key_selected_top: c(0x3a3a41),
            key_selected: c(0x2e2e34),
            board_top: c(0x18181c),
            board_bottom: c(0x101013),
            key_border: ca(0xffffff0d),
            track: c(0x2a2a30),
            slider_thumb: c(0xffffff),
            logo_top: LOGO_TOP,
            logo_bottom: LOGO_BOTTOM,
            logo_mark: LOGO_MARK,
            logo_border: LOGO_BORDER,
            raised: c(0x2c2c32),
            well: c(0x0d0d10),
            control: c(0x1e1e23),
            control_hover: c(0x28282e),
            switch_on: c(0x7a7a84),
            switch_off: c(0x34343a),
            switch_thumb: c(0xffffff),
            focus: c(0xb9b9c0),
            halo: ca(0xffffff1f),
            thumb_shadow: ca(0x00000066),
        }
    }

    pub fn light() -> Self {
        Theme {
            bg: c(0xf4f4f6),
            sidebar: c(0xececef),
            surface: c(0xffffff),
            surface_2: c(0xf4f4f6),
            surface_3: c(0xe8e8eb),
            hover: c(0xe4e4e8),
            border: c(0xe2e2e6),
            border_strong: c(0xd1d1d6),
            text: c(0x16161a),
            text_2: c(0x404046),
            muted: c(0x6a6a72),
            accent: c(0x1c1c21),
            accent_strong: c(0x000000),
            rt: c(0x5a5a62),
            actuation: c(0x7c3aed),
            on_accent: c(0xfafafa),
            accent_soft: ca(0x16161a14),
            danger: c(0xd02a3c),
            danger_soft: ca(0xd02a3c14),
            ok: c(0x3d3d44),
            warn: c(0x5a5a62),
            key: c(0xe6e6ea),
            key_top: c(0xfcfcfd),
            key_edge: c(0xc6c6cc),
            key_selected_top: c(0xdcdce1),
            key_selected: c(0xcfcfd5),
            board_top: c(0xe4e4e8),
            board_bottom: c(0xd8d8dd),
            key_border: ca(0x00000012),
            track: c(0xe0e0e5),
            slider_thumb: c(0xffffff),
            logo_top: LOGO_TOP,
            logo_bottom: LOGO_BOTTOM,
            logo_mark: LOGO_MARK,
            logo_border: LOGO_BORDER,
            raised: c(0xffffff),
            well: c(0xe9e9ed),
            control: c(0xffffff),
            control_hover: c(0xf1f1f4),
            switch_on: c(0x1c1c21),
            switch_off: c(0xcbcbd1),
            switch_thumb: c(0xffffff),
            focus: c(0x404046),
            halo: ca(0x16161a1a),
            thumb_shadow: ca(0x00000033),
        }
    }
}

pub const UI_FONT: &str = "Inter";

/// Inter's tabular figures: every digit is the same width, so live values don't jitter as
/// they change and columns of numbers line up.
pub fn tabular() -> gpui::FontFeatures {
    gpui::FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
}

/// Numeric text: the interface font with tabular figures.
pub trait Numeric: gpui::Styled + Sized {
    fn numeric(self) -> Self {
        self.font_family(UI_FONT).font_features(tabular())
    }
}
impl<T: gpui::Styled> Numeric for T {}

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
        k.mono_font_family = "JetBrains Mono".into();
        k.font_size = gpui::px(font_size);
        k.radius = gpui::px(8.);
        k.shadow = true;
        k.background = t.bg;
        k.foreground = t.text;
        // Popups float above the cards, so they take the next surface up in dark mode.
        k.popover = if dark { t.surface_2 } else { t.surface };
        k.popover_foreground = t.text;
        k.border = t.border_strong;
        k.input = t.border_strong;
        k.ring = t.focus;
        k.caret = t.text;
        k.selection = t.accent_soft;
        k.primary = t.accent;
        k.primary_hover = t.accent_strong;
        k.primary_active = t.accent_strong;
        k.primary_foreground = t.on_accent;
        k.secondary = t.surface_2;
        k.secondary_hover = t.hover;
        // Ghost buttons show "selected" (the current page, the chosen segment) with this.
        k.secondary_active = t.raised;
        k.secondary_foreground = t.text;
        k.danger = t.danger;
        // Dark text on the lighter dark-mode red; white on the deeper light-mode red.
        let on_danger = if dark { t.on_accent } else { gpui::white() };
        k.danger_foreground = on_danger;
        // Ghost and menu hover.
        k.accent = t.hover;
        k.accent_foreground = t.text;
        k.muted = t.surface_2;
        k.muted_foreground = t.muted;
        // The kit resolves its button colours from primary/secondary only when a theme
        // loads, so set them too or they keep the kit's defaults.
        k.button = t.control;
        k.button_hover = t.control_hover;
        k.button_active = t.surface_3;
        k.button_foreground = t.text;
        k.button_primary = t.accent;
        k.button_primary_hover = t.accent_strong;
        k.button_primary_active = t.accent;
        k.button_primary_foreground = t.on_accent;
        k.button_secondary = t.surface_2;
        k.button_secondary_hover = t.hover;
        k.button_secondary_active = t.surface_3;
        k.button_secondary_foreground = t.text;
        k.button_danger = t.danger;
        k.button_danger_hover = t.danger.opacity(0.88);
        k.button_danger_active = t.danger.opacity(0.8);
        k.button_danger_foreground = on_danger;
        k.colors.list = t.surface;
        k.list_hover = t.hover;
        k.list_active = t.raised;
        k.list_active_border = t.border_strong;
        k.scrollbar = gpui::transparent_black();
        k.scrollbar_thumb = t.muted.opacity(0.35);
        k.scrollbar_thumb_hover = t.muted.opacity(0.6);
        k.tab_bar = t.surface_2;
        k.tab_bar_segmented = t.well;
        k.tab_active = t.raised;
        k.tab_active_foreground = t.text;
        k.tab_foreground = t.muted;
        k.slider_bar = t.track;
        k.slider_thumb = t.slider_thumb;
        k.switch = t.switch_off;
        k.switch_thumb = t.switch_thumb;
        k.sidebar = t.sidebar;
        k.sidebar_border = t.border;
        k.sidebar_foreground = t.text;
        k.sidebar_accent = t.raised;
        k.sidebar_accent_foreground = t.text;
    });
}
