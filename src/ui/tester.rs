//! Tester: switch tester with Rapid Trigger prediction, tuner and travel graph; polling-rate test.

use super::*;
use crate::app::{TunerScope, HISTORY};
use crate::polling::{BINS, BIN_US};
use crate::rt_model::{next_threshold, RtEvent};

const MAX_TRAVEL: f32 = 4.0;
const GRAPH_H: f32 = 180.0;

impl AppView {
    pub(super) fn tester(&mut self, cx: &mut Cx) -> Div {
        let tabs = div().flex().child(self.segmented(
            "tester-tab",
            vec![
                (TesterTab::Switch, "Switch tester".into()),
                (TesterTab::Polling, "Polling rate".into()),
                (TesterTab::Calibration, "Calibration".into()),
            ],
            Some(self.features.tester_tab),
            |this, v, _, cx| this.set_tester_tab(v, cx),
            cx,
        ));
        let body = match self.features.tester_tab {
            TesterTab::Switch => self.switch_tester(cx),
            TesterTab::Polling => self.polling_tester(cx),
            TesterTab::Calibration => self.calibration(cx),
        };
        div().flex().flex_col().gap(px(16.)).child(tabs).child(body)
    }

    fn switch_tester(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let travel = self.travel.clone();
        let keys = d.keys.clone();
        let names = d.defaults.clone();
        let focus = self.tester_focus();
        let pinned = self.selection.len() == 1;
        let config = focus.and_then(|i| self.tester_config(i));
        let predicted = self.tools.rt_state;
        let next = config.map(|c| next_threshold(&predicted, &c));
        let value = focus.map_or(0.0, |i| travel[i]);
        let settings = focus.map(|i| keys[i]);
        let focus_name = focus.map_or("—".to_string(), |i| action_label(&names[i]));
        let peak = focus.map_or(0.0, |i| self.peaks[i]);

        let board = self.keyboard(
            Select::Single,
            |k| {
                let v = travel[k.index];
                let s = keys[k.index];
                let down = if Some(k.index) == focus && config.is_some() {
                    predicted.on
                } else {
                    v > 0.01 && v >= s.actuation
                };
                let col = if down { t.rt } else { t.accent };
                Cap {
                    legend: action_label(&k.base),
                    // Always present (blank at rest) so the cap content never jumps.
                    value: Some(if v > 0.01 { format!("{v:.2}") } else { " ".into() }),
                    fill: (v > 0.01).then(|| (v / MAX_TRAVEL, col)),
                    lit: down.then_some(t.rt),
                    ..Default::default()
                }
            },
            cx,
        );
        let toolbar = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().text_color(t.muted).child("Press keys on your keyboard · click one to pin and tune it"));

