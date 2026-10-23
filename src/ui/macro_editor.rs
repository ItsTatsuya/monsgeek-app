//! Macro editor in the Remap page: record, type text, add taps, edit delays.

use super::*;
use crate::keycodes::{char_to_hid, macro_key_label};
use crate::protocol::macros::{self, play, MACRO_BYTES};

const MODES: [(u8, &str, &str); 3] = [
    (
        play::REPEAT,
        "Play on press",
        "Plays the macro the set number of times each time you press the key.",
    ),
    (
        play::WHILE_HELD,
        "Repeat while held",
        "Loops the macro for as long as the key is held down.",
    ),
    (
        play::TOGGLE,
        "Toggle",
        "Press once to start looping, press again to stop.",
    ),
];

impl AppView {
    pub(super) fn macro_editor(
        &mut self,
        index: usize,
        _current: KeyAction,
        key_label: String,
        cx: &mut Cx,
    ) -> Div {
        self.ensure_macro_draft(index, cx);
        let t = self.theme;
        let saving = self.busy > 0;
        let Some(m) = self.features.macro_draft.as_ref() else {
            return div();
        };
        let draft = m.draft.clone();
        let (play_mode, recording, real, dirty, bound, loading, unreadable) = (
            m.mode,
            m.recording,
            m.real_timing,
            m.dirty,
            m.bound,
            m.loading,
            m.unreadable,
        );
        let mods = m.mods.clone();
        let size = macros::size(&draft);
        let too_big = size > MACRO_BYTES;
        let stuck = macros::held_at_end(&draft.steps);
        let duration: u32 = draft.steps.iter().map(|s| s.delay as u32).sum();
        let hint = MODES.iter().find(|m| m.0 == play_mode).map_or("", |m| m.2);
        let skipped: Vec<char> = {
            let mut v: Vec<char> = self
                .features
                .macro_text
                .chars()
                .filter(|&c| char_to_hid(c).is_none())
                .collect();
            v.dedup();
            v
        };

        let top = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(12.))
            .child(self.segmented(
                "macro-mode",
                MODES.iter().map(|m| (m.0, m.1.to_string())).collect(),
                Some(play_mode),
                |this, v, _, cx| this.edit_macro(|m| m.mode = v, cx),
                cx,
            ))
            .when(play_mode == play::REPEAT, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_color(t.muted)
                        .child("Times")
                        .child(self.text_field(Field::Repeat, &draft.repeat.to_string(), "1", 64., cx)),
                )
            });

        let record_row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(
                self.button(
                    "macro-record",
                    if recording { "■ Stop recording" } else { "● Record" },
                    if recording { Btn::Danger } else { Btn::Primary },
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_recording(cx))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_color(t.muted)
                    .text_size(px(12.5))
                    .child(self.toggle(
                        "macro-timing",
                        real,
                        recording,
                        |this, v, _, cx| {
                            if let Some(m) = this.features.macro_draft.as_mut() {
                                m.real_timing = v;
                            }
                            cx.notify();
                        },
                        cx,
                    ))
                    .child("Keep my timing"),
            )
            .child(div().flex_1())
            .child(
                self.small_button("macro-clear", "Clear", Btn::Ghost, !draft.steps.is_empty() && !recording)
                    .when(!draft.steps.is_empty() && !recording, |b| {
                        b.on_click(cx.listener(|this, _, _, cx| this.edit_macro(|m| m.draft.steps.clear(), cx)))
                    }),
            );

        let steps = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .max_h(px(360.))
            .p(px(8.))
            .rounded(px(10.))
            .bg(t.surface_2)
            .border_1()
            .border_color(t.border)
            .when(loading, |el| el.child(div().text_color(t.muted).child("Reading macro from the keyboard…")))
            .when(unreadable && draft.steps.is_empty(), |el| {
                el.child(div().text_color(t.warn).child(
                    "This key plays a macro saved by another app in a slot this firmware can't read back. Build a new one here to replace it.",
                ))
            })
            .when(!loading && draft.steps.is_empty(), |el| {
                el.child(div().p(px(6.)).text_color(t.muted).child(
                    "No steps yet. Record, type some text, or click keys below to add taps.",
                ))
            })
            .children(draft.steps.iter().enumerate().map(|(i, s)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(6.))
                    .py(px(2.))
                    .child(div().w(px(24.)).text_color(t.muted).font_family(NUM_FONT).text_size(px(12.)).child((i + 1).to_string()))
                    .child(
                        div()
                            .w(px(14.))
                            .text_color(if s.down { t.accent_strong } else { t.rt })
                            .child(if s.down { "↓" } else { "↑" }),
                    )
                    .child(self.kbd(macro_key_label(s.key)))
                    .child(div().text_color(t.muted).text_size(px(12.)).child(if s.down { "press" } else { "release" }))
                    .child(div().flex_1())
                    .child(div().text_color(t.muted).text_size(px(12.)).child("then wait"))
                    .child(self.text_field(Field::Delay(i), &s.delay.to_string(), "0", 72., cx))
                    .child(div().text_color(t.muted).text_size(px(12.)).child("ms"))
                    .child(
                        self.small_button(("macro-del", i), "✕", Btn::Ghost, true).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.features.edit = None;
                                this.edit_macro(
                                    |m| {
                                        if i < m.draft.steps.len() {
                                            m.draft.steps.remove(i);
                                        }
                                    },
                                    cx,
                                )
                            },
                        )),
                    )
            }));
        let steps = div().id("macro-steps").overflow_y_scroll().child(steps);

        let can_save = !draft.steps.is_empty() && !too_big && !recording && !saving && (dirty || bound.is_none());
        let summary = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(if too_big { t.danger } else { t.muted })
                    .child(format!(
                        "{} steps · {:.2} s · {size} / {MACRO_BYTES} bytes",
                        draft.steps.len(),
                        duration as f32 / 1000.0
                    )),
            )
            .child(div().flex_1())
            .child(
                self.button("macro-revert", "Revert", Btn::Normal)
                    .when(!dirty || recording, |b| b.opacity(0.45))
                    .when(dirty && !recording, |b| {
                        b.on_click(cx.listener(|this, _, _, cx| this.revert_macro(cx)))
                    }),
            )
            .child(
                self.button(
                    "macro-save",
                    if bound.is_some() { "Save macro".to_string() } else { format!("Assign macro to {key_label}") },
                    Btn::Primary,
                )
                .when(!can_save, |b| b.opacity(0.45))
                .when(can_save, |b| b.on_click(cx.listener(|this, _, _, cx| this.save_macro(cx)))),
            );

        let text_row = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(self.section_label("Type text"))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(self.text_field(Field::MacroText, &self.features.macro_text, "e.g. gg wp", 320., cx))
                    .child(
                        self.button("macro-add-text", "Add as keystrokes", Btn::Normal)
                            .when(self.features.macro_text.is_empty(), |b| b.opacity(0.45))
                            .when(!self.features.macro_text.is_empty(), |b| {
                                b.on_click(cx.listener(|this, _, _, cx| this.add_macro_text(cx)))
                            }),
                    ),
            )
            .when(!skipped.is_empty(), |el| {
                el.child(div().text_color(t.muted).text_size(px(12.)).child(format!(
                    "Will skip characters a US layout can't type directly: {}",
                    skipped.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")
                )))
            });

        // Plain keys from the remap palette (no modifiers baked in, no media or firmware functions).
        let taps: Vec<PaletteGroup> = palette()
            .into_iter()
            .map(|g| PaletteGroup {
                name: g.name,
                keys: g
                    .keys
                    .into_iter()
                    .filter(|(_, a)| a[0] == 0 && a[1] == 0 && a[3] == 0 && a[2] != 0)
                    .collect(),
            })
            .filter(|g| !g.keys.is_empty())
            .collect();
        let held: Vec<KeyAction> = mods.iter().map(|&m| [0, 0, m, 0]).collect();
        let mods_text = if mods.is_empty() {
            "Click a key to add a press and release. For a shortcut, first pick modifiers to hold.".to_string()
        } else {
            let mut parts: Vec<String> = mods.iter().map(|&m| macro_key_label(m)).collect();
            parts.push("…".into());
            format!("Holding {} around the next key.", parts.join(" + "))
        };
        let tap_section = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(self.section_label("Add a key tap"))
            .child(div().text_color(t.muted).text_size(px(12.5)).child(mods_text))
            .child(
                div()
                    .when(recording, |el| el.opacity(0.45))
                    .child(self.palette_grid(
                        "macro-pal",
                        taps,
                        None,
                        &held,
                        |this, a, cx| {
                            if !this.recording() {
                                this.add_macro_tap(a[2], cx)
                            }
                        },
                        cx,
                    )),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(top)
            .child(div().text_color(t.muted).text_size(px(12.5)).child(hint))
            .child(record_row)
            .when(recording, |el| {
                el.child(self.banner(
                    "Recording — type on your keyboard. Keys you press are added to the macro, not sent to the app. Click Stop when you're done.",
                    t.rt,
                ))
            })
            .child(steps)
            .child(summary)
            .when(too_big, |el| {
                el.child(self.banner(
                    "Too long for the keyboard: remove some steps (each press or release takes 2–4 bytes).",
                    t.danger,
                ))
            })
            .when(!stuck.is_empty(), |el| {
                let names: Vec<String> = stuck.iter().map(|&k| macro_key_label(k)).collect();
                el.child(self.banner(
                    format!(
                        "{} {} pressed but never released, so {} will stay held after the macro runs.",
                        names.join(", "),
                        if stuck.len() == 1 { "is" } else { "are" },
                        if stuck.len() == 1 { "it" } else { "they" }
                    ),
                    t.warn,
                ))
            })
            .child(text_row)
            .child(tap_section)
    }

    pub(super) fn section_label(&self, text: &'static str) -> Div {
        div()
            .text_size(px(11.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(self.theme.muted)
            .child(text.to_uppercase())
    }
}
