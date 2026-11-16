//! Rendering: shell, widgets and the pages. Larger page parts live in the child modules.

mod advanced;
mod auto_profiles;
mod backup_card;
mod calibration;
mod layer_card;
mod macro_editor;
mod perkey;
pub mod smooth_scroll;
mod tester;

use crate::app::{
    AppView, CalStep, Field, LightingTab, Page, RemapTab, SliderDrag, SliderId, Status, TesterTab,
};
use crate::devices::VisualKey;
use crate::keycodes::{action_description, action_label, palette, PaletteGroup};
use crate::protocol::{KeyAction, KeyHe};
use crate::actions::*;
use crate::theme::{u, Numeric, UI_FONT};
use gpui::component::button::{Button, ButtonVariants as _};
use gpui::component::menu::DropdownMenu as _;
use gpui::component::notification::Notification;
use gpui::component::switch::Switch;
use gpui::component::{TitleBar, TITLE_BAR_HEIGHT};
use gpui::component::{Disableable as _, Selectable as _, Sizable as _, WindowExt as _};
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

type Cx<'a> = Context<'a, AppView>;

/// Button emphasis, by meaning: `Primary` only for the default commit of a decision area.
#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Normal,
    Primary,
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
        self.sync_theme(window, cx);
        let t = self.theme;
        // The keyboard is sized from the window: content width = window − sidebar − page
        // padding (both in rems, so they grow with the interface size).
        let scale = f32::from(window.rem_size()) / crate::config::BASE_FONT;
        self.ui_scale = scale;
        self.kb_width
            .set(f32::from(window.viewport_size().width) - (232.0 + 56.0) * scale);
        for (text, error) in std::mem::take(&mut self.pending_notes) {
            // Bottom right, clear of the header's profile switcher.
            let note = if error {
                Notification::error(text).autohide(false)
            } else {
                Notification::info(text)
            }
            .placement(Anchor::BottomRight);
            window.push_notification(note, cx);
        }
        if let Some((profile, file)) = self.features.pending_import.take() {
            self.confirm_import(profile, file, window, cx);
        }
        let body = if self.device.is_some() {
            self.shell(window, cx).into_any_element()
        } else {
            self.connect_screen(cx).into_any_element()
        };
        // While the app reads raw keys (macro recording, calibration, the polling test, typing
        // into a value box) every key must reach it: keep focus on the root, so a focused
        // button can't take Space or Enter, and drop the key context, so shortcuts don't fire.
        let capturing = self.recording()
            || self.calibration_active()
            || self.tools.polling()
            || self.features.edit.is_some();
        // Otherwise keys go to the app unless a control (a kit input, a button) has focus.
        if window.focused(cx).is_none() || (capturing && !self.focus.is_focused(window)) {
            self.focus.focus(window, cx);
        }
        div()
            .relative()
            .size_full()
            .when(!capturing, |el| el.key_context(CONTEXT))
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.undo(cx)))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.redo(cx)))
            .on_action(cx.listener(|this, _: &SelectAllKeys, _, cx| this.select_all_keys(cx)))
            .on_action(cx.listener(|this, _: &ClearSelection, _, cx| {
                this.selection.clear();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ZoomIn, w, cx| this.zoom(1, w, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, w, cx| this.zoom(-1, w, cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, w, cx| this.zoom(0, w, cx)))
            .on_action(cx.listener(|this, _: &GoPerformance, _, cx| this.go(Page::Performance, cx)))
            .on_action(cx.listener(|this, _: &GoRemap, _, cx| this.go(Page::Remap, cx)))
            .on_action(cx.listener(|this, _: &GoAdvanced, _, cx| this.go(Page::Advanced, cx)))
            .on_action(cx.listener(|this, _: &GoLighting, _, cx| this.go(Page::Lighting, cx)))
            .on_action(cx.listener(|this, _: &GoTester, _, cx| this.go(Page::Tester, cx)))
            .on_action(cx.listener(|this, _: &GoAutoProfiles, _, cx| this.go(Page::AutoProfiles, cx)))
            .on_action(cx.listener(|this, _: &GoSettings, _, cx| this.go(Page::Settings, cx)))
            .on_action(cx.listener(|this, _: &Profile1, _, cx| this.set_profile(0, cx)))
            .on_action(cx.listener(|this, _: &Profile2, _, cx| this.set_profile(1, cx)))
            .on_action(cx.listener(|this, _: &Profile3, _, cx| this.set_profile(2, cx)))
            .on_action(cx.listener(|this, _: &Profile4, _, cx| this.set_profile(3, cx)))
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
            .text_size(u(14.))
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
            .child(self.title_bar())
    }
}

impl AppView {
    /// Keeps the kit theme (mode, colours, base size) in step with the window's appearance and
    /// the user's choices.
    pub fn sync_theme(&mut self, window: &mut Window, cx: &mut Cx) {
        let wanted = (
            crate::theme::is_dark(self.config.appearance, window),
            self.config.font_size(),
        );
        if self.applied_theme != Some(wanted) {
            self.applied_theme = Some(wanted);
            crate::theme::apply(wanted.0, wanted.1, cx);
            self.theme = crate::theme::Theme::for_mode(wanted.0);
        }
    }

    /// Ctrl + / Ctrl − / Ctrl 0: one interface size up, down, or back to the default.
    fn zoom(&mut self, dir: i32, window: &mut Window, cx: &mut Cx) {
        use crate::config::{BASE_FONT, UI_SIZES};
        let now = self.config.font_size();
        let i = UI_SIZES.iter().position(|&s| s == now).unwrap_or(1) as i32;
        let next = match dir {
            0 => BASE_FONT,
            d => UI_SIZES[(i + d).clamp(0, UI_SIZES.len() as i32 - 1) as usize],
        };
        self.set_ui_size(next, window, cx);
    }

    fn set_ui_size(&mut self, size: f32, window: &mut Window, cx: &mut Cx) {
        let size = (size != crate::config::BASE_FONT).then_some(size);
        if self.config.ui_scale == size {
            return;
        }
        self.config.ui_scale = size;
        self.save_config(cx);
        self.sync_theme(window, cx);
        cx.notify();
    }

    fn set_appearance(&mut self, a: crate::config::Appearance, window: &mut Window, cx: &mut Cx) {
        self.config.appearance = a;
        self.save_config(cx);
        self.sync_theme(window, cx);
        cx.notify();
    }

    fn go(&mut self, page: Page, cx: &mut Cx) {
        if self.device.is_some() {
            self.set_page(page, cx);
        }
    }

    fn select_all_keys(&mut self, cx: &mut Cx) {
        if self.device.is_some() && matches!(self.page, Page::Performance | Page::Advanced) {
            self.select_group(|_| true, cx);
        }
    }
}

impl AppView {
    /// The content area is too narrow for two columns of cards. Measured in design pixels,
    /// so larger interface sizes switch to one column sooner.
    fn narrow(&self) -> bool {
        self.kb_width.get() / self.ui_scale < 800.0
    }

    // ---- widgets -------------------------------------------------------------------

    fn button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        kind: Btn,
    ) -> Button {
        let b = Self::roomy(Button::new(id)).label(label);
        match kind {
            Btn::Normal => b,
            Btn::Primary => b.primary(),
            Btn::Ghost => b.ghost(),
            Btn::Danger => b.danger(),
        }
    }

    /// Standard control height: a little taller than the kit's medium (28 px at the
    /// default size), so buttons are comfortable to hit and read.
    fn roomy(b: Button) -> Button {
        b.h(u(34.)).px(u(14.))
    }

    /// Compact control height, for buttons repeated in rows and toolbars.
    fn roomy_sm(b: Button) -> Button {
        b.h(u(30.)).px(u(12.))
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
            .gap(u(16.))
            .p(u(20.))
            .rounded(u(12.))
            .bg(t.surface)
            .border_1()
            .border_color(t.border)
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(u(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(u(4.))
                            .flex_1()
                            .min_w(u(0.))
                            .child(
                                div()
                                    .text_size(u(15.))
                                    .line_height(relative(1.3))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title.into()),
                            )
                            .children(sub.map(|s| {
                                div()
                                    .text_color(t.muted)
                                    .text_size(u(13.))
                                    .line_height(relative(1.5))
                                    .child(s)
                            })),
                    )
                    .children(right),
            )
            .child(body)
    }

    /// A segmented choice: options sit in a sunken track and the selected one is raised out of
    /// it. Each option is its own `Button`, so Tab reaches every one and Space picks it.
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
        // Track radius 8, inset 2: the segments use 6 so the two curves stay concentric.
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(u(2.))
            .p(u(2.))
            .rounded(u(8.))
            .bg(t.well)
            .border_1()
            .border_color(t.border)
            .children(options.into_iter().enumerate().map(|(i, (v, label))| {
                let selected = value == Some(v);
                let on = on.clone();
                Button::new(ElementId::NamedChild(std::sync::Arc::new(id.clone()), i.to_string().into()))
                    .ghost()
                    .rounded(px(6. * self.ui_scale))
                    .h(u(30.))
                    .px(u(14.))
                    .label(label)
                    .selected(selected)
                    .toggled(selected)
                    .when(!selected, |b| b.text_color(t.muted))
                    .when(selected, |b| b.font_weight(FontWeight::MEDIUM))
                    .on_click(cx.listener(move |this, _, w, cx| on(this, v, w, cx)))
            }))
    }

    /// An option in a grid of choices (effects, key palette, advanced modes): outlined at rest,
    /// filled with the primary colour when it's the current choice.
    fn choice(id: impl Into<ElementId>, selected: bool) -> Button {
        let b = Self::roomy(Button::new(id));
        if selected {
            b.primary().selected(true).toggled(true)
        } else {
            b.outline().toggled(false)
        }
    }

    fn toggle(
        &self,
        id: &'static str,
        on: bool,
        disabled: bool,
        change: impl Fn(&mut AppView, bool, &mut Window, &mut Cx) + 'static,
        cx: &mut Cx,
    ) -> Switch {
        Switch::new(id)
            .color(self.theme.switch_on)
            .checked(on)
            .disabled(disabled)
            .on_click(cx.listener(move |this, checked: &bool, w, cx| change(this, *checked, w, cx)))
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
        let editing = self
            .features
            .edit
            .as_ref()
            .filter(|e| e.field == Field::Slider(id))
            .map(|e| (e.text.clone(), e.fresh));
        let text = value.map(|v| format!("{v:.decimals$}")).unwrap_or_else(|| {
            if disabled {
                "—".into()
            } else {
                "mixed".into()
            }
        });
        // The − / + buttons are the keyboard path for every slider (Tab, then Space).
        let step_btn = |sign: f32, name: &'static str| {
            let label = if sign < 0.0 { "−" } else { "+" };
            Button::new((if sign < 0.0 { "minus" } else { "plus" }, id as usize))
                .ghost()
                .small()
                .w(u(28.))
                .h(u(28.))
                .label(label)
                .accessibility_label(name)
                .disabled(disabled || value.is_none())
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(v) = value {
                        let next = ((v + sign * d.step) / d.step).round() * d.step;
                        this.nudge(id, next.clamp(d.min, d.max), cx);
                    }
                }))
        };
        div()
            .flex()
            .flex_col()
            .gap(u(8.))
            .when(disabled, |el| el.opacity(0.45))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(u(6.))
                    .child(div().flex_1().font_weight(FontWeight::MEDIUM).child(label))
                    .child(step_btn(-1.0, "Decrease"))
                    .child(
                        // Click to type a value; Enter or clicking elsewhere applies it, Esc cancels.
                        div()
                            .id(("value", id as usize))
                            .w(u(96.))
                            .h(u(28.))
                            .flex()
                            .items_center()
                            .justify_end()
                            .px(u(8.))
                            .rounded(u(6.))
                            .bg(t.surface_2)
                            .border_1()
                            .border_color(if editing.is_some() { t.focus } else { t.border_strong })
                            .numeric()
                            .font_weight(FontWeight::MEDIUM)
                            .text_size(u(13.))
                            .when(!disabled && editing.is_none(), |el| {
                                el.hover(|s| s.border_color(t.muted))
                            })
                            .when(!disabled, |el| {
                                let start = value.map(|v| format!("{v:.decimals$}")).unwrap_or_default();
                                el.cursor_text().on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.begin_slider_edit(d, start.clone(), cx)
                                    }),
                                )
                            })
                            .map(|el| match &editing {
                                Some((typed, fresh)) => el
                                    .child(
                                        div()
                                            .when(*fresh, |s| s.bg(t.accent_soft).rounded(u(3.)))
                                            .child(typed.clone()),
                                    )
                                    .child(div().ml(u(1.)).w(u(1.5)).h(u(15.)).bg(t.accent))
                                    .when(!unit.is_empty(), |el| {
                                        el.child(div().ml(u(4.)).text_color(t.muted).child(unit))
                                    }),
                                None => el
                                    .when(value.is_none(), |el| el.text_color(t.muted))
                                    .child(text)
                                    .when(!unit.is_empty() && value.is_some(), |el| {
                                        el.child(div().ml(u(4.)).text_color(t.muted).child(unit))
                                    }),
                            }),
                    )
                    .child(step_btn(1.0, "Increase")),
            )
            .child({
                // The thumb grows a soft ring while the pointer is over the track, and a wider
                // one while it's being dragged. Shadows are in physical pixels, so scale them.
                let s = self.ui_scale;
                let dragging = self.drag.is_some_and(|g| g.id == id);
                let ring = |spread: f32, color: Hsla| BoxShadow {
                    color,
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(spread * s),
                    inset: false,
                };
                let lift = BoxShadow {
                    color: t.thumb_shadow,
                    offset: point(px(0.), px(1. * s)),
                    blur_radius: px(3. * s),
                    spread_radius: px(0.),
                    inset: false,
                };
                let group: SharedString = format!("slider-{}", id as usize).into();
                div()
                    .id(("track", id as usize))
                    .group(group.clone())
                    .relative()
                    .h(u(20.))
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
                            .h(u(4.))
                            .rounded_full()
                            .bg(t.track)
                            .when(value.is_some(), |el| {
                                el.child(div().h_full().w(relative(frac)).rounded_full().bg(fill))
                            }),
                    )
                    // No thumb while there's no single value (nothing or a mixed selection):
                    // a thumb at the left end would claim the value is the minimum.
                    .when(value.is_some(), |el| {
                        el.child(
                            div()
                                .absolute()
                                .left(relative(frac))
                                .ml(u(-8.))
                                .size(u(16.))
                                .rounded_full()
                                .bg(t.slider_thumb)
                                .border_1()
                                .border_color(fill)
                                .map(|el| {
                                    if dragging {
                                        el.shadow(vec![lift.clone(), ring(6., t.halo)])
                                    } else {
                                        el.shadow(vec![lift.clone()]).when(!disabled, |el| {
                                            el.group_hover(group.clone(), |st| {
                                                st.shadow(vec![lift.clone(), ring(4., t.halo)])
                                            })
                                        })
                                    }
                                }),
                        )
                    })
                    .when(!disabled, |el| {
                        el.cursor_pointer().on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                                this.begin_drag(d, e.position, cx)
                            }),
                        )
                    })
            })
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
            Field::Repeat => 2,
            Field::Delay(i) => 100 + i,
            Field::Slider(s) => 200 + s as usize,
        };
        div()
            .id(("field", id))
            .w(u(width))
            .h(u(32.))
            .px(u(10.))
            .flex()
            .items_center()
            .rounded(u(8.))
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
                    .text_size(u(13.))
                    .child(if empty && active.is_none() {
                        placeholder.to_string()
                    } else {
                        text
                    }),
            )
            .when(active.is_some(), |el| {
                el.child(div().ml(u(1.)).w(u(1.5)).h(u(16.)).bg(t.accent))
            })
    }

    /// The window's title bar: drawn over the top of the window with no fill or separator, so
    /// the sidebar and page colours run up to the top edge. GPUI Kit's `TitleBar` provides
    /// dragging and the Windows minimise / maximise / close buttons.
    fn title_bar(&self) -> Div {
        let t = self.theme;
        div().absolute().top_0().left_0().w_full().flex().flex_col().child(
            TitleBar::new()
                .bg(transparent_black())
                .border_color(transparent_black())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(u(8.))
                        .child(self.logo(16.))
                        .child(
                            div()
                                .text_size(u(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.muted)
                                .child(crate::APP_NAME),
                        ),
                ),
        )
    }

    /// The app logo: an isometric keyboard on a dark rounded tile (same design as the exe
    /// icon; both come from `assets/icon/make_logo.py`).
    fn logo(&self, size: f32) -> Div {
        let t = self.theme;
        div()
            .flex_none()
            .size(u(size))
            .rounded(u(size * 0.23))
            .border_1()
            .border_color(t.logo_border)
            .bg(linear_gradient(
                180.,
                linear_color_stop(t.logo_top, 0.),
                linear_color_stop(t.logo_bottom, 1.),
            ))
            .flex()
            .items_center()
            .justify_center()
            .child(svg().path("icons/logo.svg").size(u(size)).text_color(t.logo_mark))
    }

    fn banner(&self, text: impl Into<SharedString>, color: Hsla) -> Div {
        div()
            .p(u(10.))
            .rounded(u(8.))
            .border_1()
            .border_color(color.opacity(0.5))
            .text_color(color)
            .text_size(u(13.))
            .child(text.into())
    }

    fn small_button(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        kind: Btn,
        enabled: bool,
    ) -> Button {
        Self::roomy_sm(self.button(id, label, kind)).disabled(!enabled)
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
        div().flex().flex_col().gap(u(16.)).children(
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
                        .gap(u(8.))
                        .child(
                            div()
                                .text_size(u(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.muted)
                                .child(g.name),
                        )
                        .child(div().flex().flex_wrap().gap(u(5.)).children(
                            g.keys.into_iter().enumerate().map(|(ki, (label, action))| {
                                let on = Some(action) == current || highlight.contains(&action);
                                let on_pick = on_pick.clone();
                                Self::choice((id, gi * 100 + ki), on)
                                    .h(u(32.))
                                    .px(u(10.))
                                    .min_w(u(44.))
                                    .label(label)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        on_pick(this, action, cx)
                                    }))
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
            .gap(u(16.))
            .py(u(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(u(0.))
                    .gap(u(2.))
                    .child(div().font_weight(FontWeight::MEDIUM).child(label))
                    .child(div().text_color(t.muted).text_size(u(12.)).child(hint)),
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
            .gap(u(16.))
            .p(u(20.))
            .rounded(u(12.))
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
                                .numeric()
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

    // ---- shell ---------------------------------------------------------------------

    fn connect_screen(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let connecting = matches!(self.status, Status::Connecting | Status::Disconnecting);
        let error = if let Status::Error(e) = &self.status {
            Some(e.clone())
        } else {
            None
        };
        let hint = if self.auto_connect {
            "Plug in your MonsGeek or Akko Hall-effect keyboard with its USB cable and it connects by itself. Close the official driver first so the two apps don't fight over the device."
        } else {
            "Plug in your MonsGeek or Akko Hall-effect keyboard with its USB cable. Close the official driver first so the two apps don't fight over the device."
        };
        div().size_full().flex().items_center().justify_center().child(
            div()
                .w(u(420.))
                .flex()
                .flex_col()
                .items_center()
                .gap(u(16.))
                .p(u(32.))
                .rounded(u(14.))
                .bg(t.surface)
                .border_1()
                .border_color(t.border)
                .child(self.logo(56.))
                .child(div().text_size(u(20.)).font_weight(FontWeight::SEMIBOLD).child(crate::APP_NAME))
                .child(div().text_color(t.muted).text_center().child(hint))
                .children(error.map(|e| div().w_full().p(u(10.)).rounded(u(6.)).bg(t.danger_soft).text_color(t.danger).child(e)))
                .child(
                    div()
                        .flex()
                        .gap(u(8.))
                        .child(
                            self.button(
                                "connect",
                                match self.status {
                                    Status::Disconnecting => "Disconnecting…",
                                    Status::Connecting => "Connecting…",
                                    _ => "Connect keyboard",
                                },
                                Btn::Primary,
                            )
                            .loading(connecting)
                            .disabled(connecting)
                            .on_click(cx.listener(|this, _, _, cx| this.connect_by_user(false, cx))),
                        )
                        .child(
                            self.button("demo", "Try demo", Btn::Normal)
                                .disabled(connecting)
                                .on_click(cx.listener(|this, _, _, cx| this.connect_by_user(true, cx))),
                        ),
                ),
        )
    }

    fn shell(&mut self, window: &mut Window, cx: &mut Cx) -> Div {
        self.sync_pickers(window, cx);
        let t = self.theme;
        let Some(d) = self.device.as_ref() else {
            return div();
        };
        let (title, sub) = self.page.title();
        let profile = d.profile;
        let name = d.name.clone();
        let busy = self.busy > 0;
        let shortcuts: [&dyn Action; 7] = [
            &GoPerformance,
            &GoRemap,
            &GoAdvanced,
            &GoLighting,
            &GoTester,
            &GoAutoProfiles,
            &GoSettings,
        ];
        let sidebar = div()
            .w(u(232.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(u(4.))
            .px(u(12.))
            // Clear of the title bar drawn over the top of the window.
            .pt(TITLE_BAR_HEIGHT + px(8. * self.ui_scale))
            .pb(u(16.))
            .bg(t.sidebar)
            .border_r_1()
            .border_color(t.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(u(10.))
                    .px(u(6.))
                    .pt(u(4.))
                    .pb(u(14.))
                    .mb(u(6.))
                    .border_b_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .size(u(34.))
                            .flex_none()
                            .rounded(u(8.))
                            .bg(t.surface_2)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().path("icons/keyboard.svg").size(u(18.)).text_color(t.text_2)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w(u(0.))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(u(12.))
                                    .text_color(if self.demo { t.warn } else { t.ok })
                                    .child(if self.demo { "Demo mode" } else { "Connected over USB" }),
                            ),
                    ),
            )
            .children(Page::ALL.iter().zip(shortcuts).map(|(&p, shortcut)| {
                let active = self.page == p;
                Button::new(p.label())
                    .ghost()
                    .w_full()
                    .h(u(36.))
                    .px(u(10.))
                    .icon(
                        gpui::component::Icon::empty().path(match p {
                            Page::Performance => "icons/gauge.svg",
                            Page::Remap => "icons/remap.svg",
                            Page::Advanced => "icons/bolt.svg",
                            Page::Lighting => "icons/bulb.svg",
                            Page::Tester => "icons/pulse.svg",
                            Page::AutoProfiles => "icons/swap.svg",
                            Page::Settings => "icons/settings.svg",
                        }),
                    )
                    .label(p.label())
                    // Button content is centred; the spacer keeps labels on one leading edge.
                    .child(div().flex_1())
                    .selected(active)
                    .tooltip_with_action(p.label(), shortcut, Some(CONTEXT))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_page(p, cx)))
            }))
            .child(div().flex_1())
            .child(
                self.button("disconnect", if self.demo { "Exit demo" } else { "Disconnect" }, Btn::Normal)
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| this.disconnect_by_user(cx))),
            );

        let status = if self.closing {
            "Finishing saves…"
        } else if self.transitioning {
            "Loading profile…"
        } else if busy {
            "Saving…"
        } else {
            ""
        };
        let (undo, redo) = (self.history.can_undo(), self.history.can_redo());
        let history = div()
            .flex()
            .items_center()
            .gap(u(4.))
            .child(
                Self::roomy_sm(Button::new("undo"))
                    .ghost()
                    .label("Undo")
                    .disabled(undo.is_none() || self.transitioning)
                    .tooltip_with_action(
                        undo.map_or("Nothing to undo".to_string(), |l| format!("Undo {l}")),
                        &Undo,
                        Some(CONTEXT),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.undo(cx))),
            )
            .child(
                Self::roomy_sm(Button::new("redo"))
                    .ghost()
                    .label("Redo")
                    .disabled(redo.is_none() || self.transitioning)
                    .tooltip_with_action(
                        redo.map_or("Nothing to redo".to_string(), |l| format!("Redo {l}")),
                        &Redo,
                        Some(CONTEXT),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.redo(cx))),
            );
        // Narrow windows (or large interface sizes) stack the title above the controls.
        let stacked = self.kb_width.get() / self.ui_scale < 900.0;
        let header = div()
            .flex()
            .items_center()
            .when(stacked, |d| d.flex_col().items_start())
            .gap(u(16.))
            .px(u(28.))
            .pt(u(18.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(u(0.))
                    .gap(u(2.))
                    .child(
                        div()
                            .text_size(u(22.))
                            .line_height(relative(1.25))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(div().text_size(u(13.)).text_color(t.muted).child(sub)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(u(12.))
                    // A fixed lane, so the controls don't move as the status comes and goes.
                    // Stacked, the controls start the row instead.
                    .when(!stacked, |el| {
                        el.child(div().w(u(110.)).text_right().text_color(t.muted).text_size(u(12.)).child(status))
                    })
                    .child(history)
                    .when(stacked && !status.is_empty(), |el| {
                        el.child(div().text_color(t.muted).text_size(u(12.)).child(status))
                    })
                    .child(self.segmented(
                        "profile",
                        (0..crate::app::PROFILES).map(|p| (p, short_name(&self.profile_name(p)))).collect(),
                        Some(profile),
                        |this, p, _, cx| this.set_profile(p, cx),
                        cx,
                    )),
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
                .min_w(u(0.))
                .h_full()
                .flex()
                .flex_col()
                // Clear of the title bar and its window buttons.
                .pt(TITLE_BAR_HEIGHT)
                .child(header)
                .child(
                    div()
                        .id(("content", self.page as usize))
                        .relative()
                        .flex_1()
                        .overflow_y_scroll()
                        .track_scroll(&self.content_scroll.handle)
                        .px(u(28.))
                        .pt(u(16.))
                        .pb(u(28.))
                        .child(smooth_scroll::layer(&self.content_scroll))
                        .child(page.w_full()),
                ),
        )
    }

    // ---- pages ---------------------------------------------------------------------

    fn performance(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let Some(d) = self.device.as_ref() else {
            return div();
        };
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
            .gap(u(6.))
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
                .grid()
                .grid_cols(2)
                .gap(u(8.))
                .children(
                    PRESETS
                        .iter()
                        .enumerate()
                        .map(|(i, &(name, desc, act, rt))| {
                            Button::new(("preset", i))
                                .outline()
                                .h_auto()
                                .w_full()
                                .px(u(12.))
                                .py(u(8.))
                                .disabled(none)
                                .accessibility_label(format!("{name} preset: {desc}"))
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
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_start()
                                        .gap(u(2.))
                                        .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                                        .child(div().text_color(t.muted).text_size(u(12.)).child(desc)),
                                )
                                // Button content is centred; the spacer keeps the text on the leading edge.
                                .child(div().flex_1())
                        }),
                );

        let act_card = self.card(
            "Actuation point",
            Some("How far a key travels before it registers. Lower is faster; higher prevents accidental presses."),
            None,
            div()
                .flex()
                .flex_col()
                .gap(u(16.))
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
            );
        let rt_body = div()
            .flex()
            .flex_col()
            .gap(u(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(u(10.))
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
            .gap(u(16.))
            .child(self.keyboard_card(Some(toolbar.into_any_element()), board))
            .child(
                div()
                    .flex()
                    .gap(u(16.))
                    .items_start()
                    .when(self.narrow(), |d| d.flex_col())
                    .child(
                        div()
                            .flex_1()
                            .min_w(u(0.))
                            .when(self.narrow(), |d| d.w_full())
                            .child(act_card),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(u(0.))
                            .when(self.narrow(), |d| d.w_full())
                            .flex()
                            .flex_col()
                            .gap(u(16.))
                            .child(rt_card)
                            .child(dz_card),
                    ),
            )
    }

    fn remap(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let Some(d) = self.device.as_ref() else {
            return div();
        };
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
        let Some(d) = self.device.as_ref() else {
            return div();
        };
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
            .gap(u(10.))
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
                .p(u(18.))
                .rounded(u(10.))
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
                    .gap(u(6.))
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
                    .when(tab == RemapTab::Key, |el| el.children(self.search_input(150.)))
                    .child(
                        self.button("reset-key", "Reset to factory", Btn::Normal)
                            .disabled(reset_disabled)
                            .on_click({
                                let def = defaults[index];
                                cx.listener(move |this, _, _, cx| this.set_key_action(index, def, cx))
                            }),
                    )
                    .when(layer == 1, |el| {
                        let empty = current == [0, 0, 0, 0];
                        el.child(
                            self.button("clear-fn", "Clear", Btn::Ghost)
                                .disabled(empty)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_key_action(index, [0, 0, 0, 0], cx)
                                })),
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
            .gap(u(16.))
            .child(self.keyboard_card(Some(controls.into_any_element()), board))
            .when(layer == 1 && fn_layer != active_fn, |el| {
                el.child(div().p(u(10.)).rounded(u(8.)).border_1().border_color(t.warn.opacity(0.5)).text_color(t.warn).child(format!(
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
                    .gap(u(4.))
                    .children(shortcuts.iter().map(|&(i, a)| {
                        Button::new(("short", i))
                            .ghost()
                            .w_full()
                            .gap(u(12.))
                            .selected(target == Some(i))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selection = vec![i];
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(u(110.))
                                    .numeric()
                                    .text_size(u(12.))
                                    .child(format!("Fn + {}", action_label(&base_names[i]))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(t.text_2)
                                    .child(action_description(&a)),
                            )
                            .children((a != defaults[i]).then(|| {
                                div().text_size(u(11.)).text_color(t.muted).child("Changed")
                            }))
                    }));
            page = page.child(
                div()
                    .flex()
                    .gap(u(16.))
                    .items_start()
                    .when(self.narrow(), |d| d.flex_col())
                    .child(
                        div()
                            .flex_1()
                            .min_w(u(0.))
                            .when(self.narrow(), |d| d.w_full())
                            .child(editor),
                    )
                    .child(
                        div()
                            .w(u(360.))
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
        let Some(d) = self.device.as_ref() else {
            return div();
        };
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

        // As many equal columns as fit the card (design px: card padding 2 × 20, the settings
        // column 340 + gap 16 beside it when wide), so the grid fills its card edge to edge.
        let card_w = self.kb_width.get() / self.ui_scale - if self.narrow() { 0.0 } else { 356.0 } - 40.0;
        let cols = ((card_w + 6.0) / 156.0).floor().clamp(2.0, 5.0) as u16;
        let effects =
            div()
                .grid()
                .grid_cols(cols)
                .gap(u(6.))
                .children(EFFECTS.iter().map(|e| {
                    let active = e.id == l.effect;
                    let id = e.id;
                    Self::choice(("fx", e.id as usize), active)
                        .w_full()
                        .label(e.name)
                        .child(div().flex_1())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_lighting(|l| l.effect = id, cx)
                        }))
                }));

        let settings = div()
            .flex()
            .flex_col()
            .gap(u(16.))
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
                            .gap(u(8.))
                            .child(div().font_weight(FontWeight::MEDIUM).child("Colour"))
                            .child(div().flex().flex_wrap().items_center().gap(u(8.)).children(SWATCHES.iter().map(
                                |&hex| {
                                    let rgb3 = [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8];
                                    let active = l.color == rgb3;
                                    Button::new(("sw", hex as usize))
                                        .ghost()
                                        .compact()
                                        .selected(active)
                                        .accessibility_label(format!("Colour #{hex:06X}"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.edit_lighting(|l| l.color = rgb3, cx)
                                        }))
                                        .child(
                                            div()
                                                .size(u(22.))
                                                .rounded(u(6.))
                                                .bg(rgb(hex))
                                                .border_2()
                                                .border_color(if active { t.text } else { t.border_strong }),
                                        )
                                        .into_any_element()
                                },
                            ))
                            .children(self.pickers.as_ref().map(|p| {
                                self.color_picker(&p.effect, "Custom colour", l.color)
                            }))),
                    )
                })
            });

        div()
            .flex()
            .flex_col()
            .gap(u(16.))
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
                    .gap(u(16.))
                    .items_start()
                    .when(self.narrow(), |d| d.flex_col())
                    .child(
                        div()
                            .flex_1()
                            .min_w(u(0.))
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
                            .w(u(340.))
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
        let t = self.theme;
        let Some(d) = self.device.as_ref() else {
            return div();
        };
        let fw = d.info.firmware;
        let o = d.options;
        let info_row = |label: &'static str, v: String| {
            div()
                .flex()
                .justify_between()
                .py(u(5.))
                .child(div().text_color(t.muted).child(label))
                .child(div().numeric().text_size(u(12.5)).child(v))
        };
        let rate = d.report_rate;
        let polling = div()
            .flex()
            .flex_col()
            .gap(u(8.))
            .child(div().font_weight(FontWeight::MEDIUM).child("Polling rate"))
            .map(|el| match rate {
                Some(hz) => el
                    .child(
                        div()
                            .text_color(t.muted)
                            .text_size(u(12.))
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
                None => el.child(div().text_color(t.muted).text_size(u(12.)).child(format!(
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
                .gap(u(16.))
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
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(u(4.))
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
                        .child(div().text_color(t.muted).text_size(u(12.)).child(
                            "The firmware's filter against Rapid Trigger chatter: 0 is off, each step of 25 ignores a little more jitter at the cost of some sensitivity.",
                        )),
                )
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
        let reset = self.card(
            "Factory reset",
            Some("Restores every profile, keymap, lighting and actuation setting to factory defaults. This can't be undone; back up first if you might want them again."),
            None,
            div().child(
                self.button("reset", "Factory reset…", Btn::Danger)
                    .disabled(self.transitioning)
                    .on_click(cx.listener(|this, _, window, cx| this.confirm_factory_reset(window, cx))),
            ),
        );
        let profiles = self.profiles_card(cx);
        let app = self.app_card(window, cx);
        div()
            .flex()
            .gap(u(16.))
            .items_start()
            .when(self.narrow(), |d| d.flex_col())
            .child(
                div()
                    .flex_1()
                    .min_w(u(0.))
                    .when(self.narrow(), |d| d.w_full())
                    .flex()
                    .flex_col()
                    .gap(u(16.))
                    .child(keyboard)
                    .child(profiles),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(u(0.))
                    .when(self.narrow(), |d| d.w_full())
                    .flex()
                    .flex_col()
                    .gap(u(16.))
                    .child(device)
                    .child(app)
                    .child(self.backup_card(cx))
                    .child(reset),
            )
    }
}

/// Blends `b` over `a` by `t` (0..1), in RGB.
/// A profile name short enough for the header's segmented control.
fn short_name(name: &str) -> String {
    const MAX: usize = 14;
    if name.chars().count() <= MAX {
        name.to_string()
    } else {
        name.chars().take(MAX - 1).collect::<String>() + "…"
    }
}

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

impl AppView {
    /// A colour swatch with its hex code that opens GPUI Kit's picker: palette, HSLA sliders
    /// and a hex box you can type into. The app's swatches come first in its palette.
    fn color_picker(
        &self,
        state: &Entity<gpui::component::color_picker::ColorPickerState>,
        label: &'static str,
        current: [u8; 3],
    ) -> AnyElement {
        use gpui::component::color_picker::ColorPicker;
        let t = self.theme;
        let featured = SWATCHES
            .iter()
            .map(|&h| crate::app::pickers_to_hsla([(h >> 16) as u8, (h >> 8) as u8, h as u8]))
            .collect();
        // Our own hex label: the kit's field truncates channels (#3B9BFF would read #3A9BFF).
        let hex = format!("#{:02X}{:02X}{:02X}  ▾", current[0], current[1], current[2]);
        div()
            .flex()
            .items_center()
            .h(u(32.))
            .pl(u(4.))
            .pr(u(10.))
            .rounded(u(8.))
            .bg(t.surface_2)
            .border_1()
            .border_color(t.border_strong)
            .text_size(u(12.5))
            .child(
                ColorPicker::new(state)
                    .featured_colors(featured)
                    .accessibility_label(label)
                    .label(hex),
            )
            .into_any_element()
    }
}

// ---- settings: app, profiles, confirmations ---------------------------------------------------

impl AppView {
    /// The key search box (kit input), shared by Remap and Advanced.
    fn search_input(&self, width: f32) -> Option<AnyElement> {
        let p = self.pickers.as_ref()?;
        Some(
            div()
                .w(u(width))
                .child(Styled::h(gpui::component::input::Input::new(&p.search), u(34.)).cleanable(true))
                .into_any_element(),
        )
    }

    /// Opening at sign-in, the tray, appearance and interface size.
    fn app_card(&mut self, _window: &mut Window, cx: &mut Cx) -> Div {
        use crate::config::{Appearance, UI_SIZES};
        let t = self.theme;
        let at_login = self.tools.launch_at_login;
        let in_tray = self.config.run_in_tray;
        let size = self.config.font_size();
        self.card(
            "App",
            None,
            None,
            div()
                .flex()
                .flex_col()
                .gap(u(16.))
                .child(self.setting_row(
                    "Open at sign-in",
                    "Start Monsgeek App when you sign in to Windows, so automatic profiles and per-layer lighting keep working.",
                    self.toggle(
                        "at-login",
                        at_login,
                        false,
                        |this, v, _, cx| this.set_launch_at_login(v, cx),
                        cx,
                    ),
                ))
                .child(self.setting_row(
                    "Keep running in the tray",
                    "Closing the window hides Monsgeek App to the notification area, so automatic profiles and per-layer lighting keep working. Opening at sign-in starts it there.",
                    self.toggle(
                        "in-tray",
                        in_tray,
                        false,
                        |this, v, _, cx| {
                            this.config.run_in_tray = v;
                            this.save_config(cx);
                            this.sync_auto(cx);
                            cx.notify();
                        },
                        cx,
                    ),
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(u(8.))
                        .child(div().font_weight(FontWeight::MEDIUM).child("Appearance"))
                        .child(div().flex().child(self.segmented(
                            "appearance",
                            vec![
                                (Appearance::System, "System".into()),
                                (Appearance::Light, "Light".into()),
                                (Appearance::Dark, "Dark".into()),
                            ],
                            Some(self.config.appearance),
                            |this, a, w, cx| this.set_appearance(a, w, cx),
                            cx,
                        ))),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(u(8.))
                        .child(div().font_weight(FontWeight::MEDIUM).child("Interface size"))
                        .child(div().text_color(t.muted).text_size(u(12.)).child("Ctrl + and Ctrl − also change it; Ctrl 0 resets."))
                        .child(div().flex().child(self.segmented(
                            "ui-size",
                            UI_SIZES
                                .iter()
                                .zip(["Small", "Default", "Large", "Larger"])
                                .map(|(&s, name)| (s as u32, name.to_string()))
                                .collect(),
                            Some(size as u32),
                            |this, s, w, cx| this.set_ui_size(s as f32, w, cx),
                            cx,
                        ))),
                ),
        )
    }

    /// Names, copy, export and import for the four onboard profiles.
    fn profiles_card(&self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let Some(d) = self.device.as_ref() else {
            return div();
        };
        let active = d.profile;
        let busy = self.transitioning;
        let rows = (0..crate::app::PROFILES).map(|p| {
            let name_input = self.pickers.as_ref().map(|k| {
                Styled::h(gpui::component::input::Input::new(&k.profile_names[p as usize]), u(30.))
                    .aria_label(format!("Name of profile {}", p + 1))
            });
            let names: Vec<(u8, String)> = (0..crate::app::PROFILES)
                .filter(|&q| q != p)
                .map(|q| (q, self.profile_name(q)))
                .collect();
            let view = cx.entity().downgrade();
            let from_name = self.profile_name(p);
            div()
                .flex()
                .items_center()
                .gap(u(8.))
                .child(
                    div()
                        .w(u(20.))
                        .flex_none()
                        .numeric()
                        .text_color(if p == active { t.text } else { t.muted })
                        .child(format!("{}", p + 1)),
                )
                .child(div().flex_1().min_w(u(0.)).children(name_input))
                .child(
                    Self::roomy_sm(Button::new(("copy-profile", p as usize)))
                        .label("Copy to…")
                        .dropdown_caret(true)
                        .disabled(busy)
                        .dropdown_menu(move |menu, _, _| {
                            names.iter().fold(menu, |menu, (q, name)| {
                                let (view, q, from_name, name) = (view.clone(), *q, from_name.clone(), name.clone());
                                menu.item(gpui::component::menu::PopupMenuItem::new(name.clone()).on_click(
                                    move |_, window, cx| {
                                        let view = view.clone();
                                        let (from_name, name) = (from_name.clone(), name.clone());
                                        window.open_alert_dialog(cx, move |dialog, _, _| {
                                            let view = view.clone();
                                            dialog
                                                .confirm()
                                                .title(format!("Replace {name} with a copy of {from_name}?"))
                                                .description(format!(
                                                    "{name}'s keymap, key settings and lighting are overwritten. Back up first if you might want them again."
                                                ))
                                                .ok_text("Replace").ok_variant(gpui::component::button::ButtonVariant::Danger)
                                                .on_ok(move |_, _, cx| {
                                                    view.update(cx, |this, cx| this.copy_profile(p, q, cx)).ok();
                                                    true
                                                })
                                        });
                                    },
                                ))
                            })
                        }),
                )
                .child(
                    Self::roomy_sm(Button::new(("export-profile", p as usize)))
                        .label("Export…")
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| this.export_profile(p, cx))),
                )
                .child(
                    Self::roomy_sm(Button::new(("import-profile", p as usize)))
                        .label("Import…")
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| this.pick_profile_file(p, cx))),
                )
        });
        self.card(
            "Profiles",
            Some("Names are kept in this app. Copy, export and import move a profile's keymap, key settings and lighting. Macros and Fn layers are shared by all profiles, so they stay as they are."),
            None,
            div().flex().flex_col().gap(u(8.)).children(rows),
        )
    }

    fn set_launch_at_login(&mut self, on: bool, cx: &mut Cx) {
        match crate::win::set_launch_at_login(on) {
            Ok(()) => self.tools.launch_at_login = on,
            Err(e) => self.toast(format!("Couldn't change the sign-in setting: {e}"), true, cx),
        }
        cx.notify();
    }

    fn confirm_factory_reset(&mut self, window: &mut Window, cx: &mut Cx) {
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            dialog
                .confirm()
                .title("Reset the keyboard to factory settings?")
                .description("All four profiles, keymaps, Fn layers, macros, lighting and actuation settings go back to their defaults. This can't be undone.")
                .ok_text("Reset keyboard").ok_variant(gpui::component::button::ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    view.update(cx, |this, cx| this.factory_reset(cx)).ok();
                    true
                })
        });
    }

    fn confirm_import(&mut self, profile: u8, file: crate::backup::ProfileFile, window: &mut Window, cx: &mut Cx) {
        let view = cx.entity().downgrade();
        let target = self.profile_name(profile);
        let file = std::rc::Rc::new(file);
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let (view, file) = (view.clone(), file.clone());
            dialog
                .confirm()
                .title(format!("Replace {target} with {}?", file.name))
                .description(format!(
                    "{target}'s keymap, key settings and lighting are overwritten with the ones in the file. Back up first if you might want them again."
                ))
                .ok_text("Replace").ok_variant(gpui::component::button::ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    let file = (*file).clone();
                    view.update(cx, |this, cx| this.import_profile(profile, file, cx)).ok();
                    true
                })
        });
    }
}
