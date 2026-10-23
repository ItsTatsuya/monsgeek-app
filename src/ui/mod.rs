//! Rendering: shell, widgets and the pages. Larger page parts live in the child modules.

mod advanced;
mod auto_profiles;
mod backup_card;
mod calibration;
mod layer_card;
mod macro_editor;
mod perkey;
mod tester;

use crate::app::{
    AppView, CalStep, Field, LightingTab, Page, RemapTab, SliderDrag, SliderId, Status, TesterTab,
};
use crate::devices::VisualKey;
use crate::keycodes::{action_description, action_label, palette, PaletteGroup};
use crate::protocol::{KeyAction, KeyHe};
use crate::theme::{NUM_FONT, UI_FONT};
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

type Cx<'a> = Context<'a, AppView>;

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Normal,
    Primary,
    /// Tinted in the actuation colour.
    Actuation,
    Ghost,
    Danger,
}

/// One keycap's look, computed by each page.
#[derive(Default)]
struct Cap {
    legend: String,
    value: Option<String>,
    tag: Option<&'static str>,
    /// Fill from the bottom (0..1) in a colour, for travel and progress.
    fill: Option<(f32, Hsla)>,
    /// Backlight shining through the whole cap: colour at a strength (0..1).
    glow: Option<(f32, Hsla)>,
    /// Coloured legend/value (e.g. past the actuation point).
    lit: Option<Hsla>,
    dim: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Select {
    Multi,
    Single,
    None,
}

struct Effect {
    id: u8,
    name: &'static str,
    speed: bool,
    color: bool,
}

const EFFECTS: &[Effect] = &[
    Effect {
        id: 0,
        name: "Off",
        speed: false,
        color: false,
    },
    Effect {
        id: 1,
        name: "Static",
        speed: false,
        color: true,
    },
    Effect {
        id: 2,
        name: "Breathing",
        speed: true,
        color: true,
    },
    Effect {
        id: 18,
        name: "Meteor",
        speed: true,
        color: true,
    },
    Effect {
        id: 5,
        name: "Ripple",
        speed: true,
        color: true,
    },
    Effect {
        id: 23,
        name: "Train",
        speed: true,
        color: true,
    },
    Effect {
        id: 17,
        name: "Rain down",
        speed: true,
        color: true,
    },
    Effect {
        id: 9,
        name: "Converge",
        speed: true,
        color: true,
    },
    Effect {
        id: 7,
        name: "Snake",
        speed: true,
        color: true,
    },
    Effect {
        id: 3,
        name: "Neon",
        speed: true,
        color: false,
    },
    Effect {
        id: 4,
        name: "Wave",
        speed: true,
        color: true,
    },
    Effect {
        id: 6,
        name: "Raindrop",
        speed: true,
        color: true,
    },
    Effect {
        id: 8,
        name: "Reactive",
        speed: true,
        color: true,
    },
    Effect {
        id: 10,
        name: "Sine wave",
        speed: true,
        color: true,
    },
    Effect {
        id: 11,
        name: "Kaleidoscope",
        speed: true,
        color: true,
    },
    Effect {
        id: 12,
        name: "Line wave",
        speed: true,
        color: true,
    },
    Effect {
        id: 14,
        name: "Laser",
        speed: true,
        color: true,
    },
    Effect {
        id: 15,
        name: "Circle wave",
        speed: true,
        color: true,
    },
    Effect {
        id: 16,
        name: "Dazzle",
        speed: true,
        color: true,
    },
    Effect {
        id: 19,
        name: "Reactive (inverse)",
        speed: true,
        color: true,
    },
    Effect {
        id: 24,
        name: "Fireworks",
        speed: true,
        color: true,
    },
];

const SWATCHES: [u32; 9] = [
    0xffffff, 0xff3b3b, 0xff8a2a, 0xffd23f, 0x3ddc84, 0x3ddbc4, 0x3b9bff, 0x8b7bff, 0xff4fd8,
];

const PRESETS: [(&str, &str, f32, Option<f32>); 4] = [
    ("Competitive", "0.40 mm · RT 0.10 mm", 0.4, Some(0.1)),
    ("Gaming", "1.00 mm · RT 0.30 mm", 1.0, Some(0.3)),
    ("Typing", "2.00 mm · RT off", 2.0, None),
    ("Light touch", "0.80 mm · RT off", 0.8, None),
];

/// The shared value of a field across keys, or None when they differ.
fn common<T: PartialEq + Copy>(items: &[KeyHe], f: impl Fn(&KeyHe) -> T) -> Option<T> {
    let first = f(items.first()?);
    items.iter().all(|k| f(k) == first).then_some(first)
}

fn hid(a: &KeyAction) -> i32 {
    if a[0] == 0 && a[1] == 0 && a[3] == 0 {
        a[2] as i32
    } else {
        -1
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Cx) -> impl IntoElement {
        let t = self.theme;
        // The keyboard is sized from the window: content width = window − sidebar − page padding.
        self.kb_width
            .set(f32::from(window.viewport_size().width) - 232.0 - 56.0);
        let body = if self.device.is_some() {
            self.shell(window, cx).into_any_element()
        } else {
            self.connect_screen(cx).into_any_element()
        };
        if !self.focus.is_focused(window) {
            self.focus.focus(window);
        }
        div()
            .relative()
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if this.calibration_active() {
                    cx.stop_propagation();
                } else if this.recording() {
                    this.record_keystroke(&e.keystroke, true);
                    cx.notify();
                    cx.stop_propagation();
                } else if this.edit_key(&e.keystroke, cx) {
                    cx.stop_propagation();
                } else if this.page == Page::Lighting && !e.is_held {
                    if let Some(hid) = crate::app::key_to_hid(&e.keystroke.key) {
                        this.light_press(hid);
                    }
                }
            }))
            .on_key_up(cx.listener(|this, e: &KeyUpEvent, _, cx| {
                if this.recording() {
                    this.record_keystroke(&e.keystroke, false);
                    cx.notify();
                }
            }))
            .on_modifiers_changed(cx.listener(|this, e: &ModifiersChangedEvent, _, cx| {
                if this.recording() {
                    this.record_modifiers(e.modifiers);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    // Clicking anywhere but the active field commits it.
                    if !std::mem::take(&mut this.features.field_clicked) {
                        this.commit_edit(cx);
                    }
                }),
            )
            .bg(t.bg)
            .text_color(t.text)
            .font_family(UI_FONT)
            .text_size(px(14.))
            .line_height(relative(1.4))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if this.drag.is_some() {
                    if e.pressed_button == Some(MouseButton::Left) {
                        this.drag_to(e.position, cx);
                    } else {
                        this.end_drag(cx);
                    }
                } else if this.paint.is_some() && e.pressed_button != Some(MouseButton::Left) {
                    this.paint = None;
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.end_drag(cx)),
            )
            .child(body)
            .child(self.toasts())
    }
}

