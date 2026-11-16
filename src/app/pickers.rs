//! Window-owned kit state: the colour pickers (effect colour, per-key brush) and the text
//! inputs (key search, macro text, app rules, profile names).

use super::AppView;
use gpui::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui::component::input::{InputEvent, InputState};
use gpui::{AppContext as _, Context, Entity, Hsla, Rgba, Window};

pub struct Pickers {
    /// Lighting › Effects › Colour.
    pub effect: Entity<ColorPickerState>,
    /// Lighting › Per-key colours › brush.
    pub brush: Entity<ColorPickerState>,
    /// Remap and Advanced › key search.
    pub search: Entity<InputState>,
    /// Macro editor › type text.
    pub macro_text: Entity<InputState>,
    /// Automatic profiles › app name.
    pub rule_exe: Entity<InputState>,
    /// Settings › Profiles › names.
    pub profile_names: [Entity<InputState>; 4],
}

pub fn to_hsla(c: [u8; 3]) -> Hsla {
    Rgba {
        r: c[0] as f32 / 255.0,
        g: c[1] as f32 / 255.0,
        b: c[2] as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

pub fn to_rgb(c: Hsla) -> [u8; 3] {
    let c = Rgba::from(c);
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [ch(c.r), ch(c.g), ch(c.b)]
}

impl AppView {
    /// Creates both pickers and forwards their changes. Called once, with the window.
    pub fn init_pickers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let effect = cx.new(|cx| ColorPickerState::new(window, cx));
        let brush = cx.new(|cx| ColorPickerState::new(window, cx));
        cx.subscribe(&effect, |this, _, e: &ColorPickerEvent, cx| {
            if let ColorPickerEvent::Change(Some(c)) = e {
                let c = to_rgb(*c);
                this.edit_lighting(|l| l.color = c, cx);
            }
        })
        .detach();
        cx.subscribe(&brush, |this, _, e: &ColorPickerEvent, cx| {
            if let ColorPickerEvent::Change(Some(c)) = e {
                if let Some(p) = this.features.perkey.as_mut() {
                    p.color = to_rgb(*c);
                }
                cx.notify();
            }
        })
        .detach();
        let input = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let search = input("Search keys…", window, cx);
        let macro_text = input("e.g. gg wp", window, cx);
        let rule_exe = input("App name, e.g. VALORANT-Win64-Shipping.exe", window, cx);
        let profile_names = [0, 1, 2, 3].map(|p| {
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(format!("Profile {}", p + 1)));
            // The name follows every edit (so a re-render never puts the old one back) and is
            // saved once typing pauses.
            cx.subscribe(&state, move |this, state, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    let name = state.read(cx).value().to_string();
                    this.rename_profile(p, &name, cx);
                }
            })
            .detach();
            state
        });
        cx.subscribe(&search, |this, state, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                this.features.search = state.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&macro_text, |this, state, e: &InputEvent, cx| match e {
            InputEvent::Change => {
                this.features.macro_text = state.read(cx).value().to_string();
                cx.notify();
            }
            InputEvent::PressEnter { .. } => this.add_macro_text(cx),
            _ => {}
        })
        .detach();
        cx.subscribe(&rule_exe, |this, state, e: &InputEvent, cx| match e {
            InputEvent::Change => {
                this.tools.rule_text = state.read(cx).value().to_string();
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                let text = this.tools.rule_text.clone();
                this.add_rule(&text, cx);
            }
            _ => {}
        })
        .detach();
        self.pickers = Some(Pickers { effect, brush, search, macro_text, rule_exe, profile_names });
    }

    /// Shows text changed by the app (a cleared field, a reconnect) in the inputs. Profile
    /// names are left alone while being edited.
    fn sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.pickers.as_ref() else {
            return;
        };
        let mut wanted: Vec<(Entity<InputState>, String, bool)> = vec![
            (p.search.clone(), self.features.search.clone(), false),
            (p.macro_text.clone(), self.features.macro_text.clone(), false),
            (p.rule_exe.clone(), self.tools.rule_text.clone(), false),
        ];
        let id = self.device.as_ref().map(|d| d.info.device_id);
        for (i, state) in p.profile_names.iter().enumerate() {
            let name = id
                .and_then(|id| self.config.profile_names.get(&id))
                .map(|n| n[i].clone())
                .unwrap_or_default();
            wanted.push((state.clone(), name, true));
        }
        for (state, text, skip_focused) in wanted {
            if skip_focused && gpui::Focusable::focus_handle(state.read(cx), cx).is_focused(window) {
                continue;
            }
            if state.read(cx).value().as_ref() != text {
                state.update(cx, |s, cx| s.set_value(text, window, cx));
            }
        }
    }

    /// Shows colours changed elsewhere (swatches, the keyboard, a profile switch) in the pickers.
    pub fn sync_pickers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_inputs(window, cx);
        let Some(p) = self.pickers.as_ref() else {
            return;
        };
        let wanted = [
            (&p.effect, self.device.as_ref().map(|d| d.lighting.color)),
            (&p.brush, self.features.perkey.as_ref().map(|k| k.color)),
        ];
        for (picker, color) in wanted {
            let Some(color) = color else { continue };
            // LEDs have no alpha: an 8-digit hex or the alpha slider snaps back to opaque.
            let value = picker.read(cx).value();
            if value.map(to_rgb) != Some(color) || value.is_some_and(|v| v.a < 0.999) {
                picker.update(cx, |s, cx| s.set_value(to_hsla(color), window, cx));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_round_trips_through_hsla() {
        for v in 0..=255u8 {
            for c in [[v, 255, 0], [255, 255, v], [v, v, v], [12, v, 200], [v, 0, 255 - v]] {
                assert_eq!(to_rgb(to_hsla(c)), c, "{c:?}");
            }
        }
    }
}
