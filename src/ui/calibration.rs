//! Travel calibration: measure every key at rest, then record each key's bottom.

use super::*;
use crate::app::CAL_DONE_AT;

impl AppView {
    pub(super) fn calibration(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let steps_per_mm = d.info.steps_per_mm;
        let step = self.features.cal.step;
        let peaks = self.features.cal.peaks.clone();
        let names: Vec<(usize, String)> = d
            .layout
            .keys
            .iter()
            .map(|k| (k.index, action_label(&k.base)))
            .collect();
        let peak = |i: usize| peaks.get(i).copied().unwrap_or(0);
        let finished = names.iter().filter(|(i, _)| peak(*i) >= CAL_DONE_AT).count();
        let shallow: Vec<&String> = names
            .iter()
            .filter(|(i, _)| peak(*i) > 0 && peak(*i) < CAL_DONE_AT)
            .map(|(_, n)| n)
            .collect();
        let untouched: Vec<&String> = names.iter().filter(|(i, _)| peak(*i) == 0).map(|(_, n)| n).collect();
        let total = names.len();
        let missing = total - finished;
        let show = matches!(
            step,
            CalStep::Bottom | CalStep::ConfirmFinish | CalStep::Saving | CalStep::Done
        );

        let board = self.keyboard(
            Select::None,
            |k| {
                let p = peak(k.index);
                if !show || p == 0 {
                    return Cap {
                        legend: action_label(&k.base),
                        value: Some(" ".into()),
                        ..Default::default()
                    };
                }
                let ok = p >= CAL_DONE_AT;
                let col = if ok { t.ok } else { t.warn };
                Cap {
                    legend: action_label(&k.base),
                    value: Some(format!("{:.2}", p as f32 / steps_per_mm)),
                    fill: Some(((p as f32 / CAL_DONE_AT as f32).min(1.0), col)),
                    lit: ok.then_some(t.ok),
                    ..Default::default()
                }
            },
            cx,
        );
        let hint = match step {
            CalStep::Rest(_) | CalStep::Starting => "Measuring the rest position. Don't touch the keyboard.",
            CalStep::Bottom | CalStep::ConfirmFinish => {
                "Numbers show the deepest point each key reached. Green: fully bottomed out. Amber: not deep enough yet, press it again firmly."
            }
            _ => "Each key shows how deep it went once calibration starts.",
        };
        let list = |keys: &[&String]| {
            let mut s: Vec<String> = keys.iter().take(16).map(|k| k.to_string()).collect();
            if keys.len() > 16 {
                s.push(format!("and {} more", keys.len() - 16));
            }
            s.join(", ")
        };
        let bullet = |text: String| {
            div()
                .flex()
                .gap(px(8.))
                .child(div().text_color(t.muted).child("•"))
                .child(div().flex_1().text_size(px(13.)).child(text))
        };
        let warning = |title: String, items: Vec<String>| {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(14.))
                .rounded(px(10.))
                .border_1()
                .border_color(t.warn.opacity(0.5))
                .bg(t.warn.opacity(0.06))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                .children(items.into_iter().map(bullet))
        };