        // Travel bar: fill from the top, actuation line, next predicted threshold, dead zone.
        let bar_h = 200.0;
        let bar = div()
            .relative()
            .flex_none()
            .w(px(26.))
            .h(px(bar_h))
            .rounded(px(6.))
            .bg(t.surface_2)
            .border_1()
            .border_color(t.border)
            .overflow_hidden()
            .child(div().absolute().top_0().left_0().w_full().h(relative((value / MAX_TRAVEL).clamp(0.0, 1.0))).bg(t.rt.opacity(0.45)))
            .children(settings.filter(|s| s.deadzone > 0.0).map(|s| {
                div().absolute().bottom_0().left_0().w_full().h(relative(s.deadzone / MAX_TRAVEL)).bg(t.muted.opacity(0.25))
            }))
            .children(settings.map(|s| div().absolute().left_0().w_full().h(px(2.)).top(px(bar_h * s.actuation / MAX_TRAVEL)).bg(t.actuation)))
            .children(next.filter(|_| predicted.in_rt).map(|(kind, at)| {
                div()
                    .absolute()
                    .left_0()
                    .w_full()
                    .h(px(2.))
                    .top(px(bar_h * (at / MAX_TRAVEL).clamp(0.0, 1.0)))
                    .bg(if kind == RtEvent::Press { t.rt } else { t.warn })
            }));
        let stat = |label: &'static str, v: String, color: Hsla| {
            div()
                .flex()
                .gap(px(6.))
                .text_size(px(12.5))
                .child(div().text_color(t.muted).child(label))
                .child(div().font_family(NUM_FONT).text_color(color).child(v))
        };
        let badge_on = config.is_some() && predicted.on;
        let readout = div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().text_color(t.muted).text_size(px(12.)).child("Travel"))
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(6.))
                    .child(div().w(px(110.)).font_family(NUM_FONT).text_size(px(34.)).child(format!("{value:.2}")))
                    .child(div().text_color(t.muted).pb(px(7.)).child("mm")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .w(px(40.))
                            .flex()
                            .justify_center()
                            .py(px(2.))
                            .rounded(px(5.))
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .bg(if badge_on { t.rt } else { t.surface_3 })
                            .text_color(if badge_on { t.on_accent } else { t.muted })
                            .child(if config.is_none() { "—" } else if predicted.on { "ON" } else { "OFF" }),
                    )
                    .child(div().flex_1().text_color(t.muted).text_size(px(12.5)).child(match (config, next) {
                        (Some(_), Some((kind, at))) => format!(
                            "predicted · {} {at:.2} mm",
                            if kind == RtEvent::Press { "presses at" } else { "releases at" }
                        ),
                        _ => "predicted · idle".into(),
                    })),
            )
            .child(stat("Peak", focus.map_or("—".into(), |_| format!("{peak:.2} mm")), t.text))
            .child(stat(
                "Rapid Trigger",
                settings.map_or("—".into(), |s| {
                    if s.rapid_trigger {
                        format!("{:.2} / {:.2} mm", s.rt_press, s.rt_release)
                    } else {
                        "off".into()
                    }
                }),
                t.rt,
            ))
            .child(div().child(
                self.small_button("peaks", "Reset peaks", Btn::Actuation, true)
                    .on_click(cx.listener(|this, _, _, cx| this.reset_peaks(cx))),
            ));

        let tune = if let (true, Some(i)) = (pinned, focus) {
            self.rt_tuner(i, &focus_name, cx)
        } else {
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_color(t.muted)
                        .text_size(px(12.5))
                        .child("Pin a key to tune its actuation and Rapid Trigger here."),
                )
                .child(
                    self.small_button("pin", focus.map_or("Pin key".into(), |_| format!("Pin {focus_name}")), Btn::Primary, focus.is_some())
                        .flex_none()
                        .min_w(px(96.))
                        .when_some(focus, |b, i| {
                            b.on_click(cx.listener(move |this, _, _, cx| {
                                this.selection = vec![i];
                                cx.notify();
                            }))
                        }),
                )
        };
        let key_card = self.card(
            format!("Key · {focus_name}"),
            Some(if pinned {
                "Pinned. Click the key again to follow the deepest pressed key."
            } else {
                "Following the deepest pressed key. Click a key to pin and tune it."
            }),
            None,
            div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(div().flex().gap(px(24.)).child(bar).child(readout))
                .child(div().pt(px(12.)).border_t_1().border_color(t.border).child(tune)),
        );

        let presses = self.tools.log.iter().filter(|e| e.event == RtEvent::Press).count();
        let log = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).text_size(px(13.)).child(if self.tools.log.is_empty() {
                        "Predicted events".to_string()
                    } else {
                        format!("Predicted events · {presses} {}", if presses == 1 { "press" } else { "presses" })
                    }))
                    .child(
                        self.small_button("log-clear", "Clear", Btn::Ghost, !self.tools.log.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tools.log.clear();
                                cx.notify();
                            })),
                    ),
            )
            .when(self.tools.log.is_empty(), |el| {
                el.child(div().text_color(t.muted).text_size(px(12.5)).child("Press the focused key to see where it would fire."))
            })
            .children(self.tools.log.iter().map(|e| {
                let press = e.event == RtEvent::Press;
                div()
                    .flex()
                    .gap(px(16.))
                    .text_size(px(12.5))
                    .child(div().w(px(70.)).font_family(NUM_FONT).text_color(t.muted).child(format!("{:.2} s", e.at)))
                    .child(div().w(px(80.)).text_color(if press { t.rt } else { t.warn }).child(if press { "↓ press" } else { "↑ release" }))
                    .child(div().font_family(NUM_FONT).child(format!("{:.2} mm", e.depth)))
            }));
        let graph_card = self.card(
            "Travel graph",
            Some("Last few seconds of the focused key. Purple: actuation point. Dots: predicted presses (teal) and releases (amber). Short dashed line: where the next one happens."),
            None,
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(self.travel_graph(settings.map(|s| s.actuation), next.filter(|_| predicted.in_rt)))
                .child(div().text_color(t.muted).text_size(px(12.)).child(if self.demo {
                    "Travel streams from the keyboard as the key moves, so the prediction follows every movement. Demo mode simulates key presses."
                } else {
                    "Travel streams from the keyboard as the key moves, so the prediction follows every movement."
                }))
                .child(log),
        );

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(self.keyboard_card(Some(toolbar.into_any_element()), board))
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .items_start()
                    .when(self.narrow(), |d| d.flex_col())
                    .child(div().w(px(420.)).flex_none().when(self.narrow(), |d| d.w_full()).child(key_card))
                    .child(div().flex_1().min_w(px(0.)).when(self.narrow(), |d| d.w_full()).child(graph_card)),
            )
    }

    /// Compact Performance controls for the key pinned in the switch tester.
    fn rt_tuner(&mut self, index: usize, name: &str, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let k = d.keys[index];
        let (min, max) = d.def.travel();
        let step = if d.info.steps_per_mm >= 200.0 { 0.005 } else { 0.01 };
        let count = self.edit_targets().len();
        let rt_toggle = self
            .toggle("tune-rt", k.rapid_trigger, false, |this, v, _, cx| this.update_keys(move |k| k.rapid_trigger = v, cx), cx)
            .when(k.rapid_trigger, |el| el.bg(t.rt).border_color(t.rt));
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(10.))
                    .child(div().text_color(t.muted).text_size(px(12.5)).child("Apply to"))
                    .child(self.segmented(
                        "tune-scope",
                        vec![
                            (TunerScope::Key, name.to_string()),
                            (TunerScope::Same, "Same settings".into()),
                            (TunerScope::All, "All keys".into()),
                        ],
                        Some(self.tools.scope),
                        |this, v, _, cx| {
                            this.tools.scope = v;
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(div().text_color(t.muted).text_size(px(12.)).child(format!("{count} {}", if count == 1 { "key" } else { "keys" }))),
            )
            .child(self.slider("Actuation", Some(k.actuation), SliderDrag { id: SliderId::Actuation, min, max, step }, "mm", false, cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).text_size(px(13.)).child("Rapid Trigger"))
                    .child(rt_toggle),
            )
            .when(k.rapid_trigger, |el| {
                el.child(self.slider("Press sensitivity", Some(k.rt_press), SliderDrag { id: SliderId::RtPress, min: 0.01, max: 2.0, step: 0.01 }, "mm", false, cx))
                    .child(self.slider("Release sensitivity", Some(k.rt_release), SliderDrag { id: SliderId::RtRelease, min: 0.01, max: 2.0, step: 0.01 }, "mm", false, cx))
            })
            .child(self.slider("Bottom dead zone", Some(k.deadzone), SliderDrag { id: SliderId::Deadzone, min: 0.0, max: 1.0, step: 0.01 }, "mm", false, cx))
    }

    fn travel_graph(&self, actuation: Option<f32>, next: Option<(RtEvent, f32)>) -> Div {
        let t = self.theme;
        let samples: Vec<f32> = self.tools.history.iter().copied().collect();
        let marks: Vec<Option<RtEvent>> = self.tools.marks.iter().copied().collect();
        let labels = [1.0f32, 2.0, 3.0].map(|mm| {
            div()
                .absolute()
                .left(px(6.))
                .top(px(GRAPH_H * mm / MAX_TRAVEL - 16.))
                .text_color(t.muted)
                .text_size(px(11.))
                .font_family(NUM_FONT)
                .child(format!("{mm:.0} mm"))
        });
        div()
            .relative()
            .w_full()
            .h(px(GRAPH_H))
            .rounded(px(8.))
            .bg(t.surface_2)
            .overflow_hidden()
            .children(labels)
            .child(
                canvas(
                    |_, _, _| {},
                    move |b, _, window, _| {
                        let w = f32::from(b.size.width);
                        let x0 = f32::from(b.origin.x);
                        let y0 = f32::from(b.origin.y);
                        let x = |i: usize| x0 + i as f32 / (HISTORY - 1) as f32 * w;
                        let y = |mm: f32| y0 + (mm / MAX_TRAVEL).clamp(0.0, 1.0) * GRAPH_H;
                        let hline = |window: &mut Window, from: f32, to: f32, at: f32, color: Hsla, thick: f32| {
                            window.paint_quad(fill(Bounds::new(point(px(from), px(at - thick / 2.0)), size(px(to - from), px(thick))), color));
                        };
                        let dashed = |window: &mut Window, from: f32, to: f32, at: f32, color: Hsla, dash: f32, thick: f32| {
                            let mut s = from;
                            while s < to {
                                hline(window, s, (s + dash).min(to), at, color, thick);
                                s += dash * 1.8;
                            }
                        };
                        for mm in [1.0, 2.0, 3.0] {
                            hline(window, x0, x0 + w, y(mm), t.border, 1.0);
                        }
                        if let Some(a) = actuation {
                            dashed(window, x0, x0 + w, y(a), t.actuation, 5.0, 1.5);
                        }
                        if let Some((kind, at)) = next {
                            let c = if kind == RtEvent::Press { t.rt } else { t.warn };
                            dashed(window, x0 + w * 0.75, x0 + w, y(at), c, 3.0, 2.0);
                        }
                        let offset = HISTORY - samples.len();
                        if samples.len() > 1 {
                            let mut path = PathBuilder::stroke(px(2.5));
                            for (i, &v) in samples.iter().enumerate() {
                                let p = point(px(x(i + offset)), px(y(v)));
                                if i == 0 {
                                    path.move_to(p);
                                } else {
                                    path.line_to(p);
                                }
                            }
                            if let Ok(path) = path.build() {
                                window.paint_path(path, t.rt);
                            }
                        }
                        for (i, m) in marks.iter().enumerate() {
                            let (Some(m), Some(&v)) = (m, samples.get(i)) else { continue };
                            let c = if *m == RtEvent::Press { t.rt } else { t.warn };
                            let center = point(px(x(i + offset) - 5.0), px(y(v) - 5.0));
                            window.paint_quad(
                                fill(Bounds::new(center, size(px(10.), px(10.))), c)
                                    .corner_radii(px(5.))
                                    .border_widths(px(2.))
                                    .border_color(t.surface_2),
                            );
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
    }

    fn polling_tester(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let configured = d.report_rate;
        let est = self.tools.estimate();
        let running = self.tools.polling();
        let fmt = |hz: u32| if hz >= 1000 { format!("{}K", hz / 1000) } else { hz.to_string() };
        let matches = est.hz.zip(configured).map(|(a, b)| a == b);
        let line = |label: &'static str, v: String| {
            div()
                .flex()
                .gap(px(6.))
                .text_size(px(12.5))
                .child(div().text_color(t.muted).child(label))
                .child(div().child(v))
        };
        let controls = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                self.button("poll-run", if running { "Stop" } else { "Start test" }, if running { Btn::Normal } else { Btn::Primary })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if running {
                            this.stop_polling_test(cx)
                        } else {
                            this.start_polling_test(cx)
                        }
                    })),
            )
            .child(
                self.button("poll-reset", "Reset", Btn::Ghost)
                    .when(self.tools.poll_times.is_empty(), |b| b.opacity(0.45))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tools.poll_times.clear();
                        cx.notify();
                    })),
            )
            .when(running, |el| {
                el.child(div().flex().items_center().gap(px(6.)).text_color(t.muted).text_size(px(12.5)).child(
                    div().size(px(8.)).rounded_full().bg(if self.tools.poll_ready { t.ok } else { t.muted }),
                ).child(if self.tools.poll_ready { "Listening — mash keys now" } else { "Starting…" }))
            });
        let left = self.card(
            "Polling rate test",
            Some("Press Start, then slam a handful of keys at once with your palm or fingers, 15–20 times. Keys that change together get spread across consecutive USB reports, and the gaps between them reveal the polling interval. Measured with Windows Raw Input from this keyboard only."),
            None,
            div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(controls)
                .children(self.tools.poll_error.clone().map(|e| self.banner(e, t.danger)))
                .when(self.demo, |el| {
                    el.child(self.banner("Demo mode has no keyboard to measure, so the test listens to every keyboard.", t.muted))
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(div().text_color(t.muted).text_size(px(12.)).child("Detected polling rate"))
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .gap(px(6.))
                                .child(div().font_family(NUM_FONT).text_size(px(40.)).child(est.hz.map_or("—".into(), fmt)))
                                .child(div().text_color(t.muted).pb(px(9.)).child("Hz")),
                        )
                        .child(line(
                            "Measured interval:",
                            est.measured_hz.map_or("—".into(), |m| format!("{:.0} µs (≈{m} Hz)", 1e6 / m as f64)),
                        ))
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .text_size(px(12.5))
                                .child(div().text_color(t.muted).child("Keyboard setting:"))
                                .child(configured.map_or("fixed by firmware".into(), |hz| format!("{hz} Hz")))
                                .children(matches.map(|m| {
                                    div().text_color(if m { t.ok } else { t.warn }).child(if m { "· matches" } else { "· differs" })
                                })),
                        )
                        .child(line(
                            "Key changes:",
                            format!("{} · USB reports: {} · timing samples: {}", est.events, est.reports, est.evidence),
                        ))
                        .when(est.hz.is_none() && est.events > 0, |el| {
                            el.child(div().pt(px(6.)).text_color(t.muted).text_size(px(12.5)).child(
                                "Not enough simultaneous presses yet. Keep slamming several keys at once.",
                            ))
                        }),
                ),
        );
        let max = est.histogram.iter().copied().max().unwrap_or(0).max(1) as f32;
        let bars = div()
            .h(px(200.))
            .flex()
            .items_end()
            .gap(px(2.))
            .children(est.histogram.iter().map(|&n| {
                div()
                    .flex_1()
                    .h(relative((n as f32 / max).max(0.02) * 0.92))
                    .rounded_t(px(3.))
                    .bg(if n > 0 { t.accent } else { t.surface_3 })
            }));
        let axis = div()
            .flex()
            .justify_between()
            .text_color(t.muted)
            .text_size(px(11.))
            .font_family(NUM_FONT)
            .children((0..=4).map(|i| div().child(format!("{:.1} ms", i as f64 * BIN_US * BINS as f64 / 4.0 / 1000.0))));
        let right = self.card(
            "Gap between reports",
            Some("Gaps cluster at multiples of the polling interval: 125 µs steps at 8K, 250 µs at 4K, 1 ms at 1K."),
            None,
            div().flex().flex_col().gap(px(8.)).child(bars).child(axis),
        );
        div()
            .flex()
            .gap(px(16.))
            .items_start()
            .when(self.narrow(), |d| d.flex_col())
            .child(div().flex_1().min_w(px(0.)).when(self.narrow(), |d| d.w_full()).child(left))
            .child(div().flex_1().min_w(px(0.)).when(self.narrow(), |d| d.w_full()).child(right))
    }
}
