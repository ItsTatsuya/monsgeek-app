//! Whole-keyboard backup and restore. The JSON format is the same as the earlier Actuate app's,
//! so its backups still restore.

use crate::protocol::macros::{self, Macro, MACRO_INDICES};
use crate::protocol::{
    cmd, fn_system, mag, Checksum, KbOptions, KeyboardClient, MATRIX_KEYS, PAGE_SIZE,
};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

/// Format id from the earlier Actuate app, so backups stay interchangeable.
pub const FORMAT: &str = "actuate-backup";
pub const VERSION: u32 = 1;
const PROFILES: usize = 4;
const PER_KEY_LIMIT: usize = 6;

/// Magnetism sub-tables and their bytes per key, in the order the driver writes them.
const MAG_TABLES: [(u8, usize); 12] = [
    (mag::MODE, 1),
    (mag::ACTUATION, 2),
    (mag::RELEASE, 2),
    (mag::RT_PRESS, 2),
    (mag::RT_RELEASE, 2),
    (mag::DEADZONE_BOTTOM, 2),
    (mag::DKS_TRAVEL, 2),
    (mag::MT_TIME, 1),
    (mag::DEADZONE_TOP, 1),
    (mag::DKS_MODES_READ, 4),
    (mag::SNAP_PARTNER, 1),
    (mag::SWITCH_TYPE, 1),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileBackup {
    pub keymap: Vec<u8>,
    pub mag: BTreeMap<u8, Vec<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceStamp {
    pub id: u32,
    pub name: String,
    pub firmware: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub format: String,
    pub version: u32,
    pub created: String,
    pub device: DeviceStamp,
    pub active_profile: u8,
    pub profiles: Vec<ProfileBackup>,
    pub fn_system: u8,
    pub fn_layers: Vec<Vec<u8>>,
    /// GET_LEDPARAM bytes 1–7, raw so preset colour codes survive.
    pub led: Vec<u8>,
    pub options: KbOptions,
    pub debounce: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_rate: Option<u32>,
    pub macros: BTreeMap<u8, Macro>,
}

pub type Progress<'a> = &'a mut dyn FnMut(usize, usize, &str);

fn tables(kb: &KeyboardClient) -> impl Iterator<Item = (u8, usize)> + '_ {
    MAG_TABLES
        .into_iter()
        .filter(|(sub, _)| *sub != mag::DEADZONE_TOP || kb.info.supports_top_deadzone)
}

fn read_mag(kb: &mut KeyboardClient, sub: u8, width: usize) -> Result<Vec<u8>> {
    kb.query_pages(
        |p| vec![cmd::GET_MULTI_MAGNETISM, sub, 1, p],
        (MATRIX_KEYS * width / 64) as u8,
    )
}

pub fn read(kb: &mut KeyboardClient, name: &str, fn_layers: u8, progress: Progress) -> Result<Backup> {
    let table_list: Vec<_> = tables(kb).collect();
    let total = PROFILES * (1 + table_list.len()) + fn_layers as usize + MACRO_INDICES.len() + 2;
    let mut done = 0;
    let mut step = |label: &str| {
        done += 1;
        progress(done, total, label);
    };

    let active_profile = kb.get_profile()?;
    let options = kb.get_options()?;
    let system = fn_system(options.os);
    let mut profiles = Vec::new();
    let result = (|| -> Result<()> {
        for p in 0..PROFILES as u8 {
            let keymap = kb.get_key_matrix(p, 0)?.concat();
            step(&format!("Profile {} keymap", p + 1));
            // Magnetism tables only answer for the active profile.
            if kb.get_profile()? != p {
                kb.set_profile(p)?;
            }
            let mut mag = BTreeMap::new();
            for &(sub, width) in &table_list {
                mag.insert(sub, read_mag(kb, sub, width)?);
                step(&format!("Profile {} key settings", p + 1));
            }
            profiles.push(ProfileBackup { keymap, mag });
        }
        Ok(())
    })();
    if kb.get_profile()? != active_profile {
        kb.set_profile(active_profile)?;
    }
    result?;

    let mut layers = Vec::new();
    for l in 0..fn_layers {
        layers.push(kb.get_fn_matrix(l, system)?.concat());
        step(&format!("Fn layer {}", l + 1));
    }
    let mut stored = BTreeMap::new();
    for n in MACRO_INDICES {
        let m = kb.get_macro(n)?;
        if !m.steps.is_empty() {
            stored.insert(n, m);
        }
        step("Macros");
    }
    let led = kb.query(&[cmd::GET_LEDPARAM])?[1..8].to_vec();
    let debounce = kb.get_debounce()?;
    step("Lighting");
    let report_rate = if kb.info.supports_report_rate {
        Some(kb.get_report_rate()?)
    } else {
        None
    };
    step("Options");
    Ok(Backup {
        format: FORMAT.into(),
        version: VERSION,
        created: now_iso(),
        device: DeviceStamp {
            id: kb.info.device_id,
            name: name.into(),
            firmware: kb.info.firmware,
        },
        active_profile,
        profiles,
        fn_system: system,
        fn_layers: layers,
        led,
        options,
        debounce,
        report_rate,
        macros: stored,
    })
}

pub fn parse(text: &str) -> Result<Backup> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|_| anyhow!("This file is not valid JSON."))?;
    if v.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        bail!("This file is not a Monsgeek App backup.");
    }
    if v.get("version").and_then(|f| f.as_u64()).unwrap_or(0) > VERSION as u64 {
        bail!("This backup was made by a newer version of the app.");
    }
    let b: Backup =
        serde_json::from_value(v).map_err(|e| anyhow!("This backup is damaged: {e}"))?;
    if b.profiles.len() != PROFILES {
        bail!("This backup is incomplete.");
    }
    Ok(b)
}

