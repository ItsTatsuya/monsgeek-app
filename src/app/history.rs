//! Undo and redo for edits the user makes on the pages: key settings (actuation, Rapid
//! Trigger, dead zones, Snap Tap…), single-key remaps and lighting.
//!
//! Each step holds the state to go back to. Undoing a step pushes the state it replaced onto
//! the redo stack (and the other way round), so the two stacks mirror each other. Steps are
//! tied to the profile they were made on; switching profile, reconnecting or any bulk change
//! (factory reset, restore, profile copy) clears both stacks.

use super::{AppView, OnFail};
use crate::protocol::{fn_system, mode, KeyAction, KeyHe, Lighting};
use gpui::Context;

/// How many steps each stack keeps.
const LIMIT: usize = 100;

#[derive(Clone)]
pub enum UndoStep {
    /// The whole key-settings table, as it was before an edit burst.
    Keys(Vec<KeyHe>),
    /// One key's action on the base layer (layer 0) or on Fn layer `fn_layer`.
    Remap {
        layer: u8,
        fn_layer: u8,
        index: usize,
        action: KeyAction,
    },
    Lighting(Lighting),
}

impl UndoStep {
    /// What the step changes, for the Undo/Redo button tooltips and notifications.
    pub fn label(&self) -> &'static str {
        match self {
            UndoStep::Keys(_) => "key settings",
            UndoStep::Remap { .. } => "remap",
            UndoStep::Lighting(_) => "lighting",
        }
    }
}

#[derive(Default)]
pub struct History {
    undo: Vec<(u8, UndoStep)>,
    redo: Vec<(u8, UndoStep)>,
    /// A key-settings edit burst is open: further edits join its step until the write goes out.
    keys_open: bool,
    /// Same for lighting.
    lighting_open: bool,
}

impl History {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn can_undo(&self) -> Option<&'static str> {
        self.undo.last().map(|(_, s)| s.label())
    }

    pub fn can_redo(&self) -> Option<&'static str> {
        self.redo.last().map(|(_, s)| s.label())
    }

    fn record(&mut self, profile: u8, step: UndoStep) {
        self.redo.clear();
        self.undo.push((profile, step));
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    /// Call before changing key settings. Only the first edit of a burst is recorded.
    pub fn before_keys(&mut self, profile: u8, keys: &[KeyHe]) {
        if !std::mem::replace(&mut self.keys_open, true) {
            self.record(profile, UndoStep::Keys(keys.to_vec()));
        }
    }

    /// The burst's write went out: the next key-settings edit starts a new step.
    pub fn close_keys(&mut self) {
        self.keys_open = false;
    }

    pub fn before_lighting(&mut self, profile: u8, lighting: Lighting) {
        if !std::mem::replace(&mut self.lighting_open, true) {
            self.record(profile, UndoStep::Lighting(lighting));
        }
    }

    pub fn close_lighting(&mut self) {
        self.lighting_open = false;
    }

    pub fn before_remap(&mut self, profile: u8, layer: u8, fn_layer: u8, index: usize, action: KeyAction) {
        self.record(profile, UndoStep::Remap { layer, fn_layer, index, action });
    }

    /// Advanced-key modes changed outside the history (DKS, mod-tap, calibration): older
    /// key-settings steps would put stale modes back, so drop them.
    pub fn forget_keys(&mut self) {
        self.undo.retain(|(_, s)| !matches!(s, UndoStep::Keys(_)));
        self.redo.retain(|(_, s)| !matches!(s, UndoStep::Keys(_)));
        self.keys_open = false;
    }
}