        let body = match step {
            CalStep::Idle => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .children(
                    [
                        "Keep the keyboard plugged in with the cable, and take your hands off it.",
                        "Monsgeek App measures the rest position of every key (2 seconds).",
                        "Press each key slowly all the way down and hold it for about a second, until it turns green.",
                        "Click Save calibration.",
                    ]
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        div()
                            .flex()
                            .gap(px(8.))
                            .text_color(t.text_2)
                            .text_size(px(13.))
                            .child(div().w(px(16.)).text_color(t.muted).child(format!("{}.", i + 1)))
                            .child(*s)
                    }),
                )
                .child(div().flex().child(
                    self.button("cal-start", "Start calibration…", Btn::Primary)
                        .on_click(cx.listener(|this, _, _, cx| this.set_cal_step(CalStep::Confirm, cx))),
                )),
            CalStep::Confirm => warning(
                "Before you start".into(),
                vec![
                    "This replaces the keyboard's current calibration for every key. It can't be undone, but you can calibrate again.".into(),
                    "Keep your hands off the keyboard for the first 2 seconds. Touching keys then makes their rest position wrong.".into(),
                    "Press every key until it physically bottoms out. A key that isn't pressed fully gets a shallow bottom, which can make it register early.".into(),
                    "Don't unplug the keyboard or close Monsgeek App until you've saved.".into(),
                ],
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(self.button("cal-back", "Back", Btn::Normal).on_click(cx.listener(|this, _, _, cx| this.set_cal_step(CalStep::Idle, cx))))
                    .child(self.button("cal-go", "I understand, start", Btn::Primary).on_click(cx.listener(|this, _, _, cx| this.start_calibration(cx)))),
            ),
            CalStep::Rest(_) | CalStep::Starting => div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(match step {
                    CalStep::Rest(left) => format!("Hands off the keyboard… {left}"),
                    _ => "Starting bottom calibration…".to_string(),
                }))
                .child(div().flex_1())
                .child(self.button("cal-cancel", "Cancel", Btn::Ghost).on_click(cx.listener(|this, _, _, cx| {
                    this.end_calibration(cx);
                    cx.notify();
                }))),
            CalStep::Bottom | CalStep::Saving => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .gap(px(6.))
                                .child(div().font_family(NUM_FONT).text_size(px(22.)).child(finished.to_string()))
                                .child(div().text_color(t.muted).pb(px(3.)).child(format!("/ {total} keys"))),
                        )
                        .child(
                            div()
                                .flex_1()
                                .h(px(8.))
                                .rounded_full()
                                .bg(t.track)
                                .child(div().h_full().rounded_full().bg(t.ok).w(relative(finished as f32 / total.max(1) as f32))),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().flex_1().text_color(t.muted).text_size(px(12.5)).child(if missing == 0 {
                            "All keys reached full depth.".to_string()
                        } else if !shallow.is_empty() {
                            format!("{} not pressed deep enough, {} not pressed yet.", shallow.len(), untouched.len())
                        } else {
                            format!("{} {} not pressed yet.", untouched.len(), if untouched.len() == 1 { "key" } else { "keys" })
                        }))
                        .child(
                            self.button("cal-save", if step == CalStep::Saving { "Saving…" } else { "Save calibration" }, Btn::Primary)
                                .when(step == CalStep::Saving, |b| b.opacity(0.45))
                                .when(step == CalStep::Bottom, |b| {
                                    b.on_click(cx.listener(move |this, _, _, cx| {
                                        if missing > 0 {
                                            this.set_cal_step(CalStep::ConfirmFinish, cx)
                                        } else {
                                            this.save_calibration(cx)
                                        }
                                    }))
                                }),
                        ),
                ),
            CalStep::ConfirmFinish => {
                let mut items = Vec::new();
                if !shallow.is_empty() {
                    items.push(format!(
                        "Not pressed deep enough. Saving now may give them a shallow bottom, so they can register early: {}",
                        list(&shallow)
                    ));
                }
                if !untouched.is_empty() {
                    items.push(format!("Not pressed at all. These may keep an incorrect bottom position: {}", list(&untouched)));
                }
                items.push("You can keep pressing them now: they still count while this message is open.".into());
                warning(
                    format!("{missing} {} reached full depth", if missing == 1 { "key hasn't" } else { "keys haven't" }),
                    items,
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(self.button("cal-keep", "Keep calibrating", Btn::Primary).on_click(cx.listener(|this, _, _, cx| this.set_cal_step(CalStep::Bottom, cx))))
                        .child(self.button("cal-anyway", "Save anyway", Btn::Normal).on_click(cx.listener(|this, _, _, cx| this.save_calibration(cx)))),
                )
            }
            CalStep::Done => div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if missing > 0 { t.warn } else { t.ok })
                        .child(format!(
                            "{} Calibration saved · {finished} of {total} keys at full depth",
                            if missing > 0 { "⚠" } else { "✓" }
                        )),
                )
                .child(div().flex_1())
                .child(self.button("cal-again", "Calibrate again", Btn::Normal).on_click(cx.listener(|this, _, _, cx| this.set_cal_step(CalStep::Idle, cx)))),
        };

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(div().text_color(t.muted).child(hint))
            .child(self.keyboard_card(None, board))
            .child(self.card(
                "Travel calibration",
                Some("Re-measures where each switch rests and where it bottoms out. Use it after changing switches, or if actuation feels inconsistent between keys."),
                None,
                body,
            ))
    }
}