pub fn to_json(b: &Backup) -> String {
    serde_json::to_string_pretty(b).unwrap_or_default()
}

pub fn file_name(b: &Backup) -> String {
    let mut slug = String::new();
    for c in b.device.name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    format!(
        "monsgeek-app-{}-{}.json",
        slug.trim_matches('-'),
        b.created.get(..10).unwrap_or("backup")
    )
}

fn changed_keys(a: &[u8], b: &[u8], width: usize) -> Vec<usize> {
    (0..a.len().min(b.len()) / width)
        .filter(|&i| a[i * width..(i + 1) * width] != b[i * width..(i + 1) * width])
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub keymaps: usize,
    pub fn_keys: usize,
    pub key_setting_profiles: Vec<usize>,
    pub macros: Vec<u8>,
    pub lighting: bool,
    pub options: bool,
    pub debounce: bool,
    pub report_rate: bool,
    pub profile: bool,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        *self == Plan::default()
    }

    pub fn lines(&self) -> Vec<String> {
        let plural = |n: usize, one: &str, many: &str| if n == 1 { one.to_string() } else { many.to_string() };
        let mut out = Vec::new();
        if self.keymaps > 0 {
            out.push(format!("{} remapped {} across profiles", self.keymaps, plural(self.keymaps, "key", "keys")));
        }
        if self.fn_keys > 0 {
            out.push(format!("{} Fn {}", self.fn_keys, plural(self.fn_keys, "shortcut", "shortcuts")));
        }
        if !self.key_setting_profiles.is_empty() {
            let list: Vec<String> = self.key_setting_profiles.iter().map(|p| (p + 1).to_string()).collect();
            out.push(format!("Actuation and Rapid Trigger on profile {}", list.join(", ")));
        }
        if !self.macros.is_empty() {
            out.push(format!("{} {}", self.macros.len(), plural(self.macros.len(), "macro", "macros")));
        }
        for (on, label) in [
            (self.lighting, "Lighting"),
            (self.options, "Keyboard options"),
            (self.debounce, "Debounce"),
            (self.report_rate, "Polling rate"),
            (self.profile, "Active profile"),
        ] {
            if on {
                out.push(label.into());
            }
        }
        out
    }
}

/// What a restore would change, comparing the backup with the keyboard's current state.
pub fn plan(current: &Backup, target: &Backup) -> Plan {
    let numbers: BTreeSet<u8> = current.macros.keys().chain(target.macros.keys()).copied().collect();
    let encoded = |m: Option<&Macro>| m.map(macros::encode).unwrap_or_default();
    let opts = |o: &KbOptions| (o.fn_index, o.anti_mistouch, o.rt_stability, o.wasd_swap);
    Plan {
        keymaps: target
            .profiles
            .iter()
            .zip(&current.profiles)
            .map(|(t, c)| changed_keys(&c.keymap, &t.keymap, 4).len())
            .sum(),
        fn_keys: target
            .fn_layers
            .iter()
            .zip(&current.fn_layers)
            .map(|(t, c)| changed_keys(c, t, 4).len())
            .sum(),
        key_setting_profiles: (0..PROFILES)
            .filter(|&p| {
                target.profiles[p].mag.iter().any(|(sub, want)| {
                    current.profiles[p].mag.get(sub).is_some_and(|have| have != want)
                })
            })
            .collect(),
        macros: numbers
            .into_iter()
            .filter(|n| encoded(current.macros.get(n)) != encoded(target.macros.get(n)))
            .collect(),
        lighting: current.led != target.led,
        options: opts(&current.options) != opts(&target.options),
        debounce: current.debounce != target.debounce,
        report_rate: matches!((current.report_rate, target.report_rate), (Some(a), Some(b)) if a != b),
        profile: current.active_profile != target.active_profile,
    }
}

