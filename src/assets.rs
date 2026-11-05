//! Sidebar icons, GPUI Kit component icons and bundled typography, available offline.

use gpui::{App, AssetSource, SharedString};
use std::borrow::Cow;

pub struct Assets;

const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/gauge.svg",
        include_bytes!("../assets/icons/gauge.svg"),
    ),
    (
        "icons/remap.svg",
        include_bytes!("../assets/icons/remap.svg"),
    ),
    ("icons/bulb.svg", include_bytes!("../assets/icons/bulb.svg")),
    ("icons/bolt.svg", include_bytes!("../assets/icons/bolt.svg")),
    ("icons/swap.svg", include_bytes!("../assets/icons/swap.svg")),
    ("icons/logo.svg", include_bytes!("../assets/icons/logo.svg")),
    (
        "icons/settings.svg",
        include_bytes!("../assets/icons/settings.svg"),
    ),
    (
        "icons/keyboard.svg",
        include_bytes!("../assets/icons/keyboard.svg"),
    ),
    (
        "icons/pulse.svg",
        include_bytes!("../assets/icons/pulse.svg"),
    ),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        match ICONS.iter().find(|(name, _)| *name == path) {
            Some((_, bytes)) => Ok(Some(Cow::Borrowed(*bytes))),
            // Component icons (colour picker, inputs) come from GPUI Kit.
            None => gpui::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut names: Vec<SharedString> = ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| (*name).into())
            .collect();
        names.extend(gpui::assets::Assets.list(path)?);
        names.sort();
        names.dedup();
        Ok(names)
    }
}

pub fn load_fonts(cx: &App) {
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/Inter.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf")),
        ])
        .expect("load bundled Inter and JetBrains Mono fonts");
}