/// Puts `step` on the device and returns the step that brings the current state back.
fn swap(view: &mut AppView, step: UndoStep, cx: &mut Context<AppView>) -> Option<UndoStep> {
    let d = view.device.as_mut()?;
    match step {
        UndoStep::Keys(mut keys) => {
            if keys.len() != d.keys.len() {
                return None;
            }
            // DKS and mod-tap keep data in the key's sub-layers, which this step doesn't hold:
            // leave those modes as they are now.
            for (old, now) in keys.iter_mut().zip(&d.keys) {
                let layered = |m: u8| matches!(m, mode::DKS | mode::MOD_TAP);
                if layered(old.mode) || layered(now.mode) {
                    old.mode = now.mode;
                }
            }
            let back = std::mem::replace(&mut d.keys, keys);
            view.history.close_keys();
            view.flush_keys(cx);
            Some(UndoStep::Keys(back))
        }
        UndoStep::Lighting(l) => {
            let back = std::mem::replace(&mut d.lighting, l);
            view.history.close_lighting();
            view.flush_lighting(cx);
            Some(UndoStep::Lighting(back))
        }
        UndoStep::Remap { layer, fn_layer, index, action } => {
            let showing_layer = layer == 0 || d.fn_layer == fn_layer;
            let map = if layer == 0 { &mut d.keymap } else { &mut d.fnmap };
            if index >= map.len() {
                return None;
            }
            let current = if showing_layer { map[index] } else { action };
            if showing_layer {
                map[index] = action;
            }
            let (profile, system) = (d.profile, fn_system(d.options.os));
            view.run_or(
                cx,
                "Remapping key",
                OnFail::Notify,
                move |kb| {
                    if layer == 0 {
                        kb.set_key(profile, 0, index as u8, action)
                    } else {
                        kb.set_fn_key(fn_layer, index as u8, action, system)
                    }
                },
                |_, _, _| {},
                move |s, _| {
                    if let Some(d) = s.device.as_mut() {
                        let map = if layer == 0 { &mut d.keymap } else { &mut d.fnmap };
                        if showing_layer && index < map.len() {
                            map[index] = current;
                        }
                    }
                },
            );
            Some(UndoStep::Remap { layer, fn_layer, index, action: current })
        }
    }
}

impl AppView {
    pub fn undo(&mut self, cx: &mut Context<Self>) {
        self.step_history(true, cx);
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        self.step_history(false, cx);
    }

    fn step_history(&mut self, undo: bool, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        let Some(profile) = self.device.as_ref().map(|d| d.profile) else {
            return;
        };
        self.end_drag(cx);
        self.commit_edit(cx);
        let from = if undo { &mut self.history.undo } else { &mut self.history.redo };
        let Some((p, step)) = from.pop() else {
            return;
        };
        if p != profile {
            // Steps are cleared on profile switches; this only guards against a missed one.
            self.history.clear();
            cx.notify();
            return;
        }
        if let Some(back) = swap(self, step, cx) {
            let to = if undo { &mut self.history.redo } else { &mut self.history.undo };
            to.push((profile, back));
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(actuation: f32) -> KeyHe {
        KeyHe { actuation, ..Default::default() }
    }

    #[test]
    fn an_edit_burst_records_one_step() {
        let mut h = History::default();
        h.before_keys(0, &[key(1.0)]);
        h.before_lighting(0, Lighting::default());
        // Still the same burst: no new step lands on top of the lighting one.
        h.before_keys(0, &[key(1.1)]);
        assert_eq!(h.can_undo(), Some("lighting"));
        h.close_keys();
        h.before_keys(0, &[key(1.2)]);
        assert_eq!(h.can_undo(), Some("key settings"));
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut h = History::default();
        h.redo.push((0, UndoStep::Lighting(Lighting::default())));
        h.before_remap(0, 0, 0, 3, [0, 0, 4, 0]);
        assert!(h.can_redo().is_none());
        assert_eq!(h.can_undo(), Some("remap"));
    }

    #[test]
    fn forgetting_keys_keeps_other_steps() {
        let mut h = History::default();
        h.before_lighting(0, Lighting::default());
        h.before_keys(0, &[key(1.0)]);
        h.forget_keys();
        assert_eq!(h.can_undo(), Some("lighting"));
    }
}