/// Frames that turn one profile's magnetism tables into the backup's, ending with the commit flag.
fn mag_frames(kb: &KeyboardClient, current: &BTreeMap<u8, Vec<u8>>, target: &BTreeMap<u8, Vec<u8>>) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    for (sub, width) in tables(kb) {
        let (Some(want), Some(have)) = (target.get(&sub), current.get(&sub)) else {
            continue;
        };
        let changed = changed_keys(have, want, width);
        if changed.is_empty() {
            continue;
        }
        if sub == mag::DKS_MODES_READ || changed.len() <= PER_KEY_LIMIT {
            let write_sub = if sub == mag::DKS_MODES_READ { mag::DKS_MODES } else { sub };
            for i in changed {
                let mut f = vec![cmd::SET_MULTI_MAGNETISM, write_sub, 0, i as u8, 0, 0, 0, 0];
                f.extend_from_slice(&want[i * width..(i + 1) * width]);
                frames.push(f);
            }
        } else {
            let bytes = &want[..(MATRIX_KEYS * width).min(want.len())];
            for (p, chunk) in bytes.chunks(PAGE_SIZE).enumerate() {
                let mut f = vec![cmd::SET_MULTI_MAGNETISM, sub, 1, p as u8, 0, 0, 0, 0];
                f.extend_from_slice(chunk);
                frames.push(f);
            }
        }
    }
    if let Some(last) = frames.last_mut() {
        last[4] = 1;
    }
    frames
}

/// Write the backup to the keyboard, skipping everything that already matches. Macros go
/// first so restored keys never point at stale ones.
pub fn restore(kb: &mut KeyboardClient, current: &Backup, target: &Backup, progress: Progress) -> Result<()> {
    let plan = plan(current, target);
    let total = plan.macros.len() + PROFILES + target.fn_layers.len() + plan.key_setting_profiles.len() + 4;
    let mut done = 0;
    let mut step = |label: &str| {
        done += 1;
        progress(done, total, label);
    };
    for &n in &plan.macros {
        kb.set_macro(n, &target.macros.get(&n).cloned().unwrap_or_default())?;
        step(&format!("Macro {}", n / 2 + 1));
    }
    for p in 0..PROFILES {
        let want = &target.profiles[p].keymap;
        for i in changed_keys(&current.profiles[p].keymap, want, 4) {
            if i < MATRIX_KEYS {
                kb.set_key(p as u8, 0, i as u8, [want[i * 4], want[i * 4 + 1], want[i * 4 + 2], want[i * 4 + 3]])?;
            }
        }
        step(&format!("Profile {} keymap", p + 1));
    }
    // Fn layers are stored per OS mode; write them for the mode the keyboard is in now.
    let system = fn_system(current.options.os);
    for (l, want) in target.fn_layers.iter().enumerate() {
        if let Some(have) = current.fn_layers.get(l) {
            for i in changed_keys(have, want, 4) {
                if i < MATRIX_KEYS {
                    kb.set_fn_key(l as u8, i as u8, [want[i * 4], want[i * 4 + 1], want[i * 4 + 2], want[i * 4 + 3]], system)?;
                }
            }
        }
        step(&format!("Fn layer {}", l + 1));
    }
    let result = (|| -> Result<()> {
        for &p in &plan.key_setting_profiles {
            if kb.get_profile()? != p as u8 {
                kb.set_profile(p as u8)?;
            }
            let frames = mag_frames(kb, &current.profiles[p].mag, &target.profiles[p].mag);
            if !frames.is_empty() {
                kb.write(&frames, Checksum::Bit7, Duration::from_millis(300))?;
            }
            step(&format!("Profile {} key settings", p + 1));
        }
        Ok(())
    })();
    if kb.get_profile()? != target.active_profile {
        kb.set_profile(target.active_profile)?;
    }
    result?;
    if plan.lighting {
        let mut f = vec![cmd::SET_LEDPARAM];
        f.extend_from_slice(&target.led);
        kb.write(&[f], Checksum::Bit8, Duration::from_millis(100))?;
    }
    step("Lighting");
    // The OS mode is set on the keyboard itself, so keep the current one.
    if plan.options {
        kb.set_options(&KbOptions {
            os: current.options.os,
            ..target.options
        })?;
    }
    step("Options");
    if plan.debounce {
        kb.set_debounce(target.debounce)?;
    }
    if let (true, Some(hz)) = (plan.report_rate, target.report_rate) {
        kb.set_report_rate(hz)?;
    }
    step("Debounce");
    step("Done");
    Ok(())
}

