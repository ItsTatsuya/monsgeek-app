//! Settings: backup and restore card, and the switch-type picker.

use super::*;
use crate::backup;

/// Switch type codes (MAG 252), English names for the common Akko/MonsGeek magnetic switches.
const SWITCH_TYPES: [(u8, &str); 10] = [
    (0, "Gateron (default)"),
    (1, "Akko Magnetic Jade"),
    (2, "Akko Magnetic Jade Pro"),
    (3, "Akko Magnetic Jade Gaming"),
    (4, "TTC King"),
    (5, "Magnetic King (Kailh)"),
    (6, "Kotai"),
    (8, "Magnetic White"),
    (10, "Magnetic King Pro"),
    (11, "TTC King SE"),
];

fn fmt_fw(fw: u16) -> String {
    format!("v{}.{:02x}", fw >> 8, fw & 0xff)
}

impl AppView {
    pub(super) fn switch_type_row(&self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let first = d.layout.keys.first().map(|k| d.keys[k.index].switch_type);
        let common = first.filter(|&f| d.layout.keys.iter().all(|k| d.keys[k.index].switch_type == f));
        let label = match common {
            None => "Mixed".to_string(),
            Some(c) => SWITCH_TYPES
                .iter()
                .find(|s| s.0 == c)
                .map_or(format!("Type {c}"), |s| s.1.to_string()),
        };
        let open = self.features.switch_menu;
        let menu = div()
            .absolute()
            .top(px(38.))
            .left_0()
            .w(px(240.))
            .p(px(4.))
            .rounded(px(10.))
            .bg(t.surface)
            .border_1()
            .border_color(t.border_strong)
            .shadow_lg()
            .children(SWITCH_TYPES.iter().map(|&(id, name)| {
                let active = common == Some(id);
                div()
                    .id(("switch", id as usize))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .when(active, |el| el.bg(t.accent_soft).text_color(t.accent_strong))
                    .hover(|s| s.bg(t.hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_switch_type(id, cx)))
                    .child(name)
            }));
        self.setting_row(
            "Switch type",
            "Tells the firmware which magnetic switch is installed so travel is measured correctly.",
            div()
                .relative()
                .child(
                    self.button("switch-type", format!("{label}  ▾"), Btn::Normal)
                        .min_w(px(200.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.features.switch_menu = !this.features.switch_menu;
                            cx.notify();
                        })),
                )
                .when(open, |el| el.child(deferred(menu).with_priority(1))),
        )
        .flex_col()
        .items_start()
    }

    pub(super) fn backup_card(&self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let b = &self.features.backup;
        let busy = b.busy.as_ref().map(|c| c.lock().map(|g| g.clone()).unwrap_or_default());
        let idle = busy.is_none() && !self.transitioning;

        let buttons = div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .child(
                self.button("backup", "Back up keyboard", Btn::Primary)
                    .when(!idle, |b| b.opacity(0.45))
                    .when(idle, |b| b.on_click(cx.listener(|this, _, _, cx| this.back_up(cx)))),
            )
            .child(
                self.button("restore-open", "Restore from file…", Btn::Normal)
                    .when(!idle, |b| b.opacity(0.45))
                    .when(idle, |b| b.on_click(cx.listener(|this, _, _, cx| this.open_backup(cx)))),
            );
        let progress = busy.map(|(done, total, label)| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .h(px(6.))
                        .rounded_full()
                        .bg(t.track)
                        .child(div().h_full().rounded_full().bg(t.accent).w(relative(done as f32 / total.max(1) as f32))),
                )
                .child(div().text_color(t.muted).text_size(px(12.)).child(label))
        });

        let file = b.file.as_ref().map(|(file, name)| {
            let mismatch = file.device.id != d.info.device_id;
            let plan = b.current.as_ref().filter(|_| !mismatch).map(|c| backup::plan(c, file));
            let created = file.created.replace('T', " ");
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(14.))
                .rounded(px(10.))
                .bg(t.surface_2)
                .border_1()
                .border_color(t.border)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).overflow_hidden().text_ellipsis().child(name.clone()))
                        .child(
                            self.small_button("backup-close", "✕", Btn::Ghost, idle).when(idle, |b| {
                                b.on_click(cx.listener(|this, _, _, cx| {
                                    this.features.backup.file = None;
                                    this.features.backup.current = None;
                                    cx.notify();
                                }))
                            }),
                        ),
                )
                .child(div().text_color(t.text_2).text_size(px(12.5)).child(format!(
                    "{} · firmware {} · {}",
                    file.device.name,
                    fmt_fw(file.device.firmware),
                    created.get(..16).unwrap_or(&created)
                )))
                .child(div().text_color(t.muted).text_size(px(12.5)).child(format!(
                    "4 profiles · {} Fn {} · {} macros · lighting · options{}",
                    file.fn_layers.len(),
                    if file.fn_layers.len() == 1 { "layer" } else { "layers" },
                    file.macros.len(),
                    file.report_rate.map_or(String::new(), |r| format!(" · {r} Hz"))
                )))
                .when(mismatch, |el| {
                    el.child(self.banner(
                        format!(
                            "This backup is from a different keyboard (device ID {}, this one is {}). Its layout and settings won't match, so it can't be restored here.",
                            file.device.id, d.info.device_id
                        ),
                        t.danger,
                    ))
                })
                .when(!mismatch && file.device.firmware != d.info.firmware, |el| {
                    el.child(self.banner(
                        format!(
                            "Made on firmware {}; this keyboard runs {}. Settings should carry over, but check them after restoring.",
                            fmt_fw(file.device.firmware),
                            fmt_fw(d.info.firmware)
                        ),
                        t.warn,
                    ))
                })
                .children(plan.map(|plan| {
                    if plan.is_empty() {
                        div().text_color(t.muted).child("The keyboard already matches this backup. Nothing to restore.")
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Restoring will change:"))
                            .children(plan.lines().into_iter().map(|l| {
                                div()
                                    .flex()
                                    .gap(px(8.))
                                    .text_size(px(13.))
                                    .child(div().text_color(t.muted).child("•"))
                                    .child(l)
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .pt(px(4.))
                                    .child(
                                        self.button("restore", "Restore to keyboard", Btn::Danger)
                                            .when(!idle, |b| b.opacity(0.45))
                                            .when(idle, |b| b.on_click(cx.listener(|this, _, _, cx| this.restore_backup(cx)))),
                                    )
                                    .child(div().text_color(t.muted).text_size(px(12.5)).child("Keep the keyboard plugged in until it finishes.")),
                            )
                    }
                }))
        });

        self.card(
            "Backup & restore",
            Some("Saves all four profiles, keymaps, Fn layers, actuation and Rapid Trigger, macros, lighting and options to a file."),
            None,
            div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(buttons)
                .children(progress)
                .children(b.error.clone().map(|e| self.banner(e, t.danger)))
                .children(file),
        )
    }
}
