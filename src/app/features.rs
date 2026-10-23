//! State and keyboard jobs for advanced keys, macros, per-key colours, calibration,
//! backup/restore, polling rate and switch type.

use super::{AppView, Page};
use crate::backup::{self, Backup};
use crate::keycodes::char_to_hid;
use crate::protocol::advanced::{DksConfig, ModTapConfig, MT_DEFAULT_TIME};
use crate::protocol::macros::{self, play, Macro, MacroStep, MACRO_ACTION, MACRO_INDICES};
use crate::protocol::userpic::{picture_count, picture_lighting, Rgb, USER_PICTURE_EFFECT};
use crate::protocol::{fn_system, mode, KeyAction, KeyHe};
use anyhow::anyhow;
use gpui::{Context, Keystroke, Modifiers};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TAP_DELAY: u16 = 20;
const SHIFT: u8 = 0xe1;
/// The driver counts a key as calibrated once its reading reaches 300 (about 3 mm).
pub const CAL_DONE_AT: u16 = 300;
const REST_SECS: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemapTab {
    Key,
    Macro,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LightingTab {
    Effects,
    PerKey,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TesterTab {
    Switch,
    Polling,
    Calibration,
}

pub enum AdvEditor {
    Dks {
        index: usize,
        cfg: Option<DksConfig>,
        row: usize,
    },
    ModTap {
        index: usize,
        cfg: Option<ModTapConfig>,
        hold: bool,
    },
}

impl AdvEditor {
    pub fn index(&self) -> usize {
        match self {
            AdvEditor::Dks { index, .. } | AdvEditor::ModTap { index, .. } => *index,
        }
    }
}

pub struct MacroDraft {
    pub index: usize,
    pub layer: u8,
    /// Macro number the key already plays (readable ones only).
    pub bound: Option<u8>,
    /// The key plays an odd macro, which this firmware can't read back.
    pub unreadable: bool,
    pub loading: bool,
    pub draft: Macro,
    pub mode: u8,
    pub dirty: bool,
    pub recording: bool,
    pub real_timing: bool,
    /// Modifiers held around the next key tap, for shortcuts like Ctrl+Alt+K.
    pub mods: Vec<u8>,
    last_event: Instant,
    held: Vec<u8>,
    held_mods: Modifiers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Search,
    MacroText,
    Repeat,
    Delay(usize),
    RuleExe,
}

pub struct TextEdit {
    pub field: Field,
    pub text: String,
}

pub struct PerKey {
    pub picture: u8,
    pub colors: Option<Vec<Rgb>>,
    pub color: Rgb,
    pub dirty: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CalStep {
    Idle,
    Confirm,
    Rest(u8),
    /// Rest measured; switching the keyboard to bottom calibration.
    Starting,
    Bottom,
    ConfirmFinish,
    Saving,
    Done,
}

pub struct Calibration {
    pub step: CalStep,
    /// Deepest reading per key during this run.
    pub peaks: Vec<u16>,
    gen: u64,
}

pub type ProgressCell = Arc<Mutex<(usize, usize, String)>>;

#[derive(Default)]
pub struct BackupState {
    pub busy: Option<ProgressCell>,
    pub error: Option<String>,
    pub file: Option<(Backup, String)>,
    pub current: Option<Backup>,
}

pub struct Features {
    pub remap_tab: RemapTab,
    pub lighting_tab: LightingTab,
    pub tester_tab: TesterTab,
    pub adv: Option<AdvEditor>,
    pub macro_draft: Option<MacroDraft>,
    pub edit: Option<TextEdit>,
    /// A field took this click, so the root shouldn't commit it.
    pub field_clicked: bool,
    pub search: String,
    pub macro_text: String,
    pub perkey: Option<PerKey>,
    pub cal: Calibration,
    pub backup: BackupState,
    pub switch_menu: bool,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            remap_tab: RemapTab::Key,
            lighting_tab: LightingTab::Effects,
            tester_tab: TesterTab::Switch,
            adv: None,
            macro_draft: None,
            edit: None,
            field_clicked: false,
            search: String::new(),
            macro_text: String::new(),
            perkey: None,
            cal: Calibration {
                step: CalStep::Idle,
                peaks: Vec::new(),
                gen: 0,
            },
            backup: BackupState::default(),
            switch_menu: false,
        }
    }
}

/// A key tap wrapped in held modifiers: Ctrl↓ Alt↓ K↓ K↑ Alt↑ Ctrl↑.
pub fn tap(key: u8, delay: u16, mods: &[u8]) -> Vec<MacroStep> {
    let mut out: Vec<MacroStep> = mods
        .iter()
        .map(|&m| MacroStep {
            key: m,
            down: true,
            delay: 5,
        })
        .collect();
    out.push(MacroStep {
        key,
        down: true,
        delay,
    });
    out.push(MacroStep {
        key,
        down: false,
        delay,
    });
    out.extend(mods.iter().rev().map(|&m| MacroStep {
        key: m,
        down: false,
        delay: 5,
    }));
    out
}

/// GPUI key name (Windows) → HID usage, for recording macros.
pub fn key_to_hid(key: &str) -> Option<u8> {
    Some(match key {
        "enter" => 0x28,
        "escape" => 0x29,
        "backspace" => 0x2a,
        "tab" => 0x2b,
        "space" => 0x2c,
        "insert" => 0x49,
        "home" => 0x4a,
        "pageup" => 0x4b,
        "delete" => 0x4c,
        "end" => 0x4d,
        "pagedown" => 0x4e,
        "right" => 0x4f,
        "left" => 0x50,
        "down" => 0x51,
        "up" => 0x52,
        "menu" => 0x65,
        k if k.len() > 1 && k.starts_with('f') => {
            let n: u8 = k[1..].parse().ok()?;
            match n {
                1..=12 => 0x3a + n - 1,
                13..=24 => 0x68 + n - 13,
                _ => return None,
            }
        }
        k => {
            let mut chars = k.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            if c == '0' {
                0x27
            } else {
                char_to_hid(c)?.0
            }
        }
    })
}

impl AppView {
    // ---- polling rate and switch type ------------------------------------------------

    pub fn set_report_rate(&mut self, hz: u32, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        let Some(d) = self.device.as_mut() else {
            return;
        };
        d.report_rate = Some(hz);
        self.run(cx, "Setting polling rate", move |kb| kb.set_report_rate(hz), |_, _, _| {});
    }

    pub fn set_switch_type(&mut self, t: u8, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let all: Vec<usize> = d.layout.keys.iter().map(|k| k.index).collect();
        self.features.switch_menu = false;
        self.update_indices(all, move |k| k.switch_type = t, cx);
    }

    // ---- advanced keys ----------------------------------------------------------------

    pub fn make_snap(&mut self, cx: &mut Context<Self>) {
        let [a, b] = self.selection[..] else { return };
        self.update_indices(vec![a], move |k| {
            k.mode = mode::SNAP_TAP;
            k.snap_partner = b as u8;
        }, cx);
        self.update_indices(vec![b], move |k| {
            k.mode = mode::SNAP_TAP;
            k.snap_partner = a as u8;
        }, cx);
        self.selection.clear();
    }

    pub fn set_toggle(&mut self, m: u8, cx: &mut Context<Self>) {
        let sel = self.selection.clone();
        self.update_indices(sel, move |k| k.mode = m, cx);
    }

    /// Remove any advanced behaviour from these keys (and their Snap Tap partners).
    pub fn clear_advanced(&mut self, indices: Vec<usize>, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let mut all = indices.clone();
        for &i in &indices {
            if d.keys[i].mode == mode::SNAP_TAP {
                all.push(d.keys[i].snap_partner as usize);
            }
        }
        let layered: Vec<(usize, KeyAction)> = indices
            .iter()
            .filter(|&&i| matches!(d.keys[i].mode, mode::DKS | mode::MOD_TAP))
            .map(|&i| (i, d.defaults[i]))
            .collect();
        let profile = d.profile;
        // The partner table is left alone: the firmware only reads it in Snap Tap mode.
        self.update_indices(all, |k| k.mode = mode::NORMAL, cx);
        if self
            .features
            .adv
            .as_ref()
            .is_some_and(|e| indices.contains(&e.index()))
        {
            self.features.adv = None;
        }
        if layered.is_empty() {
            return;
        }
        // DKS and mod-tap rewrote the key's sub-layers: put the factory action back.
        let job = layered.clone();
        self.run(
            cx,
            "Resetting key layers",
            move |kb| {
                for (i, base) in job {
                    kb.clear_layers(profile, i, base)?;
                }
                Ok(())
            },
            move |s, _, _| {
                if let Some(d) = s.device.as_mut() {
                    for (i, base) in layered {
                        d.keymap[i] = base;
                    }
                }
            },
        );
    }

    pub fn open_dks(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        self.selection = vec![index];
        let profile = d.profile;
        if d.keys[index].mode != mode::DKS {
            self.features.adv = Some(AdvEditor::Dks {
                index,
                cfg: Some(DksConfig::fresh(d.defaults[index])),
                row: 0,
            });
            cx.notify();
            return;
        }
        self.features.adv = Some(AdvEditor::Dks {
            index,
            cfg: None,
            row: 0,
        });
        self.run(
            cx,
            "Reading DKS",
            move |kb| kb.read_dks(profile, index),
            move |s, cfg, _| {
                if let Some(AdvEditor::Dks { index: i, cfg: slot, .. }) = s.features.adv.as_mut() {
                    if *i == index {
                        *slot = Some(cfg);
                    }
                }
            },
        );
    }

    pub fn open_mod_tap(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        self.selection = vec![index];
        let profile = d.profile;
        if d.keys[index].mode != mode::MOD_TAP {
            // A key new to mod-tap taps as itself and holds as Ctrl until changed.
            self.features.adv = Some(AdvEditor::ModTap {
                index,
                cfg: Some(ModTapConfig {
                    tap: d.defaults[index],
                    hold: [0, 0, 0xe0, 0],
                    time: MT_DEFAULT_TIME,
                }),
                hold: true,
            });
            cx.notify();
            return;
        }
        self.features.adv = Some(AdvEditor::ModTap {
            index,
            cfg: None,
            hold: true,
        });
        self.run(
            cx,
            "Reading mod-tap",
            move |kb| kb.read_mod_tap(profile, index),
            move |s, cfg, _| {
                if let Some(AdvEditor::ModTap { index: i, cfg: slot, .. }) = s.features.adv.as_mut() {
                    if *i == index {
                        *slot = Some(cfg);
                    }
                }
            },
        );
    }

    /// Selecting a single DKS or mod-tap key opens its editor; selecting anything else closes it.
    pub fn sync_adv_editor(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let single = (self.selection.len() == 1).then(|| self.selection[0]);
        match single {
            Some(i) if self.features.adv.as_ref().is_some_and(|e| e.index() == i) => {}
            Some(i) if d.keys[i].mode == mode::DKS => self.open_dks(i, cx),
            Some(i) if d.keys[i].mode == mode::MOD_TAP => self.open_mod_tap(i, cx),
            _ => self.features.adv = None,
        }
    }

    /// Pick an action for the DKS row or mod-tap slot being edited.
    pub fn pick_adv_action(&mut self, a: KeyAction, cx: &mut Context<Self>) {
        match self.features.adv.as_mut() {
            Some(AdvEditor::Dks {
                cfg: Some(cfg), row, ..
            }) => cfg.actions[*row] = a,
            Some(AdvEditor::ModTap {
                cfg: Some(cfg), hold, ..
            }) => {
                if *hold {
                    cfg.hold = a
                } else {
                    cfg.tap = a
                }
            }
            _ => {}
        }
        cx.notify();
    }

    pub fn save_adv(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let profile = d.profile;
        let label = crate::keycodes::action_label(&d.defaults[self.features.adv.as_ref().map_or(0, |e| e.index())]);
        match self.features.adv.as_ref() {
            Some(AdvEditor::Dks {
                index,
                cfg: Some(cfg),
                ..
            }) => {
                let (index, cfg) = (*index, cfg.clone());
                let base = cfg.actions[0];
                self.run(
                    cx,
                    "Saving DKS",
                    move |kb| {
                        kb.write_dks(profile, index, &cfg)?;
                        kb.set_key_mode(index, mode::DKS)
                    },
                    move |s, _, cx| {
                        // Sub-layer 0 is the key's base remap, so keep the Remap page in step.
                        s.saved_key_mode(profile, index, mode::DKS, base);
                        s.toast(format!("DKS saved to {label}"), false, cx);
                    },
                );
            }
            Some(AdvEditor::ModTap {
                index,
                cfg: Some(cfg),
                ..
            }) => {
                let (index, cfg) = (*index, *cfg);
                self.run(
                    cx,
                    "Saving mod-tap",
                    move |kb| {
                        kb.write_mod_tap(profile, index, &cfg)?;
                        kb.set_key_mode(index, mode::MOD_TAP)
                    },
                    move |s, _, cx| {
                        s.saved_key_mode(profile, index, mode::MOD_TAP, cfg.hold);
                        s.toast(format!("Mod-tap saved to {label}"), false, cx);
                    },
                );
            }
            _ => {}
        }
    }

    /// The keyboard now holds `mode` and the sub-layer 0 action for this key.
    fn saved_key_mode(&mut self, profile: u8, index: usize, m: u8, base: KeyAction) {
        let Some(d) = self.device.as_mut() else {
            return;
        };
        if d.profile != profile {
            return;
        }
        d.keymap[index] = base;
        d.keys[index].mode = m;
        d.committed[index].mode = m;
    }

    // ---- macros -------------------------------------------------------------------------

    /// Make sure the macro draft follows the selected key, loading its macro when needed.
    pub fn ensure_macro_draft(&mut self, index: usize, cx: &mut Context<Self>) {
        let layer = self.remap_layer;
        if self
            .features
            .macro_draft
            .as_ref()
            .is_some_and(|m| m.index == index && m.layer == layer)
        {
            return;
        }
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let current = if layer == 0 { d.keymap[index] } else { d.fnmap[index] };
        let is_macro = current[0] == MACRO_ACTION;
        let unreadable = is_macro && !macros::is_readable(current[2]);
        let bound = (is_macro && !unreadable).then_some(current[2]);
        let cached = bound.and_then(|n| d.macros.get(&n).cloned());
        let loading = bound.is_some() && cached.is_none();
        self.features.macro_draft = Some(MacroDraft {
            index,
            layer,
            bound,
            unreadable,
            loading,
            draft: cached.unwrap_or_default(),
            mode: if bound.is_some() { current[1] } else { play::REPEAT },
            dirty: false,
            recording: false,
            real_timing: true,
            mods: Vec::new(),
            last_event: Instant::now(),
            held: Vec::new(),
            held_mods: Modifiers::default(),
        });
        if let (true, Some(n)) = (loading, bound) {
            self.run(
                cx,
                "Reading macro",
                move |kb| kb.get_macro(n),
                move |s, m, _| {
                    if let Some(d) = s.device.as_mut() {
                        d.macros.insert(n, m.clone());
                    }
                    if let Some(md) = s.features.macro_draft.as_mut() {
                        if md.bound == Some(n) && md.loading {
                            md.loading = false;
                            if !md.dirty {
                                md.draft = m;
                            }
                        }
                    }
                },
            );
        }
    }

    pub fn edit_macro(&mut self, f: impl FnOnce(&mut MacroDraft), cx: &mut Context<Self>) {
        if let Some(m) = self.features.macro_draft.as_mut() {
            f(m);
            m.dirty = true;
        }
        cx.notify();
    }

    pub fn revert_macro(&mut self, cx: &mut Context<Self>) {
        if let Some(m) = self.features.macro_draft.take() {
            self.ensure_macro_draft(m.index, cx);
        }
        cx.notify();
    }

    pub fn toggle_recording(&mut self, cx: &mut Context<Self>) {
        if let Some(m) = self.features.macro_draft.as_mut() {
            m.recording = !m.recording;
            m.held.clear();
            m.held_mods = Modifiers::default();
            m.last_event = Instant::now();
        }
        self.features.edit = None;
        cx.notify();
    }

    pub fn recording(&self) -> bool {
        self.features.macro_draft.as_ref().is_some_and(|m| m.recording)
    }

    fn record(&mut self, key: u8, down: bool) {
        let Some(m) = self.features.macro_draft.as_mut() else {
            return;
        };
        if down {
            if m.held.contains(&key) {
                return; // auto-repeat
            }
            m.held.push(key);
        } else {
            if !m.held.contains(&key) {
                return;
            }
            m.held.retain(|&k| k != key);
        }
        let gap = m.last_event.elapsed().as_millis().clamp(1, u16::MAX as u128) as u16;
        m.last_event = Instant::now();
        let real = m.real_timing;
        if let Some(last) = m.draft.steps.last_mut() {
            last.delay = if real { gap } else { TAP_DELAY };
        }
        m.draft.steps.push(MacroStep {
            key,
            down,
            delay: TAP_DELAY,
        });
        m.dirty = true;
    }

    pub fn record_keystroke(&mut self, k: &Keystroke, down: bool) {
        if let Some(hid) = key_to_hid(&k.key) {
            self.record(hid, down);
        }
    }

    pub fn record_modifiers(&mut self, now: Modifiers) {
        let Some(prev) = self.features.macro_draft.as_ref().map(|m| m.held_mods) else {
            return;
        };
        for (was, is, hid) in [
            (prev.control, now.control, 0xe0),
            (prev.shift, now.shift, 0xe1),
            (prev.alt, now.alt, 0xe2),
            (prev.platform, now.platform, 0xe3),
        ] {
            if was != is {
                self.record(hid, is);
            }
        }
        if let Some(m) = self.features.macro_draft.as_mut() {
            m.held_mods = now;
        }
    }

    pub fn add_macro_text(&mut self, cx: &mut Context<Self>) {
        let text = std::mem::take(&mut self.features.macro_text);
        let mut steps = Vec::new();
        for ch in text.chars() {
            let Some((key, shift)) = char_to_hid(ch) else {
                continue;
            };
            if shift {
                steps.push(MacroStep { key: SHIFT, down: true, delay: 5 });
            }
            steps.extend(tap(key, 10, &[]));
            if shift {
                steps.push(MacroStep { key: SHIFT, down: false, delay: 5 });
            }
        }
        self.features.edit = None;
        self.edit_macro(|m| m.draft.steps.extend(steps), cx);
    }

    pub fn add_macro_tap(&mut self, key: u8, cx: &mut Context<Self>) {
        let Some(m) = self.features.macro_draft.as_mut() else {
            return;
        };
        if (0xe0..=0xe7).contains(&key) {
            if m.mods.contains(&key) {
                m.mods.retain(|&k| k != key);
            } else {
                m.mods.push(key);
            }
            cx.notify();
            return;
        }
        let mods = std::mem::take(&mut m.mods);
        self.edit_macro(|m| m.draft.steps.extend(tap(key, TAP_DELAY, &mods)), cx);
    }

    pub fn save_macro(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let Some(m) = self.features.macro_draft.as_ref() else {
            return;
        };
        let (index, layer, bound, draft, play_mode) =
            (m.index, m.layer, m.bound, m.draft.clone(), m.mode);
        let (profile, fn_layer, system) = (d.profile, d.fn_layer, fn_system(d.options.os));
        let fn_layers = d.def.fn_layer_count().max(1);
        self.run(
            cx,
            "Saving macro",
            move |kb| {
                // Keep the key's own slot; otherwise the lowest slot no key on any profile or Fn layer uses.
                let slot = match bound {
                    Some(n) => n,
                    None => {
                        let mut used = Vec::new();
                        for p in 0..4 {
                            used.extend(kb.get_key_matrix(p, 0)?);
                        }
                        for l in 0..fn_layers {
                            used.extend(kb.get_fn_matrix(l, system)?);
                        }
                        MACRO_INDICES
                            .into_iter()
                            .find(|n| !used.iter().any(|a| a[0] == MACRO_ACTION && a[2] == *n))
                            .ok_or_else(|| anyhow!("all {} macro slots are in use", MACRO_INDICES.len()))?
                    }
                };
                kb.set_macro(slot, &draft)?;
                let action = [MACRO_ACTION, play_mode, slot, 0];
                if layer == 0 {
                    kb.set_key(profile, 0, index as u8, action)?;
                } else {
                    kb.set_fn_key(fn_layer, index as u8, action, system)?;
                }
                Ok((slot, action, draft))
            },
            move |s, (slot, action, draft), cx| {
                if let Some(d) = s.device.as_mut() {
                    d.macros.insert(slot, draft);
                    if layer == 0 {
                        d.keymap[index] = action;
                    } else {
                        d.fnmap[index] = action;
                    }
                }
                if let Some(m) = s.features.macro_draft.as_mut() {
                    if m.index == index {
                        m.bound = Some(slot);
                        m.unreadable = false;
                        m.dirty = false;
                    }
                }
                s.toast("Macro saved", false, cx);
            },
        );
    }

    // ---- text fields ----------------------------------------------------------------------

    pub fn begin_edit(&mut self, field: Field, cx: &mut Context<Self>) {
        self.features.field_clicked = true;
        if self.features.edit.as_ref().is_some_and(|e| e.field == field) {
            return;
        }
        self.commit_edit(cx);
        let text = match field {
            Field::Search => self.features.search.clone(),
            Field::RuleExe => self.tools.rule_text.clone(),
            Field::MacroText => self.features.macro_text.clone(),
            Field::Repeat => self
                .features
                .macro_draft
                .as_ref()
                .map_or(String::new(), |m| m.draft.repeat.to_string()),
            Field::Delay(i) => self
                .features
                .macro_draft
                .as_ref()
                .and_then(|m| m.draft.steps.get(i))
                .map_or(String::new(), |s| s.delay.to_string()),
        };
        self.features.edit = Some(TextEdit { field, text });
        cx.notify();
    }

    pub fn commit_edit(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.features.edit.take() else {
            return;
        };
        match e.field {
            Field::Search | Field::MacroText | Field::RuleExe => {}
            Field::Repeat => {
                let v = e.text.trim().parse::<u16>().unwrap_or(1).clamp(1, 999);
                self.edit_macro(|m| m.draft.repeat = v, cx);
            }
            Field::Delay(i) => {
                let v = e.text.trim().parse::<u32>().unwrap_or(0).min(u16::MAX as u32) as u16;
                self.edit_macro(
                    |m| {
                        if let Some(s) = m.draft.steps.get_mut(i) {
                            s.delay = v
                        }
                    },
                    cx,
                );
            }
        }
        cx.notify();
    }

    /// Keys while a text field is active. Returns true when the key was used.
    pub fn edit_key(&mut self, k: &Keystroke, cx: &mut Context<Self>) -> bool {
        let Some(e) = self.features.edit.as_mut() else {
            return false;
        };
        let numeric = matches!(e.field, Field::Repeat | Field::Delay(_));
        match k.key.as_str() {
            "enter" => {
                let field = e.field;
                if field == Field::MacroText {
                    self.add_macro_text(cx);
                } else if field == Field::RuleExe {
                    let text = self.tools.rule_text.clone();
                    self.add_rule(&text, cx);
                } else {
                    self.commit_edit(cx);
                }
            }
            "escape" => self.features.edit = None,
            "backspace" => {
                e.text.pop();
            }
            _ => {
                if k.modifiers.control || k.modifiers.alt || k.modifiers.platform {
                    return false;
                }
                let space = " ".to_string();
                let Some(ch) = k.key_char.as_ref().or((k.key == "space").then_some(&space)) else {
                    return false;
                };
                if numeric && !ch.chars().all(|c| c.is_ascii_digit()) {
                    return true;
                }
                e.text.push_str(ch);
            }
        }
        // Search and macro text are live; numbers commit on Enter or when another control is clicked.
        if let Some(e) = self.features.edit.as_ref() {
            match e.field {
                Field::Search => self.features.search = e.text.clone(),
                Field::MacroText => self.features.macro_text = e.text.clone(),
                Field::RuleExe => self.tools.rule_text = e.text.clone(),
                _ => {}
            }
        }
        cx.notify();
        true
    }

    // ---- per-key colours ---------------------------------------------------------------

    pub fn set_lighting_tab(&mut self, tab: LightingTab, cx: &mut Context<Self>) {
        self.features.lighting_tab = tab;
        self.selection.clear();
        if tab == LightingTab::PerKey && self.features.perkey.is_none() {
            let showing = self
                .device
                .as_ref()
                .map(|d| d.lighting)
                .filter(|l| l.effect == USER_PICTURE_EFFECT)
                .map_or(0, |l| l.option);
            self.features.perkey = Some(PerKey {
                picture: showing,
                colors: None,
                color: [0x3b, 0x9b, 0xff],
                dirty: false,
            });
            self.load_picture(showing, cx);
        }
        cx.notify();
    }

    pub fn picture_count(&self) -> u8 {
        self.device.as_ref().map_or(0, |d| picture_count(d.info.device_id))
    }

    pub fn load_picture(&mut self, picture: u8, cx: &mut Context<Self>) {
        if let Some(p) = self.features.perkey.as_mut() {
            p.picture = picture;
            p.colors = None;
            p.dirty = false;
        }
        self.run(
            cx,
            "Reading key colours",
            move |kb| kb.get_picture(picture),
            move |s, colors, _| {
                if let Some(p) = s.features.perkey.as_mut() {
                    if p.picture == picture {
                        p.colors = Some(colors);
                    }
                }
            },
        );
    }

    /// Paint the selected keys with the brush colour (brush mode clears the selection after).
    pub fn paint_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let Some(p) = self.features.perkey.as_mut() else {
            return;
        };
        let Some(colors) = p.colors.as_mut() else {
            return;
        };
        for &i in &self.selection {
            if let Some(c) = colors.get_mut(i) {
                *c = p.color;
            }
        }
        p.dirty = true;
        self.selection.clear();
    }

    pub fn fill_picture(&mut self, color: Option<Rgb>, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let keys: Vec<usize> = d.layout.keys.iter().map(|k| k.index).collect();
        if let Some(p) = self.features.perkey.as_mut() {
            let c = color.unwrap_or(p.color);
            if let Some(colors) = p.colors.as_mut() {
                for i in keys {
                    colors[i] = c;
                }
                p.dirty = true;
            }
        }
        cx.notify();
    }

    pub fn save_picture(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let Some(p) = self.features.perkey.as_mut() else {
            return;
        };
        let Some(colors) = p.colors.clone() else {
            return;
        };
        let picture = p.picture;
        let l = picture_lighting(d.lighting, picture);
        let profile = d.profile;
        self.light_gen += 1;
        self.run(
            cx,
            "Saving key colours",
            move |kb| {
                kb.set_picture(picture, &colors)?;
                kb.set_lighting(&l)
            },
            move |s, _, cx| {
                if let Some(p) = s.features.perkey.as_mut() {
                    p.dirty = false;
                }
                if let Some(d) = s.device.as_mut() {
                    if d.profile == profile {
                        d.lighting = l;
                        d.committed_lighting = l;
                    }
                }
                s.toast(
                    format!("Saved to picture {} and switched the lighting to it", picture + 1),
                    false,
                    cx,
                );
            },
        );
    }

    // ---- calibration -------------------------------------------------------------------

    pub fn set_tester_tab(&mut self, tab: TesterTab, cx: &mut Context<Self>) {
        let leaving = self.features.tester_tab;
        if leaving == tab {
            return;
        }
        self.features.tester_tab = tab;
        self.set_tester_tab_tools(leaving, cx);
        if leaving == TesterTab::Calibration {
            self.end_calibration(cx);
        }
        if tab == TesterTab::Switch {
            self.start_stream(cx);
        } else {
            self.stop_stream(cx);
        }
        cx.notify();
    }

    pub fn set_cal_step(&mut self, step: CalStep, cx: &mut Context<Self>) {
        self.features.cal.step = step;
        cx.notify();
    }

    pub fn start_calibration(&mut self, cx: &mut Context<Self>) {
        let cal = &mut self.features.cal;
        cal.peaks = vec![0; crate::protocol::MATRIX_KEYS];
        cal.gen += 1;
        cal.step = CalStep::Rest(REST_SECS);
        let gen = cal.gen;
        self.run(cx, "Starting calibration", |kb| kb.set_rest_calibration(true), |_, _, _| {});
        cx.spawn(async move |this, cx| {
            for left in (0..REST_SECS).rev() {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this
                    .update(cx, |s, cx| {
                        let ok = s.features.cal.gen == gen && matches!(s.features.cal.step, CalStep::Rest(_));
                        if ok && left > 0 {
                            s.features.cal.step = CalStep::Rest(left);
                            cx.notify();
                        }
                        ok
                    })
                    .unwrap_or(false);
                if !alive {
                    return;
                }
            }
            this.update(cx, |s, cx| {
                if s.features.cal.gen != gen {
                    return;
                }
                // Cancelling from here on must also leave bottom calibration.
                s.features.cal.step = CalStep::Starting;
                s.run(
                    cx,
                    "Starting calibration",
                    |kb| {
                        let started = kb
                            .set_rest_calibration(false)
                            .and_then(|_| kb.set_bottom_calibration(true));
                        Ok(started.map_err(|e| format!("{e:#}")))
                    },
                    move |s, started, cx| {
                        if s.features.cal.gen != gen || s.features.cal.step != CalStep::Starting {
                            return;
                        }
                        match started {
                            Ok(()) => {
                                s.features.cal.step = CalStep::Bottom;
                                s.poll_calibration(gen, cx);
                            }
                            Err(e) => {
                                s.toast(format!("Starting calibration failed: {e}"), true, cx);
                                s.end_calibration(cx);
                            }
                        }
                    },
                );
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn poll_calibration(&mut self, gen: u64, cx: &mut Context<Self>) {
        let polling = matches!(self.features.cal.step, CalStep::Bottom | CalStep::ConfirmFinish);
        if self.features.cal.gen != gen || !polling {
            return;
        }
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let result = d.client.submit(|kb| kb.get_raw_travel());
        cx.spawn(async move |this, cx| {
            let r = result.recv().await;
            cx.background_executor().timer(Duration::from_millis(40)).await;
            this.update(cx, |s, cx| {
                if s.features.cal.gen != gen {
                    return;
                }
                if let Ok(Ok(values)) = r {
                    let peaks = &mut s.features.cal.peaks;
                    if values.iter().zip(peaks.iter()).any(|(v, p)| v > p) {
                        for (p, v) in peaks.iter_mut().zip(values) {
                            *p = (*p).max(v);
                        }
                        cx.notify();
                    }
                }
                s.poll_calibration(gen, cx);
            })
            .ok();
        })
        .detach();
    }

    pub fn save_calibration(&mut self, cx: &mut Context<Self>) {
        self.features.cal.step = CalStep::Saving;
        let gen = self.features.cal.gen;
        self.run(
            cx,
            "Saving calibration",
            |kb| {
                // Calibration can move every key's measured travel; re-read what the pages show.
                let saved = kb
                    .set_bottom_calibration(false)
                    .and_then(|_| kb.get_key_settings());
                Ok(saved.map_err(|e| format!("{e:#}")))
            },
            move |s, saved, cx| match saved {
                // Finish even if the page was left meanwhile (end_calibration keeps Saving).
                Ok(keys) => {
                    s.features.cal.gen += 1;
                    if s.features.cal.step == CalStep::Saving {
                        s.features.cal.step = CalStep::Done;
                    }
                    if let Some(d) = s.device.as_mut() {
                        d.committed = keys.clone();
                        d.keys = keys;
                    }
                    s.toast("Calibration saved", false, cx);
                }
                Err(e) => {
                    s.toast(format!("Saving calibration failed: {e}"), true, cx);
                    if s.features.cal.step != CalStep::Saving {
                        return;
                    }
                    if s.features.cal.gen == gen {
                        // Still calibrating: let the user press on and save again.
                        s.features.cal.step = CalStep::Bottom;
                        s.poll_calibration(gen, cx);
                    } else {
                        // The page was left; try once more to take the keyboard out of calibration.
                        s.features.cal.step = CalStep::Bottom;
                        s.end_calibration(cx);
                    }
                }
            },
        );
        cx.notify();
    }

    /// Leave calibration mode on the keyboard if a run is in progress.
    pub fn end_calibration(&mut self, cx: &mut Context<Self>) {
        let step = self.features.cal.step;
        self.features.cal.gen += 1;
        match step {
            // The countdown checks gen, so bottom calibration never gets queued after this.
            CalStep::Rest(_) => {
                self.run(cx, "Cancelling calibration", |kb| kb.set_rest_calibration(false), |_, _, _| {})
            }
            // Starting: "rest off, bottom on" is already queued, so switch bottom off after it.
            CalStep::Starting | CalStep::Bottom | CalStep::ConfirmFinish => {
                self.run(cx, "Cancelling calibration", |kb| kb.set_bottom_calibration(false), |_, _, _| {})
            }
            _ => {}
        }
        if step != CalStep::Saving {
            self.features.cal.step = CalStep::Idle;
        }
    }

    pub fn calibration_active(&self) -> bool {
        self.page == Page::Tester
            && matches!(
                self.features.cal.step,
                CalStep::Rest(_) | CalStep::Starting | CalStep::Bottom | CalStep::ConfirmFinish
            )
    }

    // ---- backup and restore --------------------------------------------------------------

    fn progress_cell(&mut self, cx: &mut Context<Self>) -> ProgressCell {
        let cell: ProgressCell = Arc::new(Mutex::new((0, 1, "Starting".into())));
        self.features.backup.busy = Some(cell.clone());
        self.features.backup.error = None;
        // Redraw the progress bar while a backup job runs.
        let watched = cell.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let alive = this
                .update(cx, |s, cx| {
                    let same = s.features.backup.busy.as_ref().is_some_and(|c| Arc::ptr_eq(c, &watched));
                    if same {
                        cx.notify();
                    }
                    same
                })
                .unwrap_or(false);
            if !alive {
                break;
            }
        })
        .detach();
        cell
    }

    fn backup_meta(&self) -> Option<(String, u8)> {
        self.device
            .as_ref()
            .map(|d| (d.def.display_name.clone(), d.def.fn_layer_count().max(1)))
    }

    pub fn back_up(&mut self, cx: &mut Context<Self>) {
        let Some((name, fn_layers)) = self.backup_meta() else {
            return;
        };
        if self.transitioning || self.features.backup.busy.is_some() {
            return;
        }
        self.end_drag(cx);
        self.flush_keys(cx);
        self.flush_lighting(cx);
        let cell = self.progress_cell(cx);
        let report = cell.clone();
        self.transitioning = true;
        let result = self.device.as_ref().unwrap().client.submit(move |kb| {
            backup::read(kb, &name, fn_layers, &mut |d, t, l| {
                *report.lock().unwrap() = (d, t, format!("Reading · {l}"));
            })
        });
        let connection = self.connection_gen;
        cx.spawn(async move |this, cx| {
            let r = result.recv().await.unwrap_or_else(|_| Err(anyhow!("Keyboard connection closed")));
            let path = match &r {
                Ok(b) => {
                    let dir = std::env::var_os("USERPROFILE")
                        .map(|h| std::path::PathBuf::from(h).join("Documents"))
                        .unwrap_or_default();
                    let name = backup::file_name(b);
                    let rx = this.update(cx, |_, cx| cx.prompt_for_new_path(&dir, Some(&name))).ok();
                    match rx {
                        Some(rx) => rx.await.ok().and_then(|p| p.ok()).flatten(),
                        None => None,
                    }
                }
                Err(_) => None,
            };
            this.update(cx, |s, cx| {
                if s.connection_gen != connection {
                    return;
                }
                s.transitioning = false;
                s.features.backup.busy = None;
                match (r, path) {
                    (Ok(b), Some(path)) => match std::fs::write(&path, backup::to_json(&b)) {
                        Ok(()) => s.toast("Backup saved", false, cx),
                        Err(e) => s.features.backup.error = Some(format!("Saving the file failed: {e}")),
                    },
                    (Ok(_), None) => {}
                    (Err(e), _) => s.features.backup.error = Some(format!("Backup failed: {e:#}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn open_backup(&mut self, cx: &mut Context<Self>) {
        if self.transitioning || self.features.backup.busy.is_some() {
            return;
        }
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Some(path) = rx.await.ok().and_then(|r| r.ok()).flatten().and_then(|p| p.into_iter().next()) else {
                return;
            };
            let parsed = std::fs::read_to_string(&path)
                .map_err(|e| anyhow!("Can't read the file: {e}"))
                .and_then(|t| backup::parse(&t));
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            this.update(cx, |s, cx| {
                s.features.backup.current = None;
                match parsed {
                    Ok(b) => {
                        let same = s.device.as_ref().is_some_and(|d| d.info.device_id == b.device.id);
                        s.features.backup.file = Some((b, name));
                        s.features.backup.error = None;
                        if same {
                            s.compare_backup(cx);
                        }
                    }
                    Err(e) => {
                        s.features.backup.file = None;
                        s.features.backup.error = Some(format!("{e:#}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Read the keyboard so the restore only writes real differences.
    fn compare_backup(&mut self, cx: &mut Context<Self>) {
        let Some((name, fn_layers)) = self.backup_meta() else {
            return;
        };
        self.end_drag(cx);
        self.flush_keys(cx);
        self.flush_lighting(cx);
        let cell = self.progress_cell(cx);
        self.transitioning = true;
        let connection = self.connection_gen;
        let result = self.device.as_ref().unwrap().client.submit(move |kb| {
            backup::read(kb, &name, fn_layers, &mut |d, t, l| {
                *cell.lock().unwrap() = (d, t, format!("Comparing with keyboard · {l}"));
            })
        });
        cx.spawn(async move |this, cx| {
            let r = result.recv().await.unwrap_or_else(|_| Err(anyhow!("Keyboard connection closed")));
            this.update(cx, |s, cx| {
                if s.connection_gen != connection {
                    return;
                }
                s.transitioning = false;
                s.features.backup.busy = None;
                match r {
                    Ok(b) => s.features.backup.current = Some(b),
                    Err(e) => s.features.backup.error = Some(format!("Reading the keyboard failed: {e:#}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn restore_backup(&mut self, cx: &mut Context<Self>) {
        let (Some((target, _)), true) = (self.features.backup.file.clone(), self.features.backup.current.is_some()) else {
            return;
        };
        let Some((name, fn_layers)) = self.backup_meta() else {
            return;
        };
        if self.transitioning || self.features.backup.busy.is_some() {
            return;
        }
        // Pending edits reach the keyboard first, so the fresh read below includes them.
        self.end_drag(cx);
        self.flush_keys(cx);
        self.flush_lighting(cx);
        let cell = self.progress_cell(cx);
        self.transitioning = true;
        self.key_gen += 1;
        self.light_gen += 1;
        let connection = self.connection_gen;
        let result = self.device.as_ref().unwrap().client.submit(move |kb| {
            // The comparison shown was read when the file was opened; plan against the keyboard as it is now.
            let restored = backup::restore_fresh(kb, &name, fn_layers, &target, &mut |d, t, l| {
                *cell.lock().unwrap() = (d, t, l.to_string());
            });
            // Re-read even after a failure, so the pages show what the keyboard really holds.
            let fresh = super::reload(kb);
            let rate = if kb.info.supports_report_rate { kb.get_report_rate().ok() } else { None };
            Ok((restored.map_err(|e| format!("{e:#}")), fresh, rate))
        });
        cx.spawn(async move |this, cx| {
            let r = result.recv().await.unwrap_or_else(|_| Err(anyhow!("Keyboard connection closed")));
            this.update(cx, |s, cx| {
                if s.connection_gen != connection {
                    return;
                }
                s.transitioning = false;
                s.features.backup.busy = None;
                match r {
                    Ok((restored, fresh, rate)) => {
                        if let (Some(d), Ok(fresh)) = (s.device.as_mut(), fresh) {
                            fresh.apply(d);
                            d.report_rate = rate;
                            d.macros.clear();
                        }
                        s.features.macro_draft = None;
                        s.features.perkey = None;
                        s.features.adv = None;
                        s.selection.clear();
                        match restored {
                            Ok(()) => {
                                s.features.backup.file = None;
                                s.features.backup.current = None;
                                s.toast("Backup restored", false, cx);
                            }
                            Err(e) => {
                                s.features.backup.error = Some(format!(
                                    "Restore failed: {e}. The keyboard may be partly restored; try again."
                                ))
                            }
                        }
                    }
                    Err(e) => s.features.backup.error = Some(format!("Restore failed: {e:#}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

/// Key settings for every key in `indices` (used by the switch-type picker).
#[allow(dead_code)]
pub fn common_switch(keys: &[KeyHe], indices: &[usize]) -> Option<u8> {
    let first = keys.get(*indices.first()?)?.switch_type;
    indices.iter().all(|&i| keys[i].switch_type == first).then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_key_names_map_to_hid() {
        assert_eq!(key_to_hid("a"), Some(0x04));
        assert_eq!(key_to_hid("0"), Some(0x27));
        assert_eq!(key_to_hid("f13"), Some(0x68));
        assert_eq!(key_to_hid("enter"), Some(0x28));
        assert_eq!(key_to_hid(";"), Some(0x33));
        assert_eq!(key_to_hid("nope"), None);
        assert_eq!(tap(4, 20, &[0xe0]).len(), 4);
    }
}