/// Re-read the keyboard, then restore against that read: edits made since the comparison
/// shown to the user must not be skipped as "already matching".
pub fn restore_fresh(kb: &mut KeyboardClient, name: &str, fn_layers: u8, target: &Backup, progress: Progress) -> Result<()> {
    let current = read(kb, name, fn_layers, &mut |d, t, l| progress(d, t, &format!("Re-reading the keyboard · {l}")))?;
    restore(kb, &current, target, &mut |d, t, l| progress(d, t, &format!("Restoring · {l}")))
}

/// UTC timestamp in the ISO 8601 form JavaScript's toISOString() writes.
fn now_iso() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let (days, rem) = ((secs / 86400) as i64, secs % 86400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + (month <= 2) as i64;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        d.subsec_millis()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::macros::MacroStep;
    use crate::protocol::mock::MockTransport;

    fn client() -> KeyboardClient {
        let mut kb = KeyboardClient::new(Box::new(MockTransport::new(crate::devices::find(2304).unwrap())));
        kb.identify().unwrap();
        kb
    }

    #[test]
    fn backup_restores_onto_a_changed_keyboard() {
        let mut kb = client();
        kb.set_key(1, 0, 9, [0, 0, 0x29, 0]).unwrap();
        kb.set_macro(
            2,
            &Macro {
                repeat: 1,
                steps: vec![MacroStep { key: 4, down: true, delay: 10 }, MacroStep { key: 4, down: false, delay: 10 }],
            },
        )
        .unwrap();
        let mut keys = kb.get_key_settings().unwrap();
        let before = keys.clone();
        keys[14].actuation = 0.5;
        kb.apply_key_settings(&before, &keys).unwrap();
        let saved = read(&mut kb, "FUN60 PRO", 2, &mut |_, _, _| {}).unwrap();
        let json = to_json(&saved);
        let parsed = parse(&json).unwrap();
        assert_eq!(parsed.macros.len(), 1);

        // Wipe and restore.
        kb.factory_reset().unwrap();
        let current = read(&mut kb, "FUN60 PRO", 2, &mut |_, _, _| {}).unwrap();
        let p = plan(&current, &parsed);
        assert_eq!(p.keymaps, 1);
        assert_eq!(p.key_setting_profiles, vec![0]);
        assert_eq!(p.macros, vec![2]);
        restore(&mut kb, &current, &parsed, &mut |_, _, _| {}).unwrap();
        assert_eq!(kb.get_key_matrix(1, 0).unwrap()[9], [0, 0, 0x29, 0]);
        assert!((kb.get_key_settings().unwrap()[14].actuation - 0.5).abs() < 1e-4);
        assert_eq!(kb.get_macro(2).unwrap().steps.len(), 2);
        let after = read(&mut kb, "FUN60 PRO", 2, &mut |_, _, _| {}).unwrap();
        assert!(plan(&after, &parsed).is_empty());
    }

    #[test]
    fn restore_uses_the_keyboard_as_it_is_now() {
        let mut kb = client();
        let saved = read(&mut kb, "FUN60 PRO", 2, &mut |_, _, _| {}).unwrap();
        // The comparison read happens when the file is opened…
        let stale = read(&mut kb, "FUN60 PRO", 2, &mut |_, _, _| {}).unwrap();
        assert!(plan(&stale, &saved).is_empty());
        // …and the user edits the keyboard afterwards.
        kb.set_key(0, 0, 9, [0, 0, 0x29, 0]).unwrap();
        restore_fresh(&mut kb, "FUN60 PRO", 2, &saved, &mut |_, _, _| {}).unwrap();
        assert_eq!(kb.get_key_matrix(0, 0).unwrap()[9], saved.profiles[0].keymap[36..40]);
        let after = read(&mut kb, "FUN60 PRO", 2, &mut |_, _, _| {}).unwrap();
        assert!(plan(&after, &saved).is_empty());
    }

    #[test]
    fn rejects_other_files() {
        assert!(parse("nope").is_err());
        assert!(parse(r#"{"format":"other"}"#).is_err());
        assert!(now_iso().ends_with('Z'));
        assert_eq!(now_iso().len(), 24);
    }
}