impl AppView {
    fn narrow(&self) -> bool {
        self.kb_width.get() < 800.0
    }

    // ---- widgets -------------------------------------------------------------------

    fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        kind: Btn,
    ) -> Stateful<Div> {
        let t = self.theme;
        let (bg, fg, border, hover) = match kind {
            Btn::Normal => (t.surface_2, t.text, t.border_strong, t.hover),
            Btn::Primary => (t.accent, t.on_accent, t.accent, t.accent_strong),
            Btn::Actuation => (t.actuation_soft, t.actuation, t.actuation.opacity(0.45), t.actuation.opacity(0.28)),
            Btn::Ghost => (transparent_black(), t.text_2, transparent_black(), t.hover),
            Btn::Danger => (
                t.danger_soft,
                t.danger,
                t.danger.opacity(0.45),
                t.danger.opacity(0.28),
            ),
        };
        div()
            .id(id.into())
            .flex()
            .items_center()
            .justify_center()
            .h(px(34.))
            .px(px(14.))
            .rounded(px(8.))
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_color(fg)
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(13.))
            .whitespace_nowrap()
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(label.into())
    }

    fn card(
        &self,
        title: impl Into<SharedString>,
        sub: Option<&'static str>,
        right: Option<AnyElement>,
        body: impl IntoElement,
    ) -> Div {
        let t = self.theme;
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .py(px(18.))
            .px(px(20.))
            .rounded(px(12.))
            .bg(t.surface)
            .border_1()
            .border_color(t.border)
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight(650.))
                                    .child(title.into()),
                            )
                            .children(sub.map(|s| {
                                div()
                                    .text_color(t.muted)
                                    .text_size(px(12.5))
                                    .line_height(relative(1.45))
                                    .child(s)
                            })),
                    )
                    .children(right),
            )
            .child(body)
    }

    /// `id` must be unique on the page: a name, or (name, row) inside lists.
    fn segmented<T: Copy + PartialEq + 'static>(
        &self,
        id: impl Into<ElementId>,
        options: Vec<(T, String)>,
        value: Option<T>,
        on: impl Fn(&mut AppView, T, &mut Window, &mut Cx) + 'static,
        cx: &mut Cx,
    ) -> Div {
        let t = self.theme;
        let id: ElementId = id.into();
        let on = Rc::new(on);
        div()
            .flex()
            .p(px(3.))
            .gap(px(2.))
            .rounded(px(10.))
            .bg(t.surface)
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .border_1()
            .border_color(t.border)
            .children(options.into_iter().enumerate().map(|(i, (v, label))| {
                let active = value == Some(v);
                let on = on.clone();
                div()
                    .id(ElementId::NamedChild(Box::new(id.clone()), i.to_string().into()))
                    .px(px(12.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .rounded(px(7.))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .text_color(if active { t.text } else { t.text_2 })
                    .when(active, |d| d.bg(t.surface_3))
                    .when(!active, |d| d.hover(|s| s.text_color(t.text)))
                    .on_click(cx.listener(move |this, _, w, cx| on(this, v, w, cx)))
                    .child(label)
            }))
    }

    fn toggle(
        &self,
        id: &'static str,
        on: bool,
        disabled: bool,
        change: impl Fn(&mut AppView, bool, &mut Window, &mut Cx) + 'static,
        cx: &mut Cx,
    ) -> Stateful<Div> {
        let t = self.theme;
        div()
            .id(id)
            .flex_none()
            .w(px(40.))
            .h(px(22.))
            .border_1()
            .border_color(if on { t.accent } else { t.border_strong })
            .rounded_full()
            .p(px(2.))
            .flex()
            .when(on, |d| d.justify_end())
            .bg(if on { t.accent } else { t.surface_3 })
            .when(disabled, |d| d.opacity(0.4))
            .when(!disabled, |d| {
                d.cursor_pointer()
                    .on_click(cx.listener(move |this, _, w, cx| change(this, !on, w, cx)))
            })
            .child(
                div()
                    .size(px(16.))
                    .rounded_full()
                    .bg(if on { t.on_accent } else { t.text_2 }),
            )
    }

    /// Label, value readout, − / + buttons and a draggable track.
    fn slider(
        &self,
        label: &'static str,
        value: Option<f32>,
        d: SliderDrag,
        unit: &'static str,
        disabled: bool,
        cx: &mut Cx,
    ) -> Div {
        let t = self.theme;
        let decimals = if d.step < 0.01 {
            3
        } else if d.step < 1.0 {
            2
        } else {
            0
        };
        let frac = value
            .map(|v| ((v - d.min) / (d.max - d.min)).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let bounds = self.slider_bounds.clone();
        let id = d.id;
        let fill = if matches!(
            id,
            SliderId::RtBoth | SliderId::RtPress | SliderId::RtRelease
        ) {
            t.rt
        } else if id == SliderId::Actuation {
            t.actuation
        } else {
            t.accent
        };
        let text = value.map(|v| format!("{v:.decimals$}")).unwrap_or_else(|| {
            if disabled {
                "—".into()
            } else {
                "mixed".into()
            }
        });
        let step_btn = |sign: f32, label: &'static str| {
            div()
                .id((if sign < 0.0 { "minus" } else { "plus" }, id as usize))
                .size(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .text_color(t.text_2)
                .when(!disabled && value.is_some(), |b| {
                    b.cursor_pointer()
                        .hover(|s| s.bg(t.hover))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(v) = value {
                                let next = ((v + sign * d.step) / d.step).round() * d.step;
                                this.nudge(id, next.clamp(d.min, d.max), cx);
                            }
                        }))
                })
                .child(label)
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .when(disabled, |el| el.opacity(0.45))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(div().flex_1().font_weight(FontWeight(550.)).child(label))
                    .child(step_btn(-1.0, "−"))
                    .child(
                        div()
                            .w(px(96.))
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .justify_end()
                            .px(px(8.))
                            .rounded(px(7.))
                            .bg(t.surface_2)
                            .border_1()
                            .border_color(t.border)
                            .font_family(NUM_FONT)
                            .font_weight(FontWeight::MEDIUM)
                            .text_size(px(13.))
                            .child(format!("{text} {unit}")),
                    )
                    .child(step_btn(1.0, "+")),
            )
            .child(
                div()
                    .id(("track", id as usize))
                    .relative()
                    .h(px(18.))
                    .flex()
                    .items_center()
                    .child(
                        canvas(
                            move |b, _, _| {
                                bounds.borrow_mut().insert(id, b);
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(6.))
                            .rounded_full()
                            .bg(t.track)
                            .child(div().h_full().w(relative(frac)).rounded_full().bg(fill)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(relative(frac))
                            .ml(px(-9.))
                            .size(px(18.))
                            .rounded_full()
                            .bg(gpui::white())
                            .border_2()
                            .border_color(fill),
                    )
                    .when(!disabled, |el| {
                        el.cursor_pointer().on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                                this.begin_drag(d, e.position, cx)
                            }),
                        )
                    }),
            )
    }

    /// A one-line text box. Typing goes to the field while it is active.
    fn text_field(
        &self,
        field: Field,
        value: &str,
        placeholder: &'static str,
        width: f32,
        cx: &mut Cx,
    ) -> Stateful<Div> {
        let t = self.theme;
        let active = self.features.edit.as_ref().filter(|e| e.field == field);
        let text = active.map_or(value.to_string(), |e| e.text.clone());
        let empty = text.is_empty();
        let id = match field {
            Field::Search => 0,
            Field::MacroText => 1,
            Field::Repeat => 2,
            Field::RuleExe => 3,
            Field::Delay(i) => 100 + i,
        };
        div()
            .id(("field", id))
            .w(px(width))
            .h(px(32.))
            .px(px(10.))
            .flex()
            .items_center()
            .rounded(px(8.))
            .bg(t.surface_2)
            .border_1()
            .border_color(if active.is_some() { t.accent } else { t.border })
            .cursor_text()
            .overflow_hidden()
            .whitespace_nowrap()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| this.begin_edit(field, cx)),
            )
            .child(
                div()
                    .text_color(if empty { t.muted } else { t.text })
                    .text_size(px(13.))
                    .child(if empty && active.is_none() {
                        placeholder.to_string()
                    } else {
                        text
                    }),
            )
            .when(active.is_some(), |el| {
                el.child(div().ml(px(1.)).w(px(1.5)).h(px(16.)).bg(t.accent))
            })
    }

    /// The app logo: a dark rounded tile with a bold M (same design as the exe icon).
    fn logo(&self, size: f32) -> Div {
        div()
            .flex_none()
            .size(px(size))
            .rounded(px(size * 0.23))
            .border_1()
            .border_color(gpui::white().opacity(0.08))
            .bg(linear_gradient(
                180.,
                linear_color_stop(rgb(0x2e2e2e), 0.),
                linear_color_stop(rgb(0x0e0e0e), 1.),
            ))
            .flex()
            .items_center()
            .justify_center()
            .child(
                svg()
                    .path("icons/logo.svg")
                    .size(px(size * 0.86))
                    .text_color(rgb(0xf4f4f4)),
            )
    }

    fn banner(&self, text: impl Into<SharedString>, color: Hsla) -> Div {
        div()
            .p(px(10.))
            .rounded(px(8.))
            .border_1()
            .border_color(color.opacity(0.5))
            .text_color(color)
            .text_size(px(13.))
            .child(text.into())
    }

    fn small_button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        kind: Btn,
        enabled: bool,
    ) -> Stateful<Div> {
        self.button(id, label, kind)
            .h(px(28.))
            .px(px(10.))
            .text_size(px(12.))
            .when(!enabled, |b| b.opacity(0.45).cursor_default())
    }

    /// Key palette groups as clickable chips; `current` is highlighted.
    fn palette_grid(
        &self,
        id: &'static str,
        groups: Vec<PaletteGroup>,
        current: Option<KeyAction>,
        highlight: &[KeyAction],
        on_pick: impl Fn(&mut AppView, KeyAction, &mut Cx) + 'static,
        cx: &mut Cx,
    ) -> Div {
        let t = self.theme;
        let on_pick = Rc::new(on_pick);
        let query = self.features.search.to_lowercase();
        div().flex().flex_col().gap(px(16.)).children(
            groups
                .into_iter()
                .map(|g| PaletteGroup {
                    name: g.name,
                    keys: g
                        .keys
                        .into_iter()
                        .filter(|(l, _)| query.is_empty() || l.to_lowercase().contains(&query))
                        .collect(),
                })
                .filter(|g| !g.keys.is_empty())
                .enumerate()
                .map(|(gi, g)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.muted)
                                .child(g.name.to_uppercase()),
                        )
                        .child(div().flex().flex_wrap().gap(px(5.)).children(
                            g.keys.into_iter().enumerate().map(|(ki, (label, action))| {
                                let on = Some(action) == current || highlight.contains(&action);
                                let on_pick = on_pick.clone();
                                div()
                                    .id((id, gi * 100 + ki))
                                    .min_w(px(40.))
                                    .h(px(32.))
                                    .px(px(9.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(if on { t.accent } else { t.border })
                                    .bg(if on { t.accent } else { t.surface_2 })
                                    .text_color(if on { t.on_accent } else { t.text })
                                    .text_size(px(12.))
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(t.accent))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        on_pick(this, action, cx)
                                    }))
                                    .child(label)
                            }),
                        ))
                }),
        )
    }

    fn setting_row(
        &self,
        label: &'static str,
        hint: &'static str,
        control: impl IntoElement,
    ) -> Div {
        let t = self.theme;
        div()
            .flex()
            .items_center()
            .gap(px(16.))
            .py(px(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(2.))
                    .child(div().font_weight(FontWeight::MEDIUM).child(label))
                    .child(div().text_color(t.muted).text_size(px(12.)).child(hint)),
            )
            .child(control)
    }

    fn keyboard_card(&self, toolbar: Option<AnyElement>, board: Div) -> Div {
        let t = self.theme;
        // An explicit width: a percentage here sometimes collapses to the board's width.
        div()
            .w(px(self.kb_width.get()))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(20.))
            .rounded(px(12.))
            .bg(t.surface)
            .border_1()
            .border_color(t.border)
            .children(toolbar)
            .child(div().flex().justify_center().child(board))
    }

    /// Layered keycaps and keyboard bezel.
    fn keyboard(&self, select: Select, cap: impl Fn(&VisualKey) -> Cap, cx: &mut Cx) -> Div {
        let t = self.theme;
        let Some(d) = self.device.as_ref() else {
            return div();
        };
        let pad = 12.0;
        let unit = ((self.kb_width.get() - 42.0 - 26.0) / d.layout.width).clamp(24.0, 62.0);
        let compact = unit < 46.0;
        let gap = 4.0;
        let keys = d.layout.keys.iter().map(|k| {
            let c = cap(k);
            let selected = self.selection.contains(&k.index);
            let index = k.index;
            div()
                .id(("key", index))
                .absolute()
                .left(px(pad + k.x * unit + gap / 2.0))
                .top(px(pad + k.y * unit + gap / 2.0))
                .w(px(k.w * unit - gap))
                .h(px(k.h * unit - gap))
                .rounded(px(7.))
                .p(px(1.))
                .pb(px(4.))
                .bg(if selected {
                    t.accent
                } else if let Some((a, col)) = c.glow {
                    // Light leaks out round the skirt brighter than through the cap.
                    mix(t.key_edge, col, a * 0.9)
                } else {
                    t.key_edge
                })
                .when(c.dim, |el| el.opacity(0.5))
                .when(select != Select::None, |el| {
                    el.cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.press_key(index, select == Select::Single, cx)
                            }),
                        )
                        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                            if e.pressed_button == Some(MouseButton::Left) {
                                this.paint_key(index, cx);
                            }
                        }))
                })
                .child(
                    div()
                        .relative()
                        .size_full()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(t.key_border)
                        .bg(match c.glow {
                            // Lit through the whole cap, a little stronger toward the LEDs below.
                            Some((a, col)) => linear_gradient(
                                180.,
                                linear_color_stop(mix(t.key_top, col, a * 0.6), 0.),
                                linear_color_stop(mix(t.key, col, a * 0.8), 1.),
                            ),
                            None => linear_gradient(
                                180.,
                                linear_color_stop(
                                    if selected {
                                        t.key_selected_top
                                    } else {
                                        t.key_top
                                    },
                                    0.,
                                ),
                                linear_color_stop(
                                    if selected { t.key_selected } else { t.key },
                                    1.,
                                ),
                            ),
                        })
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .px(px(if compact { 3. } else { 6. }))
                        .py(px(if compact { 2. } else { 4. }))
                        .children(c.fill.map(|(f, col)| {
                            div()
                                .absolute()
                                .bottom_0()
                                .left_0()
                                .w_full()
                                .h(relative(f.clamp(0.0, 1.0)))
                                .bg(col.opacity(0.35))
                        }))
                        .child(
                            div()
                                .relative()
                                .text_size(px(if compact { 9.5 } else { 11. }))
                                .line_height(relative(1.2))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c.lit.unwrap_or(t.text_2))
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(c.legend),
                        )
                        .children(c.tag.map(|tag| {
                            if tag == "RT" {
                                div()
                                    .absolute()
                                    .top(px(if compact { 3. } else { 5. }))
                                    .right(px(if compact { 3. } else { 5. }))
                                    .size(px(if compact { 4. } else { 6. }))
                                    .rounded_full()
                                    .bg(t.rt)
                                    .into_any_element()
                            } else {
                                div()
                                    .absolute()
                                    .top(px(4.))
                                    .right(px(4.))
                                    .text_size(px(9.))
                                    .font_weight(FontWeight(650.))
                                    .px(px(4.))
                                    .rounded(px(3.))
                                    .bg(t.accent_soft)
                                    .text_color(t.accent_strong)
                                    .child(tag)
                                    .into_any_element()
                            }
                        }))
                        .children(c.value.map(|v| {
                            div()
                                .relative()
                                .w_full()
                                .flex()
                                .justify_end()
                                .font_family(NUM_FONT)
                                .font_weight(FontWeight::MEDIUM)
                                .line_height(relative(1.0))
                                .text_size(px(if compact { 9. } else { 11. }))
                                .text_color(c.lit.unwrap_or(t.text))
                                .child(v)
                        })),
                )
        });
        let width = d.layout.width * unit + pad * 2.0;
        div()
            .w(px(width + 2.0))
            .flex_none()
            .rounded(px(16.))
            .border_1()
            .border_color(t.border)
            .bg(linear_gradient(
                180.,
                linear_color_stop(t.board_top, 0.),
                linear_color_stop(t.board_bottom, 1.),
            ))
            .child(
                div()
                    .relative()
                    .w(px(width))
                    .h(px(d.layout.height * unit + pad * 2.0))
                    .children(keys),
            )
    }

    fn toasts(&self) -> Div {
        let t = self.theme;
        div()
            .absolute()
            .bottom(px(16.))
            .right(px(16.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(self.toasts.iter().map(|toast| {
                div()
                    .max_w(px(420.))
                    .px(px(14.))
                    .py(px(10.))
                    .rounded(px(8.))
                    .bg(t.surface_2)
                    .border_1()
                    .border_color(if toast.error {
                        t.danger.opacity(0.5)
                    } else {
                        t.border
                    })
                    .text_color(if toast.error { t.danger } else { t.text })
                    .child(toast.text.clone())
            }))
    }

    // ---- shell ---------------------------------------------------------------------

    fn connect_screen(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let connecting = matches!(self.status, Status::Connecting | Status::Disconnecting);
        let error = if let Status::Error(e) = &self.status {
            Some(e.clone())
        } else {
            None
        };
        div().size_full().flex().items_center().justify_center().child(
            div()
                .w(px(420.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(16.))
                .p(px(32.))
                .rounded(px(14.))
                .bg(t.surface)
                .border_1()
                .border_color(t.border)
                .child(self.logo(56.))
                .child(div().text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).child(crate::APP_NAME))
                .child(
                    div()
                        .text_color(t.muted)
                        .text_center()
                        .child("Plug in your MonsGeek or Akko Hall-effect keyboard with its USB cable. Close the official driver first so the two apps don't fight over the device."),
                )
                .children(error.map(|e| div().w_full().p(px(10.)).rounded(px(6.)).bg(t.danger_soft).text_color(t.danger).child(e)))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            self.button("connect", match self.status { Status::Disconnecting => "Disconnecting…", Status::Connecting => "Connecting…", _ => "Connect keyboard" }, Btn::Primary)
                                .when(!connecting, |b| b.on_click(cx.listener(|this, _, _, cx| this.connect(false, cx)))),
                        )
                        .child(self.button("demo", "Try demo", Btn::Normal).when(!connecting, |b| b.on_click(cx.listener(|this, _, _, cx| this.connect(true, cx))))),
                ),
        )
    }

    fn shell(&mut self, window: &mut Window, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let (title, sub) = self.page.title();
        let profile = d.profile;
        let name = d.name.clone();
        let busy = self.busy > 0;
        let sidebar = div()
            .w(px(232.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(12.))
            .py(px(16.))
            .bg(t.sidebar)
            .border_r_1()
            .border_color(t.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(6.))
                    .pb(px(14.))
                    .child(self.logo(26.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .child(crate::APP_NAME),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(6.))
                    .pt(px(4.))
                    .pb(px(14.))
                    .mb(px(6.))
                    .border_b_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .size(px(34.))
                            .flex_none()
                            .rounded(px(8.))
                            .bg(t.surface_2)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(t.text_2)
                            .child(
                                svg()
                                    .path("icons/keyboard.svg")
                                    .size(px(18.))
                                    .text_color(t.text_2),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.))
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(if self.demo { t.warn } else { t.ok })
                                    .child(if self.demo { "Demo mode" } else { "Connected" }),
                            ),
                    ),
            )
            .child(
                div()
                    .pt(px(12.))
                    .pb(px(6.))
                    .px(px(10.))
                    .text_size(px(11.))
                    .text_color(t.muted)
                    .child("CONFIGURE"),
            )
            .children(Page::ALL.iter().map(|&p| {
                let active = self.page == p;
                div()
                    .id(p.label())
                    .px(px(12.))
                    .h(px(40.))
                    .gap(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .flex()
                    .items_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_color(if active { t.text } else { t.text_2 })
                    .when(active, |el| el.bg(t.accent_soft))
                    .when(!active, |el| {
                        el.hover(|s| s.bg(t.surface).text_color(t.text))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.set_page(p, cx)))
                    .child(
                        svg()
                            .path(match p {
                                Page::Performance => "icons/gauge.svg",
                                Page::Remap => "icons/remap.svg",
                                Page::Advanced => "icons/bolt.svg",
                                Page::Lighting => "icons/bulb.svg",
                                Page::Tester => "icons/pulse.svg",
                                Page::AutoProfiles => "icons/swap.svg",
                                Page::Settings => "icons/settings.svg",
                            })
                            .size(px(18.))
                            .text_color(if active { t.accent_strong } else { t.muted }),
                    )
                    .child(p.label())
            }))
            .child(div().flex_1())
            .child(
                self.button(
                    "disconnect",
                    if self.demo { "Exit demo" } else { "Disconnect" },
                    Btn::Danger,
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| this.disconnect(cx))),
            );

        let header = div()
            .flex()
            .items_center()
            .when(self.kb_width.get() < 900.0, |d| d.flex_col().items_start())
            .gap(px(16.))
            .px(px(28.))
            .pt(px(18.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight(650.))
                            .child(title),
                    )
                    .child(div().text_size(px(13.)).text_color(t.muted).child(sub)),
            )
            .children(
                (self.kb_width.get() >= 900.0 || busy || self.transitioning || self.closing).then(
                    || {
                        div()
                            .w(px(100.))
                            .text_color(t.muted)
                            .text_size(px(12.))
                            .child(if self.closing {
                                "Finishing saves…"
                            } else if self.transitioning {
                                "Loading profile…"
                            } else if busy {
                                "Saving…"
                            } else {
                                ""
                            })
                    },
                ),
            )
            .child(
                self.segmented(
                    "profile",
                    (0..4)
                        .map(|p| (p as u8, format!("Profile {}", p + 1)))
                        .collect(),
                    Some(profile),
                    |this, p, _, cx| this.set_profile(p, cx),
                    cx,
                ),
            );

        let page = match self.page {
            Page::Performance => self.performance(cx),
            Page::Remap => self.remap(cx),
            Page::Advanced => self.advanced(cx),
            Page::Lighting => self.lighting(cx),
            Page::Tester => self.tester(cx),
            Page::AutoProfiles => self.auto_profiles(cx),
            Page::Settings => self.settings(window, cx),
        };
        div().size_full().flex().child(sidebar).child(
            div()
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .flex()
                .flex_col()
                .child(header)
                .child(
                    div()
                        .id(("content", self.page as usize))
                        .flex_1()
                        .overflow_y_scroll()
                        .px(px(28.))
                        .pt(px(16.))
                        .pb(px(28.))
                        .child(page.w_full()),
                ),
        )
    }

    // ---- pages ---------------------------------------------------------------------

    fn performance(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let sel: Vec<KeyHe> = self
            .selection
            .iter()
            .filter_map(|&i| d.keys.get(i).copied())
            .collect();
        let none = sel.is_empty();
        let (min, max) = d.def.travel();
        let step = if d.info.steps_per_mm >= 200.0 {
            0.005
        } else {
            0.01
        };
        let actuation = common(&sel, |k| k.actuation);
        let release = common(&sel, |k| k.release);
        let rt = common(&sel, |k| k.rapid_trigger);
        let any_rt = sel.iter().any(|k| k.rapid_trigger);
        let rtp = common(&sel, |k| k.rt_press);
        let rtr = common(&sel, |k| k.rt_release);
        let dz = common(&sel, |k| k.deadzone);
        let keys = d.keys.clone();
        let count = self.selection.len();

        let board = self.keyboard(
            Select::Multi,
            |k| {
                let s = keys[k.index];
                Cap {
                    legend: action_label(&k.base),
                    value: Some(format!("{:.2}", s.actuation)),
                    tag: s.rapid_trigger.then_some("RT"),
                    ..Default::default()
                }
            },
            cx,
        );
        let toolbar = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().flex_1().text_color(t.muted).child(if count == 0 {
                "Click or drag across keys to select them".to_string()
            } else {
                format!(
                    "{count} {} selected",
                    if count == 1 { "key" } else { "keys" }
                )
            }))
            .child(
                self.button("sel-wasd", "WASD", Btn::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_group(|a| [0x1a, 0x04, 0x16, 0x07].contains(&hid(a)), cx)
                    })),
            )
            .child(
                self.button("sel-letters", "Letters", Btn::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_group(|a| (0x04..=0x1d).contains(&hid(a)), cx)
                    })),
            )
            .child(
                self.button("sel-all", "Select all", Btn::Normal)
                    .on_click(cx.listener(|this, _, _, cx| this.select_group(|_| true, cx))),
            )
            .child(
                self.button("sel-none", "Clear", Btn::Normal)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selection.clear();
                        cx.notify();
                    })),
            );

        let presets =
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.))
                .children(
                    PRESETS
                        .iter()
                        .enumerate()
                        .map(|(i, &(name, desc, act, rt))| {
                            div()
                                .id(("preset", i))
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .px(px(12.))
                                .py(px(9.))
                                .min_w(px(140.))
                                .rounded(px(8.))
                                .bg(t.surface_2)
                                .border_1()
                                .border_color(t.border)
                                .when(none, |el| el.opacity(0.45))
                                .when(!none, |el| {
                                    el.cursor_pointer()
                                        .hover(|s| s.border_color(t.accent))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.update_keys(
                                                move |k| {
                                                    k.actuation = act;
                                                    k.release = act;
                                                    k.rapid_trigger = rt.is_some();
                                                    if let Some(r) = rt {
                                                        k.rt_press = r;
                                                        k.rt_release = r;
                                                    }
                                                },
                                                cx,
                                            )
                                        }))
                                })
                                .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                                .child(div().text_color(t.muted).text_size(px(12.)).child(desc))
                        }),
                );

        let act_card = self.card(
            "Actuation point",
            Some("How far a key travels before it registers. Lower is faster; higher prevents accidental presses."),
            None,
            div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(self.slider("Actuation", actuation, SliderDrag { id: SliderId::Actuation, min, max, step }, "mm", none, cx))
                // With Rapid Trigger on, the key resets as soon as it moves up, so the release point is never used.
                .when(rt != Some(true), |el| el.child(self.slider("Release point", release, SliderDrag { id: SliderId::Release, min, max, step }, "mm", none, cx)))
                .child(presets),
        );

        let linked = self.linked_rt;
        let rt_toggle = self
            .toggle(
                "rt",
                rt == Some(true),
                none,
                |this, v, _, cx| this.update_keys(move |k| k.rapid_trigger = v, cx),
                cx,
            )
            .when(rt == Some(true), |toggle| {
                toggle.bg(t.rt).border_color(t.rt)
            });
        let rt_body = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(self.toggle(
                        "rt-link",
                        linked,
                        none || !any_rt,
                        |this, v, _, cx| {
                            this.linked_rt = v;
                            if v {
                                this.update_keys(|k| k.rt_release = k.rt_press, cx);
                            }
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(
                        div()
                            .text_color(t.text_2)
                            .child("Same value for press and release"),
                    ),
            )
            .map(|el| {
                if linked {
                    el.child(self.slider(
                        "Sensitivity",
                        if rtp == rtr { rtp } else { None },
                        SliderDrag {
                            id: SliderId::RtBoth,
                            min: 0.01,
                            max: 2.0,
                            step: 0.01,
                        },
                        "mm",
                        none || !any_rt,
                        cx,
                    ))
                } else {
                    el.child(self.slider(
                        "Press sensitivity",
                        rtp,
                        SliderDrag {
                            id: SliderId::RtPress,
                            min: 0.01,
                            max: 2.0,
                            step: 0.01,
                        },
                        "mm",
                        none || !any_rt,
                        cx,
                    ))
                    .child(self.slider(
                        "Release sensitivity",
                        rtr,
                        SliderDrag {
                            id: SliderId::RtRelease,
                            min: 0.01,
                            max: 2.0,
                            step: 0.01,
                        },
                        "mm",
                        none || !any_rt,
                        cx,
                    ))
                }
            });
        let rt_card = self.card(
            "Rapid Trigger",
            Some("After the first press at the actuation point, the key resets as soon as it moves up and re-triggers as soon as it moves down, until it comes all the way up."),
            Some(rt_toggle.into_any_element()),
            rt_body,
        );
        let dz_card = self.card(
            "Dead zone",
            Some("Ignore the very bottom of the travel to filter out switch wobble."),
            None,
            self.slider(
                "Bottom dead zone",
                dz,
                SliderDrag {
                    id: SliderId::Deadzone,
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                },
                "mm",
                none,
                cx,
            ),
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
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .when(self.narrow(), |d| d.w_full())
                            .child(act_card),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .when(self.narrow(), |d| d.w_full())
                            .flex()
                            .flex_col()
                            .gap(px(16.))
                            .child(rt_card)
                            .child(dz_card),
                    ),
            )
    }

    fn remap(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let layer = self.remap_layer;
        let map = if layer == 0 {
            d.keymap.clone()
        } else {
            d.fnmap.clone()
        };
        let defaults = if layer == 0 {
            d.defaults.clone()
        } else {
            d.fn_defaults.clone()
        };
        let target = if self.selection.len() == 1 {
            Some(self.selection[0])
        } else {
            None
        };
        // Keys that already play a macro open the macro editor straight away.
        if let Some(i) = target {
            let opened = self.features.macro_draft.as_ref().map(|m| (m.index, m.layer));
            if opened != Some((i, layer)) {
                self.features.remap_tab = if map[i][0] == crate::protocol::macros::MACRO_ACTION {
                    RemapTab::Macro
                } else {
                    RemapTab::Key
                };
                self.ensure_macro_draft(i, cx);
            }
        }
        let d = self.device.as_ref().unwrap();
        let fn_layers = d.def.fn_layer_count();
        let (fn_layer, active_fn) = (d.fn_layer, d.options.fn_index);
        let base_names = d.defaults.clone();
        let shortcuts: Vec<(usize, KeyAction)> = d
            .layout
            .keys
            .iter()
            .filter(|k| map[k.index] != [0, 0, 0, 0])
            .map(|k| (k.index, map[k.index]))
            .collect();

        let m = map.clone();
        let dfl = defaults.clone();
        let board = self.keyboard(
            Select::Single,
            move |k| {
                let a = m[k.index];
                if layer == 1 && a == [0, 0, 0, 0] {
                    // No Fn function: show which key it is, dimmed.
                    return Cap {
                        legend: action_label(&k.base),
                        dim: true,
                        ..Default::default()
                    };
                }
                Cap {
                    legend: action_label(&a),
                    tag: (a != dfl[k.index]).then_some("MOD"),
                    ..Default::default()
                }
            },
            cx,
        );

        let mut controls = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(self.segmented(
                "layer",
                vec![(0u8, "Base layer".into()), (1u8, "Fn layer".into())],
                Some(layer),
                |this, l, _, cx| this.set_remap_layer(l, cx),
                cx,
            ));
        if layer == 1 && fn_layers > 1 {
            controls = controls.child(
                self.segmented(
                    "fn-layer",
                    (0..fn_layers)
                        .map(|l| {
                            (
                                l,
                                if l == active_fn {
                                    format!("Fn {} · active", l + 1)
                                } else {
                                    format!("Fn {}", l + 1)
                                },
                            )
                        })
                        .collect(),
                    Some(fn_layer),
                    |this, l, _, cx| this.set_fn_layer(l, cx),
                    cx,
                ),
            );
        }

        let editor = match target {
            None => div()
                .p(px(18.))
                .rounded(px(10.))
                .border_1()
                .border_color(t.border)
                .text_color(t.muted)
                .child("Pick a key on the keyboard, then choose what it should send."),
            Some(index) => {
                let current = map[index];
                let tab = self.features.remap_tab;
                let reset_disabled = current == defaults[index];
                let right = div()
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .gap(px(6.))
                    .child(self.segmented(
                        "remap-tab",
                        vec![(RemapTab::Key, "Key".into()), (RemapTab::Macro, "Macro".into())],
                        Some(tab),
                        |this, v, _, cx| {
                            if this.recording() {
                                this.toggle_recording(cx);
                            }
                            this.features.remap_tab = v;
                            cx.notify();
                        },
                        cx,
                    ))
                    .when(tab == RemapTab::Key, |el| {
                        el.child(self.text_field(
                            Field::Search,
                            &self.features.search,
                            "Search keys…",
                            150.,
                            cx,
                        ))
                    })
                    .child(
                        self.button("reset-key", "Reset to factory", Btn::Normal)
                            .when(reset_disabled, |b| b.opacity(0.45))
                            .when(!reset_disabled, |b| {
                                let def = defaults[index];
                                b.on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_key_action(index, def, cx)
                                }))
                            }),
                    )
                    .when(layer == 1, |el| {
                        let empty = current == [0, 0, 0, 0];
                        el.child(
                            self.button("clear-fn", "Clear", Btn::Ghost)
                                .when(empty, |b| b.opacity(0.45))
                                .when(!empty, |b| {
                                    b.on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_key_action(index, [0, 0, 0, 0], cx)
                                    }))
                                }),
                        )
                    });
                let body = if tab == RemapTab::Macro {
                    self.macro_editor(index, current, action_label(&base_names[index]), cx)
                } else {
                    self.palette_grid(
                        "pal",
                        palette(),
                        Some(current),
                        &[],
                        move |this, a, cx| this.set_key_action(index, a, cx),
                        cx,
                    )
                };
                let what = if layer == 1 && current == [0, 0, 0, 0] {
                    "Nothing".to_string()
                } else {
                    action_label(&current)
                };
                self.card(
                    format!(
                        "{} {} → {}",
                        if layer == 1 { "Fn +" } else { "Remap" },
                        action_label(&base_names[index]),
                        what
                    ),
                    Some(if tab == RemapTab::Macro {
                        "A macro plays a sequence of key presses and releases. Macros are stored on the keyboard (25 slots shared by all profiles and Fn layers)."
                    } else if layer == 0 {
                        "Saved to the active profile as soon as you pick."
                    } else {
                        "What the key does while Fn is held. Fn layers are shared by all profiles."
                    }),
                    Some(right.into_any_element()),
                    body,
                )
            }
        };

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(self.keyboard_card(Some(controls.into_any_element()), board))
            .when(layer == 1 && fn_layer != active_fn, |el| {
                el.child(div().p(px(10.)).rounded(px(8.)).border_1().border_color(t.warn.opacity(0.5)).text_color(t.warn).child(format!(
                    "You're editing Fn layer {}, but the keyboard is using Fn layer {}. Press Fn + Right Alt to switch.",
                    fn_layer + 1,
                    active_fn + 1
                )))
            });
        if layer == 1 {
            let list =
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(shortcuts.iter().map(|&(i, a)| {
                        div()
                            .id(("short", i))
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|s| s.bg(t.hover))
                            .when(target == Some(i), |el| el.bg(t.hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selection = vec![i];
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(110.))
                                    .font_family(NUM_FONT)
                                    .text_size(px(12.))
                                    .child(format!("Fn + {}", action_label(&base_names[i]))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(t.text_2)
                                    .child(action_description(&a)),
                            )
                            .children((a != defaults[i]).then(|| {
                                div().text_size(px(10.)).text_color(t.accent).child("MOD")
                            }))
                    }));
            page = page.child(
                div()
                    .flex()
                    .gap(px(16.))
                    .items_start()
                    .when(self.narrow(), |d| d.flex_col())
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .when(self.narrow(), |d| d.w_full())
                            .child(editor),
                    )
                    .child(
                        div()
                            .w(px(360.))
                            .when(self.narrow(), |d| d.w_full())
                            .flex_none()
                            .child(self.card(
                                format!("Fn shortcuts · layer {}", fn_layer + 1),
                                Some("Everything this Fn layer does. Click a row to edit it."),
                                None,
                                list,
                            )),
                    ),
            );
        } else {
            page = page.child(editor);
        }
        page
    }

    fn lighting_tabs(&self, cx: &mut Cx) -> Option<Div> {
        (self.picture_count() > 0).then(|| {
            div().flex().child(self.segmented(
                "light-tab",
                vec![
                    (LightingTab::Effects, "Effects".into()),
                    (LightingTab::PerKey, "Per-key colours".into()),
                ],
                Some(self.features.lighting_tab),
                |this, v, _, cx| this.set_lighting_tab(v, cx),
                cx,
            ))
        })
    }

    fn lighting(&mut self, cx: &mut Cx) -> Div {
        if self.features.lighting_tab == LightingTab::PerKey {
            return self.per_key(cx);
        }
        let tabs = self.lighting_tabs(cx);
        self.tick_light_preview(cx);
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let l = d.lighting;
        let effect = EFFECTS.iter().find(|e| e.id == l.effect);
        let picture = l.effect == crate::protocol::userpic::USER_PICTURE_EFFECT;
        let off = l.effect == 0;
        // Animated per-key colours, as the keyboard shows them.
        let keys = crate::light_preview::centres(&d.layout);
        let t_now = self.light_time();
        let mut presses = self.light_presses.clone();
        if crate::light_preview::REACTIVE.contains(&l.effect)
            && !presses.iter().any(|e| t_now - e.t < 2.0)
            && !keys.is_empty()
        {
            // Idle: fake a press every 0.45 s so the effect is visible.
            presses = (0..4)
                .map(|i| {
                    let n = (t_now / 0.45).floor() - i as f32;
                    let (index, x, y) = keys[((n * 91.7).sin().abs() * keys.len() as f32) as usize
                        % keys.len()];
                    crate::light_preview::Press {
                        index,
                        x,
                        y,
                        t: n * 0.45,
                    }
                })
                .collect();
        }
        let lights: std::collections::HashMap<usize, (Hsla, f32)> = keys
            .iter()
            .map(|k| k.0)
            .zip(crate::light_preview::render(
                &l,
                effect.is_some_and(|e| e.color),
                &keys,
                t_now,
                &presses,
            ))
            .collect();
        let brightness = 0.3 + 0.7 * l.brightness as f32 / 4.0;
        let text = t.text;

        let board = self.keyboard(
            Select::None,
            move |k| {
                let legend = action_label(&k.base);
                let Some(&(c, level)) = lights.get(&k.index).filter(|_| !off) else {
                    return Cap {
                        legend,
                        dim: off,
                        ..Default::default()
                    };
                };
                let a = level * brightness;
                if a < 0.02 {
                    return Cap {
                        legend,
                        ..Default::default()
                    };
                }
                Cap {
                    legend,
                    glow: Some((a, c)),
                    // Legends shine brightest: the light passes straight through them.
                    lit: Some(mix(text, c, a.min(1.0) * 0.3)),
                    ..Default::default()
                }
            },
            cx,
        );

        let effects =
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .children(EFFECTS.iter().map(|e| {
                    let active = e.id == l.effect;
                    let id = e.id;
                    div()
                        .id(("fx", e.id as usize))
                        .w(px(138.))
                        .h(px(34.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(7.))
                        .border_1()
                        .border_color(if active { t.accent } else { t.border })
                        .bg(if active { t.accent_soft } else { t.surface_2 })
                        .cursor_pointer()
                        .hover(|s| s.border_color(t.accent))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_lighting(|l| l.effect = id, cx)
                        }))
                        .child(e.name)
                }));

        let settings = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(self.slider(
                "Brightness",
                Some(l.brightness as f32),
                SliderDrag {
                    id: SliderId::Brightness,
                    min: 0.0,
                    max: 4.0,
                    step: 1.0,
                },
                "",
                off,
                cx,
            ))
            .when(effect.is_some_and(|e| e.speed), |el| {
                el.child(self.slider(
                    "Speed",
                    Some(l.speed as f32),
                    SliderDrag {
                        id: SliderId::Speed,
                        min: 0.0,
                        max: 4.0,
                        step: 1.0,
                    },
                    "",
                    off,
                    cx,
                ))
            })
            .when(effect.is_some_and(|e| e.color), |el| {
                el.child(self.setting_row(
                    "Rainbow",
                    "Cycle through colours instead of a single colour.",
                    self.toggle(
                        "rainbow",
                        l.rainbow,
                        off,
                        |this, v, _, cx| this.edit_lighting(|l| l.rainbow = v, cx),
                        cx,
                    ),
                ))
                .when(!l.rainbow, |el| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(div().font_weight(FontWeight::MEDIUM).child("Colour"))
                            .child(div().flex().gap(px(8.)).children(SWATCHES.iter().map(
                                |&hex| {
                                    let rgb3 = [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8];
                                    let active = l.color == rgb3;
                                    div()
                                        .id(("sw", hex as usize))
                                        .size(px(26.))
                                        .rounded(px(7.))
                                        .bg(rgb(hex))
                                        .border_2()
                                        .border_color(if active {
                                            t.text
                                        } else {
                                            t.border_strong
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.edit_lighting(|l| l.color = rgb3, cx)
                                        }))
                                },
                            ))),
                    )
                })
            });

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .children(tabs)
            .child(self.keyboard_card(None, board))
            .when(picture, |el| {
                el.child(self.banner(
                    format!(
                        "The keyboard is showing per-key colour picture {}. Edit it in the Per-key colours tab, or pick an effect.",
                        l.option + 1
                    ),
                    t.accent_strong,
                ))
            })
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .items_start()
                    .when(self.narrow(), |d| d.flex_col())
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .when(self.narrow(), |d| d.w_full())
                            .child(self.card(
                                "Effect",
                                Some("Lighting is stored per profile."),
                                None,
                                effects,
                            )),
                    )
                    .child(
                        div()
                            .w(px(340.))
                            .when(self.narrow(), |d| d.w_full())
                            .flex_none()
                            .child(self.card(
                                effect.map_or("Effect".to_string(), |e| e.name.to_string()),
                                None,
                                None,
                                settings,
                            )),
                    ),
            )
            .child(self.layer_lighting_card(cx))
    }

    fn settings(&mut self, window: &mut Window, cx: &mut Cx) -> Div {
        let _ = window;
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let fw = d.info.firmware;
        let o = d.options;
        let info_row = |label: &'static str, v: String| {
            div()
                .flex()
                .justify_between()
                .py(px(5.))
                .child(div().text_color(t.muted).child(label))
                .child(div().font_family(NUM_FONT).text_size(px(12.5)).child(v))
        };
        let rate = d.report_rate;
        let polling = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().font_weight(FontWeight::MEDIUM).child("Polling rate"))
            .map(|el| match rate {
                Some(hz) => el
                    .child(
                        div()
                            .text_color(t.muted)
                            .text_size(px(12.))
                            .child("How often the keyboard reports to the PC. 8K needs a fast USB port and CPU."),
                    )
                    .child(div().flex().child(self.segmented(
                        "rate",
                        crate::protocol::REPORT_RATES
                            .iter()
                            .map(|&r| {
                                (r, if r >= 1000 { format!("{}K", r / 1000) } else { r.to_string() })
                            })
                            .collect(),
                        Some(hz),
                        |this, v, _, cx| this.set_report_rate(v, cx),
                        cx,
                    ))),
                None => el.child(div().text_color(t.muted).text_size(px(12.)).child(format!(
                    "Fixed by the firmware. v{}.{:02x} ignores polling-rate changes; the official driver only offers a choice from firmware 4.00.",
                    fw >> 8,
                    fw & 0xff
                ))),
            });
        let keyboard = self.card(
            "Keyboard",
            None,
            None,
            div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(polling)
                .child(self.switch_type_row(cx))
                .child(self.slider(
                    "Debounce",
                    Some(d.debounce as f32),
                    SliderDrag {
                        id: SliderId::Debounce,
                        min: 0.0,
                        max: 10.0,
                        step: 1.0,
                    },
                    "ms",
                    false,
                    cx,
                ))
                .child(self.slider(
                    "Rapid Trigger stability",
                    Some(o.rt_stability as f32),
                    SliderDrag {
                        id: SliderId::Stability,
                        min: 0.0,
                        max: 125.0,
                        step: 25.0,
                    },
                    "",
                    false,
                    cx,
                ))
                .child(self.setting_row(
                    "Swap WASD and arrows",
                    "For games that only support arrow keys.",
                    self.toggle(
                        "wasd",
                        o.wasd_swap,
                        false,
                        |this, v, _, cx| this.edit_options(|o| o.wasd_swap = v, cx),
                        cx,
                    ),
                ))
                .child(self.setting_row(
                    "Accidental press guard",
                    "The firmware's anti-mistouch filter.",
                    self.toggle(
                        "guard",
                        o.anti_mistouch,
                        false,
                        |this, v, _, cx| this.edit_options(|o| o.anti_mistouch = v, cx),
                        cx,
                    ),
                )),
        );
        let device = self.card(
            "Device",
            None,
            None,
            div()
                .flex()
                .flex_col()
                .child(info_row("Model", d.def.display_name.clone()))
                .child(info_row("Device ID", d.info.device_id.to_string()))
                .child(info_row(
                    "Firmware",
                    format!("v{}.{:02x}", fw >> 8, fw & 0xff),
                ))
                .child(info_row(
                    "Travel precision",
                    format!("{:.3} mm", 1.0 / d.info.steps_per_mm),
                ))
                .child(info_row(
                    "System mode",
                    ["Windows", "macOS", "iOS", "Android"]
                        .get(o.os as usize)
                        .unwrap_or(&"Windows")
                        .to_string(),
                ))
                .child(info_row(
                    "Connection",
                    if self.demo {
                        "Demo (simulated)".into()
                    } else {
                        "USB".into()
                    },
                )),
        );
        let confirming = self.confirm_reset;
        let reset = self.card(
            "Factory reset",
            Some("Restores every profile, keymap, lighting and actuation setting to factory defaults. This can't be undone."),
            None,
            if confirming {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1().text_color(t.danger).child("Reset all four profiles?"))
                    .child(self.button("reset-cancel", "Cancel", Btn::Normal).on_click(cx.listener(|this, _, _, cx| {
                        this.confirm_reset = false;
                        cx.notify();
                    })))
                    .child(self.button("reset-go", "Reset keyboard", Btn::Danger).on_click(cx.listener(|this, _, _, cx| this.factory_reset(cx))))
            } else {
                div().child(self.button("reset", "Factory reset…", Btn::Danger).on_click(cx.listener(|this, _, _, cx| {
                    this.confirm_reset = true;
                    cx.notify();
                })))
            },
        );
        div()
            .flex()
            .gap(px(16.))
            .items_start()
            .when(self.narrow(), |d| d.flex_col())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .when(self.narrow(), |d| d.w_full())
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(keyboard),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .when(self.narrow(), |d| d.w_full())
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(device)
                    .child(self.backup_card(cx))
                    .child(reset),
            )
    }
}

/// Blends `b` over `a` by `t` (0..1), in RGB.
fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b, t) = (Rgba::from(a), Rgba::from(b), t.clamp(0.0, 1.0));
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
    .into()
}
