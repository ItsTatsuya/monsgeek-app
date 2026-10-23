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
}

fn path_in(folder: &str) -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join(folder).join("config.json"))
}

fn path() -> Option<PathBuf> {
    path_in("Monsgeek App")
}

impl Config {
    pub fn load() -> Self {
        // Settings saved under the app's earlier name carry over until the first save.
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .or_else(|| path_in("Actuate").and_then(|p| std::fs::read_to_string(p).ok()))
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, serde_json::to_string_pretty(self).unwrap_or_default());
        }
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
