//! Advanced keys: Snap Tap, toggle keys, dynamic keystroke (DKS) and mod-tap.

use super::*;
use crate::app::AdvEditor;
use crate::protocol::advanced::{
    cycle_cell, segments, DKS_PHASES, DKS_TRAVEL_RANGE, MT_TIME_RANGE,
};
use crate::protocol::mode;

fn mode_info(m: u8) -> Option<(&'static str, &'static str)> {
    Some(match m {
        mode::DKS => ("DKS", "Dynamic keystroke"),
        mode::MOD_TAP => ("MT", "Mod-tap"),
        mode::TOGGLE_HOLD => ("TGL", "Toggle (hold)"),
        mode::TOGGLE_TAP => ("TGL", "Toggle (tap)"),
        mode::SNAP_TAP => ("SNAP", "Snap Tap"),
        _ => return None,
    })
}

fn is_empty(a: &KeyAction) -> bool {
    *a == [0, 0, 0, 0]
}

impl AppView {
    pub(super) fn pill(&self, text: impl Into<SharedString>) -> Div {
        let t = self.theme;
        div()
            .flex_none()
            .px(px(6.))
            .py(px(1.))
            .rounded(px(4.))
            .bg(t.accent_soft)
            .text_color(t.accent_strong)
            .text_size(px(10.5))
            .font_weight(FontWeight(650.))
            .child(text.into())
    }

    pub(super) fn kbd(&self, text: impl Into<SharedString>) -> Div {
        let t = self.theme;
        div()
            .flex_none()
            .px(px(7.))
            .py(px(2.))
            .rounded(px(5.))
            .bg(t.surface_2)
            .border_1()
            .border_color(t.border_strong)
            .font_family(NUM_FONT)
            .text_size(px(12.))
            .child(text.into())
    }

    pub(super) fn advanced(&mut self, cx: &mut Cx) -> Div {
        self.sync_adv_editor(cx);
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let keys = d.keys.clone();
        let names = d.defaults.clone();
        let sel = self.selection.clone();
        let single = (sel.len() == 1).then(|| sel[0]);

        let board = self.keyboard(
            Select::Multi,
            |k| Cap {
                legend: action_label(&k.base),
                tag: mode_info(keys[k.index].mode).map(|(tag, _)| tag),
                ..Default::default()
            },
            cx,
        );
        let count = sel.len();
        let toolbar = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().flex_1().text_color(t.muted).child(if count == 0 {
                "Click or drag across keys to select them".to_string()
            } else {
                format!("{count} {} selected", if count == 1 { "key" } else { "keys" })
            }))
            .child(self.button("adv-clear-sel", "Clear", Btn::Normal).on_click(cx.listener(
                |this, _, _, cx| {
                    this.selection.clear();
                    cx.notify();
                },
            )));

