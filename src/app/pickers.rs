//! Colour pickers (GPUI Kit `ColorPicker`) for the effect colour and the per-key brush.

use super::AppView;
use gpui::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui::{AppContext as _, Context, Entity, Hsla, Rgba, Window};

pub struct Pickers {
    /// Lighting › Effects › Colour.
    pub effect: Entity<ColorPickerState>,
    /// Lighting › Per-key colours › brush.
    pub brush: Entity<ColorPickerState>,
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
        self.pickers = Some(Pickers { effect, brush });
    }

    /// Shows colours changed elsewhere (swatches, the keyboard, a profile switch) in the pickers.
    pub fn sync_pickers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
