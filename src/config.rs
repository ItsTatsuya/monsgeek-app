//! App preferences in %APPDATA%\Monsgeek App\config.json.

use crate::layer_lighting::LayerLooks;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoRule {
    pub exe: String,
    pub profile: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoProfiles {
    pub enabled: bool,
    /// Profile for apps without a rule; None leaves the profile alone.
    pub default_profile: Option<u8>,
    pub rules: Vec<AutoRule>,
    /// Keep running in the tray when the window is closed.
    pub tray: bool,
}

impl Default for AutoProfiles {
    fn default() -> Self {
        Self {
            enabled: false,
            default_profile: None,
            rules: Vec::new(),
            tray: true,
        }
    }
}

impl AutoProfiles {
    /// Profile the rules pick for an exe, or None to leave the keyboard alone.
    pub fn target(&self, exe: &str) -> Option<u8> {
        self.rules
            .iter()
            .find(|r| r.exe.eq_ignore_ascii_case(exe))
            .map(|r| r.profile)
            .or(self.default_profile)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub auto_profiles: AutoProfiles,
    /// Lighting per profile and Fn layer, by device id.
    pub layer_looks: BTreeMap<u32, LayerLooks>,
    /// Names the user gave the four onboard profiles, by device id. Empty means "Profile N".
    pub profile_names: BTreeMap<u32, [String; 4]>,
    /// Start hidden in the tray (used when the app opens at sign-in).
    pub start_in_tray: bool,
    /// Interface size: the base font size in pixels (None is the default size).
    pub ui_scale: Option<f32>,
    pub appearance: Appearance,
}

/// Light or dark, or follow Windows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

/// The interface sizes offered in Settings and by Ctrl + / Ctrl −, as base font sizes.
pub const UI_SIZES: [f32; 4] = [12.0, 14.0, 16.0, 18.0];

fn path_in(folder: &str) -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join(folder).join("config.json"))
}

fn path() -> Option<PathBuf> {
    path_in("Monsgeek App")
}

/// Default base font size, in pixels; `ui_scale` overrides it.
pub const BASE_FONT: f32 = 14.0;

impl Config {
    /// Loads the settings. A file that exists but can't be parsed is moved aside (so the next
    /// save can't overwrite it) and the returned message says where it went.
    pub fn load() -> (Self, Option<String>) {
        let Some(p) = path() else {
            return (Self::default(), None);
        };
        match std::fs::read_to_string(&p) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(c) => (c, None),
                Err(e) => {
                    let aside = p.with_extension("json.bad");
                    let moved = std::fs::rename(&p, &aside).is_ok();
                    let msg = if moved {
                        format!(
                            "Couldn't read your settings ({e}). They were moved to {} and defaults are in use.",
                            aside.display()
                        )
                    } else {
                        format!("Couldn't read your settings ({e}). Defaults are in use.")
                    };
                    (Self::default(), Some(msg))
                }
            },
            // Settings saved under the app's earlier name carry over until the first save.
            Err(_) => (
                path_in("Actuate")
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default(),
                None,
            ),
        }
    }

    /// Writes the settings atomically: a temporary file next to the real one, then a rename,
    /// so a crash mid-write never leaves a truncated config behind.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(p) = path() else {
            return Ok(());
        };
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &p)
    }

    /// The base font size in pixels.
    pub fn font_size(&self) -> f32 {
        self.ui_scale
            .filter(|s| UI_SIZES.contains(s))
            .unwrap_or(BASE_FONT)
    }

    /// The name shown for a profile (0-based).
    pub fn profile_name(&self, device: u32, profile: u8) -> String {
        self.profile_names
            .get(&device)
            .and_then(|n| n.get(profile as usize))
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Profile {}", profile + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_match_without_case_and_fall_back_to_the_default() {
        let mut a = AutoProfiles {
            rules: vec![AutoRule {
                exe: "Game.exe".into(),
                profile: 2,
            }],
            ..Default::default()
        };
        assert_eq!(a.target("game.EXE"), Some(2));
        assert_eq!(a.target("explorer.exe"), None);
        a.default_profile = Some(0);
        assert_eq!(a.target("explorer.exe"), Some(0));
        // Older config files (with a theme, without the new fields) still load.
        let old: Config = serde_json::from_str(r#"{"theme":"Light","blur":true}"#).unwrap();
        assert!(!old.auto_profiles.enabled && old.auto_profiles.tray);
    }
}