        type Action = fn(&mut AppView, &mut Cx);
        let cards: [(&str, &str, &str, bool, Action); 6] = [
            (
                "SNAP",
                "Snap Tap",
                "Select exactly two keys (e.g. A and D). Pressing one releases the other, so the newest direction always wins.",
                count == 2,
                |this, cx| this.make_snap(cx),
            ),
            (
                "TGL",
                "Toggle on tap",
                "Tap once to hold the key down, tap again to release it. Handy for auto-run or crouch.",
                count > 0,
                |this, cx| this.set_toggle(mode::TOGGLE_TAP, cx),
            ),
            (
                "TGL",
                "Toggle on hold",
                "The firmware's hold-style toggle: taps latch the key, a long hold acts like a normal press.",
                count > 0,
                |this, cx| this.set_toggle(mode::TOGGLE_HOLD, cx),
            ),
            (
                "DKS",
                "Dynamic keystroke",
                "Select one key. Up to four actions fire at different points of one press, e.g. walk on a light press, sprint at the bottom.",
                single.is_some(),
                |this, cx| {
                    if let [i] = this.selection[..] {
                        this.open_dks(i, cx)
                    }
                },
            ),
            (
                "MT",
                "Mod-tap",
                "Select one key. A quick tap sends one key, holding it sends another (e.g. tap for Esc, hold for Ctrl).",
                single.is_some(),
                |this, cx| {
                    if let [i] = this.selection[..] {
                        this.open_mod_tap(i, cx)
                    }
                },
            ),
            (
                "",
                "Reset to normal",
                "Remove any advanced behaviour from the selected keys.",
                count > 0,
                |this, cx| {
                    let sel = this.selection.clone();
                    this.clear_advanced(sel, cx)
                },
            ),
        ];
        let columns = if self.narrow() { 2 } else { 3 };
        let mut tiles: Vec<_> = cards
            .into_iter()
            .enumerate()
            .map(|(i, (tag, title, desc, enabled, act))| {
                div()
                    .id(("mode-card", i))
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .p(px(14.))
                    .rounded(px(10.))
                    .bg(t.surface)
                    .border_1()
                    .border_color(t.border)
                    .when(!enabled, |el| el.opacity(0.45))
                    .when(enabled, |el| {
                        el.cursor_pointer()
                            .hover(|s| s.border_color(t.accent))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                act(this, cx);
                                cx.notify();
                            }))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .when(!tag.is_empty(), |el| el.child(self.pill(tag)))
                            .child(title),
                    )
                    .child(div().text_color(t.muted).text_size(px(12.5)).child(desc))
            })
            .collect();
        let mut mode_cards = div().flex().flex_col().gap(px(12.));
        while !tiles.is_empty() {
            let rest = tiles.split_off(columns.min(tiles.len()));
            mode_cards = mode_cards.child(div().flex().gap(px(12.)).children(tiles));
            tiles = rest;
        }

        let editor = self.adv_editor_card(&names, cx);

        // Active advanced keys, Snap Tap pairs listed once.
        let d = self.device.as_ref().unwrap();
        let active: Vec<usize> = d
            .layout
            .keys
            .iter()
            .map(|k| k.index)
            .filter(|&i| keys[i].mode != mode::NORMAL)
            .collect();
        let rows = active
            .iter()
            .filter(|&&i| keys[i].mode != mode::SNAP_TAP || i < keys[i].snap_partner as usize)
            .map(|&i| {
                let m = keys[i].mode;
                let (tag, name) = mode_info(m).unwrap_or(("?", "Unknown mode"));
                let editable = mode_info(m).is_some();
                let mut row = div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .bg(t.surface_2)
                    .child(self.pill(tag))
                    .child(self.kbd(action_label(&names[i])));
                if m == mode::SNAP_TAP {
                    let partner = keys[i].snap_partner as usize;
                    row = row
                        .child(div().text_color(t.muted).child("⇄"))
                        .child(self.kbd(action_label(names.get(partner).unwrap_or(&[0; 4]))));
                } else {
                    row = row.child(div().text_color(t.muted).child(name)).when(!editable, |el| {
                        el.child(
                            div()
                                .text_color(t.muted)
                                .text_size(px(12.))
                                .child(format!("mode {m} · set by another app")),
                        )
                    });
                }
                row.child(div().flex_1())
                    .when(m == mode::DKS || m == mode::MOD_TAP, |el| {
                        el.child(self.small_button(("adv-edit", i), "Edit", Btn::Normal, true).on_click(
                            cx.listener(move |this, _, _, cx| {
                                if m == mode::DKS {
                                    this.open_dks(i, cx)
                                } else {
                                    this.open_mod_tap(i, cx)
                                }
                            }),
                        ))
                    })
                    .child(self.small_button(("adv-remove", i), "Remove", Btn::Ghost, true).on_click(
                        cx.listener(move |this, _, _, cx| this.clear_advanced(vec![i], cx)),
                    ))
            })
            .collect::<Vec<_>>();
        let list = self.card(
            "Active advanced keys",
            active.is_empty().then_some("No keys use an advanced mode in this profile."),
            None,
            div().flex().flex_col().gap(px(6.)).children(rows),
        );

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(self.keyboard_card(Some(toolbar.into_any_element()), board))
            .child(mode_cards)
            .children(editor)
            .child(list)
    }

    fn adv_editor_card(&mut self, names: &[KeyAction], cx: &mut Cx) -> Option<Div> {
        let t = self.theme;
        let editor = self.features.adv.as_ref()?;
        let index = editor.index();
        let key = action_label(&names[index]);
        let mode_now = self.device.as_ref()?.keys[index].mode;
        let close = self.button("adv-close", "Close", Btn::Normal).on_click(cx.listener(|this, _, _, cx| {
            this.features.adv = None;
            this.selection.clear();
            cx.notify();
        }));
        let (title, body) = match editor {
            AdvEditor::Dks { cfg: None, .. } | AdvEditor::ModTap { cfg: None, .. } => (
                "Reading from the keyboard…".to_string(),
                div().text_color(t.muted).child("Reading the key's settings from the keyboard…"),
            ),
            AdvEditor::Dks {
                cfg: Some(cfg), row, ..
            } => {
                let (cfg, row) = (cfg.clone(), *row);
                let unused = (0..4).any(|r| !is_empty(&cfg.actions[r]) && segments(cfg.cells[r]).is_empty());
                let depth = [
                    format!("{:.2} mm ↓", cfg.travel),
                    "bottom".into(),
                    "bottom ↑".into(),
                    format!("{:.2} mm ↑", cfg.travel),
                ];
                let head = div()
                    .flex()
                    .gap(px(8.))
                    .child(div().w(px(120.)).flex_none())
                    .children(DKS_PHASES.iter().enumerate().map(|(i, p)| {
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .gap(px(4.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(12.5))
                                    .child(div().text_color(if i < 2 { t.accent_strong } else { t.rt }).child(if i < 2 { "↓" } else { "↑" }))
                                    .child(*p),
                            )
                            .child(div().text_color(t.muted).text_size(px(11.5)).child(depth[i].clone()))
                    }));
                let rows: Vec<_> = (0..4).map(|r| {
                    let action = cfg.actions[r];
                    let cells = cfg.cells[r];
                    let selected = r == row;
                    let segs = segments(cells);
                    let track = div()
                        .flex_1()
                        .relative()
                        .h(px(36.))
                        .rounded(px(8.))
                        .bg(t.surface_2)
                        .border_1()
                        .border_color(t.border)
                        .child(div().absolute().left(px(8.)).right(px(8.)).top(px(17.)).h(px(1.)).bg(t.border_strong))
                        .children(segs.iter().map(|s| {
                            let x = |p: usize| relative(0.125 + p as f32 * 0.25);
                            match s.to {
                                None => div()
                                    .absolute()
                                    .left(x(s.from))
                                    .ml(px(-7.))
                                    .top(px(10.))
                                    .size(px(14.))
                                    .rounded_full()
                                    .bg(t.accent),
                                Some(to) => div()
                                    .absolute()
                                    .left(x(s.from))
                                    .w(relative((to - s.from) as f32 * 0.25))
                                    .ml(px(-7.))
                                    .top(px(10.))
                                    .h(px(14.))
                                    .rounded_full()
                                    .bg(t.rt),
                            }
                        }))
                        .child(div().absolute().inset_0().flex().children((0..4).map(|p| {
                            div()
                                .id(("dks-cell", r * 4 + p))
                                .flex_1()
                                .h_full()
                                .cursor_pointer()
                                .hover(|s| s.bg(t.hover.opacity(0.4)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(AdvEditor::Dks { cfg: Some(c), .. }) = this.features.adv.as_mut() {
                                        c.cells[r] = cycle_cell(c.cells[r], p);
                                    }
                                    cx.notify();
                                }))
                        })));
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .id(("dks-slot", r))
                                .w(px(120.))
                                .flex_none()
                                .h(px(36.))
                                .px(px(10.))
                                .flex()
                                .items_center()
                                .rounded(px(8.))
                                .border_1()
                                .border_color(if selected { t.accent } else { t.border })
                                .bg(if selected { t.accent_soft } else { t.surface_2 })
                                .cursor_pointer()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(AdvEditor::Dks { row, .. }) = this.features.adv.as_mut() {
                                        *row = r;
                                    }
                                    cx.notify();
                                }))
                                .child(if is_empty(&action) {
                                    div().text_color(t.muted).child("+ key")
                                } else {
                                    div().child(action_label(&action))
                                }),
                        )
                        .child(track)
                }).collect();
                let save_label = if mode_now == mode::DKS {
                    "Save DKS".to_string()
                } else {
                    format!("Make {key} a DKS key")
                };
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(self.slider(
                        "Trigger point",
                        Some(cfg.travel),
                        SliderDrag {
                            id: SliderId::DksTravel,
                            min: DKS_TRAVEL_RANGE.0,
                            max: DKS_TRAVEL_RANGE.1,
                            step: 0.01,
                        },
                        "mm",
                        false,
                        cx,
                    ))
                    .child(div().flex().flex_col().gap(px(8.)).child(head).children(rows))
                    .child(div().text_color(t.muted).text_size(px(12.5)).child(
                        "Click a phase to add a tap there; click again to hold it until the next phase, again to hold longer, and once more to remove it.",
                    ))
                    .when(unused, |el| el.child(self.banner("A slot has a key but no phases, so it will never fire.", t.warn)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().flex_1().text_color(t.muted).text_size(px(12.5)).child(format!("Picking a key for slot {}", row + 1)))
                            .child(close)
                            .child(self.button("adv-save", save_label, Btn::Primary).on_click(cx.listener(|this, _, _, cx| this.save_adv(cx)))),
                    )
                    .child(self.action_picker(Some(cfg.actions[row]), cx));
                (format!("Dynamic keystroke · {key}"), body)
            }
            AdvEditor::ModTap {
                cfg: Some(cfg), hold, ..
            } => {
                let (cfg, hold) = (*cfg, *hold);
                let slot = |which_hold: bool, title: &'static str, hint: String, a: KeyAction| {
                    let selected = which_hold == hold;
                    div()
                        .id(if which_hold { "mt-hold" } else { "mt-tap" })
                        .w(px(180.))
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .p(px(12.))
                        .rounded(px(10.))
                        .border_1()
                        .border_color(if selected { t.accent } else { t.border })
                        .bg(if selected { t.accent_soft } else { t.surface_2 })
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(AdvEditor::ModTap { hold, .. }) = this.features.adv.as_mut() {
                                *hold = which_hold;
                            }
                            cx.notify();
                        }))
                        .child(div().text_color(t.muted).text_size(px(12.)).child(title))
                        .child(
                            div()
                                .text_size(px(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(if is_empty(&a) { "+ key".to_string() } else { action_label(&a) }),
                        )
                        .child(div().text_color(t.muted).text_size(px(11.5)).child(hint))
                };
                let can_save = !is_empty(&cfg.tap) && !is_empty(&cfg.hold);
                let save_label = if mode_now == mode::MOD_TAP {
                    "Save mod-tap".to_string()
                } else {
                    format!("Make {key} a mod-tap key")
                };
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(12.))
                            .child(slot(false, "Tap", format!("released within {} ms", cfg.time), cfg.tap))
                            .child(slot(true, "Hold", format!("held longer than {} ms", cfg.time), cfg.hold)),
                    )
                    .child(self.slider(
                        "Hold time",
                        Some(cfg.time as f32),
                        SliderDrag {
                            id: SliderId::MtTime,
                            min: MT_TIME_RANGE.0,
                            max: MT_TIME_RANGE.1,
                            step: 10.0,
                        },
                        "ms",
                        false,
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().flex_1().text_color(t.muted).text_size(px(12.5)).child(format!(
                                "Picking the {} key",
                                if hold { "hold" } else { "tap" }
                            )))
                            .child(close)
                            .child(
                                self.button("adv-save", save_label, Btn::Primary)
                                    .when(!can_save, |b| b.opacity(0.45))
                                    .when(can_save, |b| b.on_click(cx.listener(|this, _, _, cx| this.save_adv(cx)))),
                            ),
                    )
                    .child(self.action_picker(Some(if hold { cfg.hold } else { cfg.tap }), cx));
                (format!("Mod-tap · {key}"), body)
            }
        };
        Some(self.card(title, Some("Saved to the active profile when you press Save."), None, body))
    }

    /// Compact key palette for DKS and mod-tap slots.
    fn action_picker(&self, current: Option<KeyAction>, cx: &mut Cx) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(self.text_field(Field::Search, &self.features.search, "Search keys…", 200., cx))
                    .child(
                        self.button("clear-slot", "Clear slot", Btn::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.pick_adv_action([0, 0, 0, 0], cx))),
                    ),
            )
            .child(self.palette_grid(
                "adv-pal",
                palette(),
                current,
                &[],
                |this, a, cx| this.pick_adv_action(a, cx),
                cx,
            ))
    }
}
