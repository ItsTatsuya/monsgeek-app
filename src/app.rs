//! App state and everything that talks to the keyboard. Rendering lives in ui/.

mod features;
mod tools;
pub use features::*;
pub use tools::*;

use crate::config::Config;
use crate::devices::{self, DeviceDef, Layout};
use crate::protocol::hid::{HidTransport, TravelStream};
use crate::protocol::mock::{self, MockTransport};
use crate::protocol::worker::KeyboardWorker;
use crate::protocol::macros::Macro;
use crate::protocol::{
    fn_system, ColorModes, DeviceInfo, KbOptions, KeyAction, KeyHe, KeyboardClient, Lighting,
    VendorEvent,
};
use crate::theme::Theme;
use anyhow::{anyhow, Result};
use gpui::{Bounds, Context, FocusHandle, Pixels, Point};
use hidapi::HidApi;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a key resting below its actuation point may go without a report before it reads as released.
const STALE_RELEASE: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Performance,
    Remap,
    Advanced,
    Lighting,
    Tester,
    AutoProfiles,
    Settings,
}

impl Page {
    pub fn parse(s: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| {
            let label = p.label();
            label.eq_ignore_ascii_case(s)
                || label.split(' ').next().is_some_and(|w| w.eq_ignore_ascii_case(s))
        })
    }

    pub const ALL: [Page; 7] = [
        Page::Performance,
        Page::Remap,
        Page::Advanced,
        Page::Lighting,
        Page::Tester,
        Page::AutoProfiles,
        Page::Settings,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Page::Performance => "Performance",
            Page::Remap => "Remap",
            Page::Advanced => "Advanced keys",
            Page::Lighting => "Lighting",
            Page::Tester => "Tester",
            Page::AutoProfiles => "Auto profiles",
            Page::Settings => "Settings",
        }
    }
    pub fn title(self) -> (&'static str, &'static str) {
        match self {
            Page::Performance => (
                "Actuation & Rapid Trigger",
                "How far each key travels before it fires.",
            ),
            Page::Remap => (
                "Remap",
                "Change what any key sends, on the base or Fn layer.",
            ),
            Page::Advanced => (
                "Advanced keys",
                "Dynamic keystrokes, mod-tap, Snap Tap and toggle keys, handled by the keyboard itself.",
            ),
            Page::Lighting => ("Lighting", "Effects, per-key colour and brightness."),
            Page::Tester => (
                "Tester",
                "Watch live switch travel, measure the real polling rate and calibrate the switches.",
            ),
            Page::AutoProfiles => (
                "Automatic profiles",
                "Switch profiles when a game comes to the front.",
            ),
            Page::Settings => (
                "Settings",
                "Polling rate, keyboard options, backup and device info.",
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SliderId {
    Actuation,
    Release,
    RtBoth,
    RtPress,
    RtRelease,
    Deadzone,
    Brightness,
    Speed,
    Debounce,
    Stability,
    DksTravel,
    MtTime,
}

#[derive(Clone, Copy)]
pub struct SliderDrag {
    pub id: SliderId,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

pub enum Status {
    Idle,
    Connecting,
    Disconnecting,
    Error(String),
}

pub struct Device {
    pub client: Arc<KeyboardWorker>,
    /// Kept for as long as the devices it opened.
    #[allow(dead_code)]
    pub api: Option<Arc<Mutex<HidApi>>>,
    /// The notify collection, open for the whole connection: travel stream and on-board changes.
    pub notify: Option<TravelStream>,
    pub notify_error: Option<String>,
    pub def: DeviceDef,
    pub info: DeviceInfo,
    pub layout: Layout,
    pub defaults: Vec<KeyAction>,
    pub fn_defaults: Vec<KeyAction>,
    pub name: String,
    pub profile: u8,
    pub options: KbOptions,
    pub debounce: u8,
    /// None when the firmware fixes the polling rate.
    pub report_rate: Option<u32>,
    /// Macros read from the keyboard, by macro number.
    pub macros: HashMap<u8, Macro>,
    pub keys: Vec<KeyHe>,
    /// What the keyboard holds; edits are diffed against it.
    pub committed: Vec<KeyHe>,
    pub keymap: Vec<KeyAction>,
    pub fnmap: Vec<KeyAction>,
    /// Fn layer shown in Remap (the keyboard's active one is options.fn_index).
    pub fn_layer: u8,
    pub lighting: Lighting,
    pub committed_lighting: Lighting,
}

pub struct Toast {
    pub id: u64,
    pub text: String,
    pub error: bool,
}

pub struct AppView {
    pub config: Config,
    pub theme: Theme,
    pub page: Page,
    pub status: Status,
    pub device: Option<Device>,
    pub demo: bool,
    pub selection: Vec<usize>,
    /// 0 = base layer, 1 = Fn layer.
    pub remap_layer: u8,
    pub linked_rt: bool,
    pub drag: Option<SliderDrag>,
    /// Painting a selection by dragging across keys: true adds, false removes.
    pub paint: Option<bool>,
    pub slider_bounds: Rc<RefCell<HashMap<SliderId, Bounds<Pixels>>>>,
    pub kb_width: Rc<Cell<f32>>,
    pub toasts: Vec<Toast>,
    pub busy: usize,
    pub confirm_reset: bool,
    pub travel: Vec<f32>,
    pub peaks: Vec<f32>,
    /// Page to open once the keyboard is loaded (the --page option).
    pub start_page: Option<Page>,
    pub transitioning: bool,
    pub closing: bool,
    pub features: Features,
    pub tools: Tools,
    pub focus: FocusHandle,
    draining: Option<Arc<KeyboardWorker>>,
    connection_gen: u64,
    /// The keyboard's travel stream is switched on (tester open).
    streaming: bool,
    stream_gen: u64,
    key_gen: u64,
    light_gen: u64,
    toast_id: u64,
    /// Lighting preview: animation clock, recent key presses, and whether its redraw loop runs.
    pub light_clock: std::time::Instant,
    pub light_presses: Vec<crate::light_preview::Press>,
    light_ticking: bool,
}

impl AppView {
    pub fn new(config: Config, cx: &mut Context<Self>) -> Self {
        Self {
            features: Features::default(),
            tools: Tools::new(&config),
            focus: cx.focus_handle(),
            theme: Theme::new(),
            config,
            page: Page::Performance,
            status: Status::Idle,
            device: None,
            demo: false,
            selection: Vec::new(),
            remap_layer: 0,
            linked_rt: true,
            drag: None,
            paint: None,
            slider_bounds: Rc::new(RefCell::new(HashMap::new())),
            kb_width: Rc::new(Cell::new(900.0)),
            toasts: Vec::new(),
            busy: 0,
            confirm_reset: false,
            travel: vec![0.0; crate::protocol::MATRIX_KEYS],
            peaks: vec![0.0; crate::protocol::MATRIX_KEYS],
            start_page: None,
            transitioning: false,
            closing: false,
            draining: None,
            connection_gen: 0,
            streaming: false,
            stream_gen: 0,
            key_gen: 0,
            light_gen: 0,
            toast_id: 0,
            light_clock: std::time::Instant::now(),
            light_presses: Vec::new(),
            light_ticking: false,
        }
    }

    // ---- lighting preview ------------------------------------------------------

    /// Seconds on the lighting preview clock.
    pub fn light_time(&self) -> f32 {
        self.light_clock.elapsed().as_secs_f32()
    }

    fn light_previewing(&self) -> bool {
        self.page == Page::Lighting
            && self.features.lighting_tab == LightingTab::Effects
            && self
                .device
                .as_ref()
                .is_some_and(|d| crate::light_preview::animated(d.lighting.effect))
    }

    /// Redraws the animated lighting preview at ~30 fps while it's on screen.
    pub fn tick_light_preview(&mut self, cx: &mut Context<Self>) {
        if self.light_ticking || !self.light_previewing() {
            return;
        }
        self.light_ticking = true;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(33))
                .await;
            let alive = this
                .update(cx, |s, cx| {
                    if !s.light_previewing() {
                        s.light_ticking = false;
                        return false;
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !alive {
                break;
            }
        })
        .detach();
    }

    /// A real key press, for the reactive effects in the lighting preview.
    pub fn light_press(&mut self, hid: u8) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let Some(k) = d
            .layout
            .keys
            .iter()
            .find(|k| k.base[0] == 0 && k.base[1] == 0 && k.base[3] == 0 && k.base[2] == hid)
        else {
            return;
        };
        let press = crate::light_preview::Press {
            index: k.index,
            x: (k.x + k.w / 2.0) / d.layout.width,
            y: (k.y + k.h / 2.0) / d.layout.height,
            t: self.light_time(),
        };
        if self.light_presses.len() >= 20 {
            self.light_presses.remove(0);
        }
        self.light_presses.push(press);
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        self.toast_id += 1;
        let id = self.toast_id;
        self.toasts.push(Toast {
            id,
            text: text.into(),
            error,
        });
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            this.update(cx, |s, cx| {
                s.toasts.retain(|t| t.id != id);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Run a blocking keyboard job off the UI thread, then apply its result.
    pub(crate) fn run<T: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        label: &'static str,
        job: impl FnOnce(&mut KeyboardClient) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        let Some(device) = self.device.as_ref() else {
            return;
        };
        let result = device.client.submit(job);
        let connection = self.connection_gen;
        self.busy += 1;
        cx.spawn(async move |this, cx| {
            let r = result
                .recv()
                .await
                .unwrap_or_else(|_| Err(anyhow!("Keyboard connection closed")));
            this.update(cx, |s, cx| {
                if s.connection_gen != connection {
                    return;
                }
                s.busy -= 1;
                match r {
                    Ok(v) => done(s, v, cx),
                    Err(e) => {
                        if label == "Switching profile" || label == "Factory reset" {
                            // The active profile may now be unknown. Require a fresh
                            // connection rather than writing with stale UI settings
                            // (transitioning, so disconnect drops pending edits).
                            s.disconnect(cx);
                            s.status = Status::Error(format!(
                                "{label} failed: {e:#}. Reconnect to read the keyboard again."
                            ));
                        } else {
                            s.toast(format!("{label} failed: {e:#}"), true, cx);
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    // ---- connection ------------------------------------------------------------

    pub fn connect(&mut self, demo: bool, cx: &mut Context<Self>) {
        if matches!(self.status, Status::Connecting | Status::Disconnecting)
            || self.device.is_some()
        {
            return;
        }
        self.connection_gen += 1;
        let connection = self.connection_gen;
        self.status = Status::Connecting;
        self.demo = demo;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_executor()
                .spawn(async move { load(demo) })
                .await;
            this.update(cx, |s, cx| {
                if s.connection_gen != connection {
                    return;
                }
                match r {
                    Ok((d, events)) => {
                        s.device = Some(d);
                        s.watch_keyboard(events, cx);
                        s.after_connect(cx);
                        s.status = Status::Idle;
                        s.selection.clear();
                        s.page = Page::Performance;
                        if let Some(p) = s.start_page.take() {
                            s.set_page(p, cx);
                        }
                    }
                    Err(e) => s.status = Status::Error(format!("{e:#}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn disconnect(&mut self, cx: &mut Context<Self>) {
        self.end_calibration(cx);
        self.stop_polling_test(cx);
        self.tools.engine.reset();
        if !self.transitioning {
            self.end_drag(cx);
            self.flush_keys(cx);
            self.flush_lighting(cx);
        }
        self.stop_stream(cx);
        self.draining = self.device.as_ref().map(|d| d.client.clone());
        let barrier = self.device.as_ref().map(|d| d.client.submit(|_| Ok(())));
        self.connection_gen += 1;
        let connection = self.connection_gen;
        self.key_gen += 1;
        self.light_gen += 1;
        self.busy = 0;
        self.transitioning = false;
        self.device = None;
        self.demo = false;
        self.selection.clear();
        self.features = Features::default();
        self.travel.fill(0.0);
        self.peaks.fill(0.0);
        self.status = Status::Disconnecting;
        cx.spawn(async move |this, cx| {
            if let Some(done) = barrier {
                let _ = done.recv().await;
            }
            this.update(cx, |s, cx| {
                if s.connection_gen == connection {
                    s.draining = None;
                    // Keep an error set by a failed profile switch or factory reset.
                    if matches!(s.status, Status::Disconnecting) {
                        s.status = Status::Idle;
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page == page {
            return;
        }
        self.end_drag(cx);
        if self.page == Page::Tester && page != Page::Tester {
            self.stop_stream(cx);
            self.end_calibration(cx);
            self.stop_polling_test(cx);
        }
        if self.recording() {
            self.toggle_recording(cx);
        }
        self.commit_edit(cx);
        self.page = page;
        self.selection.clear();
        self.confirm_reset = false;
        self.features.adv = None;
        self.features.switch_menu = false;
        if page == Page::Tester && self.features.tester_tab == TesterTab::Switch {
            self.start_stream(cx);
        }
        cx.notify();
    }

    pub fn set_profile(&mut self, p: u8, cx: &mut Context<Self>) {
        if self.transitioning || self.device.as_ref().is_none_or(|d| d.profile == p) {
            return;
        }
        self.end_drag(cx);
        self.flush_keys(cx);
        self.flush_lighting(cx);
        self.key_gen += 1;
        self.light_gen += 1;
        self.transitioning = true;
        self.features.adv = None;
        self.features.macro_draft = None;
        self.selection.clear();
        self.run(
            cx,
            "Switching profile",
            move |kb| {
                // Reload even if switching fails: a partial USB failure must not leave
                // the UI showing the previous profile while edits target another one.
                let switched = kb.set_profile(p);
                Ok((reload(kb)?, switched.map_err(|e| format!("{e:#}"))))
            },
            move |s, (fresh, switched), cx| {
                s.transitioning = false;
                if let Some(d) = s.device.as_mut() {
                    fresh.apply(d);
                }
                s.peaks.fill(0.0);
                s.schedule_layer_lighting(cx);
                if let Err(e) = switched {
                    s.toast(format!("Switching profile failed: {e}"), true, cx);
                }
            },
        );
    }

    // ---- selection ---------------------------------------------------------------

    pub fn press_key(&mut self, index: usize, single: bool, cx: &mut Context<Self>) {
        if single {
            self.selection = if self.selection == [index] {
                vec![]
            } else {
                vec![index]
            };
        } else {
            let add = !self.selection.contains(&index);
            self.paint = Some(add);
            self.paint_key(index, cx);
        }
        cx.notify();
    }

    pub fn paint_key(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.paint {
            Some(true) if !self.selection.contains(&index) => self.selection.push(index),
            Some(false) => self.selection.retain(|&i| i != index),
            _ => return,
        }
        cx.notify();
    }

    pub fn select_group(&mut self, f: impl Fn(&KeyAction) -> bool, cx: &mut Context<Self>) {
        if let Some(d) = &self.device {
            self.selection = d
                .layout
                .keys
                .iter()
                .filter(|k| f(&k.base))
                .map(|k| k.index)
                .collect();
        }
        cx.notify();
    }

    // ---- key settings ------------------------------------------------------------

    /// Edit the selected keys locally, then write the difference after a short pause.
    pub fn update_keys(&mut self, f: impl Fn(&mut KeyHe), cx: &mut Context<Self>) {
        let targets = self.edit_targets();
        self.update_indices(targets, f, cx);
    }

    pub fn update_indices(
        &mut self,
        indices: Vec<usize>,
        f: impl Fn(&mut KeyHe),
        cx: &mut Context<Self>,
    ) {
        if self.transitioning {
            return;
        }
        let Some(d) = self.device.as_mut() else {
            return;
        };
        for &i in &indices {
            if let Some(k) = d.keys.get_mut(i) {
                f(k);
            }
        }
        self.key_gen += 1;
        let gen = self.key_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(350))
                .await;
            this.update(cx, |s, cx| {
                if s.key_gen == gen && s.drag.is_none() {
                    s.flush_keys(cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn flush_keys(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let (prev, next) = (d.committed.clone(), d.keys.clone());
        let profile = d.profile;
        if prev == next && self.busy == 0 {
            return;
        }
        self.key_gen += 1;
        self.run(
            cx,
            "Saving key settings",
            move |kb| kb.apply_key_settings(&prev, &next).map(|_| next),
            move |s, next, _| {
                if let Some(d) = s.device.as_mut() {
                    if d.profile == profile {
                        d.committed = next;
                    }
                }
            },
        );
    }

    // ---- sliders -----------------------------------------------------------------

    pub fn begin_drag(&mut self, drag: SliderDrag, pos: Point<Pixels>, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        self.drag = Some(drag);
        self.drag_to(pos, cx);
    }

    pub fn drag_to(&mut self, pos: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(d) = self.drag else { return };
        let Some(b) = self.slider_bounds.borrow().get(&d.id).copied() else {
            return;
        };
        let t = ((pos.x - b.origin.x) / b.size.width).clamp(0.0, 1.0);
        let v = ((d.min + t * (d.max - d.min)) / d.step).round() * d.step;
        self.set_slider(d.id, v.clamp(d.min, d.max), cx);
    }

    pub fn end_drag(&mut self, cx: &mut Context<Self>) {
        self.paint = None;
        let Some(d) = self.drag.take() else { return };
        match d.id {
            SliderId::Debounce => {
                let ms = self.device.as_ref().map_or(0, |d| d.debounce);
                self.run(
                    cx,
                    "Setting debounce",
                    move |kb| kb.set_debounce(ms),
                    |_, _, _| {},
                );
            }
            SliderId::Stability => self.write_options(cx),
            SliderId::DksTravel | SliderId::MtTime => {}
            SliderId::Brightness | SliderId::Speed => self.flush_lighting(cx),
            _ => self.flush_keys(cx),
        }
        cx.notify();
    }

    /// Step a slider by one notch (the − / + buttons), writing immediately.
    pub fn nudge(&mut self, id: SliderId, value: f32, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        self.set_slider(id, value, cx);
        self.drag = Some(SliderDrag {
            id,
            min: 0.0,
            max: 0.0,
            step: 1.0,
        });
        self.end_drag(cx);
    }

    pub fn set_slider(&mut self, id: SliderId, v: f32, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        match id {
            SliderId::Actuation => self.update_keys(
                move |k| {
                    // Moving the actuation point also moves a matching release point, so non-RT keys stay symmetric.
                    if (k.release - k.actuation).abs() < 1e-4 {
                        k.release = v;
                    }
                    k.actuation = v;
                },
                cx,
            ),
            SliderId::Release => self.update_keys(move |k| k.release = v, cx),
            SliderId::RtBoth => self.update_keys(
                move |k| {
                    k.rt_press = v;
                    k.rt_release = v;
                },
                cx,
            ),
            SliderId::RtPress => self.update_keys(move |k| k.rt_press = v, cx),
            SliderId::RtRelease => self.update_keys(move |k| k.rt_release = v, cx),
            SliderId::Deadzone => self.update_keys(move |k| k.deadzone = v, cx),
            SliderId::Brightness => self.edit_lighting(|l| l.brightness = v as u8, cx),
            SliderId::Speed => self.edit_lighting(|l| l.speed = v as u8, cx),
            SliderId::Debounce => {
                if let Some(d) = self.device.as_mut() {
                    d.debounce = v as u8;
                }
            }
            SliderId::Stability => {
                if let Some(d) = self.device.as_mut() {
                    d.options.rt_stability = v as u8;
                }
            }
            SliderId::DksTravel => {
                if let Some(AdvEditor::Dks { cfg: Some(c), .. }) = self.features.adv.as_mut() {
                    c.travel = v;
                }
            }
            SliderId::MtTime => {
                if let Some(AdvEditor::ModTap { cfg: Some(c), .. }) = self.features.adv.as_mut() {
                    c.time = v as u16;
                }
            }
        }
        cx.notify();
    }

    // ---- remap -------------------------------------------------------------------

    pub fn set_key_action(&mut self, index: usize, action: KeyAction, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        let Some(d) = self.device.as_mut() else {
            return;
        };
        let (layer, profile, fn_layer, system) = (
            self.remap_layer,
            d.profile,
            d.fn_layer,
            fn_system(d.options.os),
        );
        if layer == 0 {
            d.keymap[index] = action;
        } else {
            d.fnmap[index] = action;
        }
        self.run(
            cx,
            "Remapping key",
            move |kb| {
                if layer == 0 {
                    kb.set_key(profile, 0, index as u8, action)
                } else {
                    kb.set_fn_key(fn_layer, index as u8, action, system)
                }
            },
            |_, _, _| {},
        );
    }

    pub fn set_remap_layer(&mut self, layer: u8, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        if self.recording() {
            self.toggle_recording(cx);
        }
        self.remap_layer = layer;
        self.selection.clear();
        self.features.macro_draft = None;
        if layer == 1 {
            // Fn + Right Alt changes the active Fn layer on the keyboard, so re-read it.
            self.run(
                cx,
                "Reading Fn layer",
                move |kb| {
                    let o = kb.get_options()?;
                    Ok((o, kb.get_fn_matrix(o.fn_index, fn_system(o.os))?))
                },
                |s, (o, map), cx| {
                    if let Some(d) = s.device.as_mut() {
                        d.options = o;
                    }
                    s.show_fn_layer(o.fn_index, map, cx);
                    s.schedule_layer_lighting(cx);
                },
            );
        }
        cx.notify();
    }

    pub fn set_fn_layer(&mut self, layer: u8, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        let system = self.device.as_ref().map_or(0, |d| fn_system(d.options.os));
        self.run(
            cx,
            "Reading Fn layer",
            move |kb| kb.get_fn_matrix(layer, system),
            move |s, map, cx| s.show_fn_layer(layer, map, cx),
        );
    }

    /// Show another Fn layer in Remap. A macro draft opened on the old layer would save to
    /// the new one, so it's dropped along with the selection.
    fn show_fn_layer(&mut self, layer: u8, map: Vec<KeyAction>, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_mut() else {
            return;
        };
        let changed = d.fn_layer != layer;
        d.fn_layer = layer;
        d.fnmap = map;
        if !changed {
            return;
        }
        if self.features.macro_draft.as_ref().is_some_and(|m| m.layer == 1) {
            if self.recording() {
                self.toggle_recording(cx);
            }
            self.features.macro_draft = None;
        }
        if self.page == Page::Remap && self.remap_layer == 1 {
            self.selection.clear();
        }
        cx.notify();
    }

    // ---- lighting ----------------------------------------------------------------

    pub fn edit_lighting(&mut self, f: impl FnOnce(&mut Lighting), cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        let Some(d) = self.device.as_mut() else {
            return;
        };
        f(&mut d.lighting);
        self.light_gen += 1;
        let gen = self.light_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            this.update(cx, |s, cx| {
                if s.light_gen == gen && s.drag.is_none() {
                    s.flush_lighting(cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn flush_lighting(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let l = d.lighting;
        if l == d.committed_lighting && self.busy == 0 {
            return;
        }
        let profile = d.profile;
        self.light_gen += 1;
        self.run(
            cx,
            "Saving lighting",
            move |kb| kb.set_lighting(&l),
            move |s, _, _| {
                if let Some(d) = s.device.as_mut() {
                    if d.profile == profile {
                        d.committed_lighting = l;
                    }
                }
            },
        );
    }

    // ---- settings ----------------------------------------------------------------

    pub fn edit_options(&mut self, f: impl FnOnce(&mut KbOptions), cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        let Some(d) = self.device.as_mut() else {
            return;
        };
        f(&mut d.options);
        self.write_options(cx);
    }

    fn write_options(&mut self, cx: &mut Context<Self>) {
        let Some(o) = self.device.as_ref().map(|d| d.options) else {
            return;
        };
        self.run(
            cx,
            "Saving options",
            move |kb| kb.set_options(&o),
            |_, _, _| {},
        );
    }

    pub fn factory_reset(&mut self, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        self.end_drag(cx);
        self.key_gen += 1;
        self.light_gen += 1;
        self.transitioning = true;
        self.confirm_reset = false;
        self.run(
            cx,
            "Factory reset",
            |kb| {
                kb.factory_reset()?;
                let fresh = reload(kb)?;
                // The reset also puts the polling rate back to its default.
                let rate = if kb.info.supports_report_rate { kb.get_report_rate().ok() } else { None };
                Ok((fresh, rate))
            },
            |s, (fresh, rate), cx| {
                s.transitioning = false;
                if let Some(d) = s.device.as_mut() {
                    fresh.apply(d);
                    d.macros.clear();
                    if rate.is_some() {
                        d.report_rate = rate;
                    }
                }
                s.features.macro_draft = None;
                s.features.perkey = None;
                s.toast("Keyboard restored to factory settings", false, cx);
            },
        );
    }

    // ---- live travel ---------------------------------------------------------------

    pub(crate) fn start_stream(&mut self, cx: &mut Context<Self>) {
        if self.streaming {
            return;
        }
        let Some(d) = self.device.as_ref() else {
            return;
        };
        let Some(stream) = d.notify.as_ref() else {
            let e = d.notify_error.clone().unwrap_or_else(|| "no keyboard".into());
            self.toast(format!("Live travel unavailable: {e}"), true, cx);
            return;
        };
        let buffer = stream.buffer.clone();
        let peaks = stream.peaks.clone();
        buffer.lock().unwrap().fill(0);
        peaks.lock().unwrap().fill(0);
        let steps = d.info.steps_per_mm;
        self.streaming = true;
        self.travel.fill(0.0);
        self.peaks.fill(0.0);
        self.reset_tester_model();
        self.run(
            cx,
            "Starting live travel",
            |kb| kb.set_travel_stream(true),
            |_, _, _| {},
        );
        self.stream_gen += 1;
        let gen = self.stream_gen;
        // Copy the stream buffer into the view at ~60 fps while the tester is open.
        cx.spawn(async move |this, cx| {
            // The keyboard only reports changes and can stop short of zero on release, so a
            // key that sits still below its actuation point for a moment counts as released.
            let mut last_raw: Vec<u16> = Vec::new();
            let mut moved_at: Vec<Instant> = Vec::new();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let raw: Vec<u16> = buffer.lock().map(|b| b.clone()).unwrap_or_default();
                let now = Instant::now();
                if last_raw.len() != raw.len() {
                    last_raw = vec![0; raw.len()];
                    moved_at = vec![now; raw.len()];
                }
                for (i, &v) in raw.iter().enumerate() {
                    if v != last_raw[i] {
                        last_raw[i] = v;
                        moved_at[i] = now;
                    }
                }
                let mut snapshot: Vec<f32> = raw.iter().map(|&v| v as f32 / steps).collect();
                let peak_snapshot: Vec<f32> = peaks
                    .lock()
                    .map(|b| b.iter().map(|&v| v as f32 / steps).collect())
                    .unwrap_or_default();
                let alive = this
                    .update(cx, |s, cx| {
                        if s.stream_gen != gen {
                            return false;
                        }
                        let keys = s.device.as_ref().map(|d| d.keys.as_slice()).unwrap_or(&[]);
                        for (i, v) in snapshot.iter_mut().enumerate() {
                            let actuation = keys.get(i).map_or(1.0, |k| k.actuation);
                            if *v > 0.0 && *v < actuation && now - moved_at[i] > STALE_RELEASE {
                                *v = 0.0;
                            }
                        }
                        s.peaks = peak_snapshot;
                        s.travel = snapshot;
                        // The graph scrolls every frame, so redraw even when nothing moved.
                        s.tester_frame();
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn stop_stream(&mut self, cx: &mut Context<Self>) {
        self.stream_gen += 1;
        if std::mem::take(&mut self.streaming) {
            self.run(
                cx,
                "Stopping live travel",
                |kb| kb.set_travel_stream(false),
                |_, _, _| {},
            );
        }
    }

    pub fn reset_peaks(&mut self, cx: &mut Context<Self>) {
        if let Some(stream) = self.device.as_ref().and_then(|d| d.notify.as_ref()) {
            stream.peaks.lock().unwrap().fill(0);
        }
        self.peaks.iter_mut().for_each(|p| *p = 0.0);
        cx.notify();
    }

    /// Window hidden to the tray: stop everything that only serves the open window (live
    /// travel, the polling test, calibration) and send pending edits. Reopening the window
    /// restarts live travel if the Tester's switch tab is showing.
    pub fn hide_to_tray(&mut self, cx: &mut Context<Self>) {
        self.end_calibration(cx);
        self.stop_polling_test(cx);
        if !self.transitioning {
            self.end_drag(cx);
            self.flush_keys(cx);
            self.flush_lighting(cx);
        }
        self.stop_stream(cx);
        if self.recording() {
            self.toggle_recording(cx);
        }
        crate::win::show_window(self.tools.hwnd, false);
        cx.notify();
    }

    pub fn show_from_tray(&mut self, cx: &mut Context<Self>) {
        crate::win::show_window(self.tools.hwnd, true);
        if self.page == Page::Tester && self.features.tester_tab == TesterTab::Switch {
            self.start_stream(cx);
        }
    }

    /// Stops calibration, the stream and the polling test and sends pending edits; the
    /// future resolves once the keyboard has handled them. GPUI's own quit only waits
    /// 100 ms for on_app_quit futures (too short for a key-settings write), so the window
    /// close and tray Quit paths await this before quitting; on_app_quit is a fallback.
    pub fn shutdown(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl std::future::Future<Output = ()> + 'static {
        self.end_calibration(cx);
        self.stop_polling_test(cx);
        if !self.transitioning {
            self.end_drag(cx);
            self.flush_keys(cx);
            self.flush_lighting(cx);
        }
        self.stop_stream(cx);
        let worker = self
            .device
            .as_ref()
            .map(|d| &d.client)
            .or(self.draining.as_ref());
        let barrier = worker.map(|worker| worker.submit(|_| Ok(())));
        async move {
            if let Some(done) = barrier {
                let _ = done.recv().await;
            }
        }
    }
}

/// Everything the pages show, read after connect or factory reset.
struct Fresh {
    profile: u8,
    options: KbOptions,
    debounce: u8,
    keys: Vec<KeyHe>,
    keymap: Vec<KeyAction>,
    fnmap: Vec<KeyAction>,
    lighting: Lighting,
}

impl Fresh {
    fn apply(self, d: &mut Device) {
        d.profile = self.profile;
        d.options = self.options;
        d.debounce = self.debounce;
        d.committed = self.keys.clone();
        d.keys = self.keys;
        d.keymap = self.keymap;
        d.fn_layer = self.options.fn_index;
        d.fnmap = self.fnmap;
        d.lighting = self.lighting;
        d.committed_lighting = self.lighting;
    }
}

fn reload(kb: &mut KeyboardClient) -> Result<Fresh> {
    let profile = kb.get_profile()?;
    let options = kb.get_options()?;
    Ok(Fresh {
        profile,
        options,
        debounce: kb.get_debounce()?,
        keys: kb.get_key_settings()?,
        keymap: kb.get_key_matrix(profile, 0)?,
        fnmap: kb.get_fn_matrix(options.fn_index, fn_system(options.os))?,
        lighting: kb.get_lighting()?,
    })
}

/// Open the keyboard (or the demo) and read everything the pages show. The receiver delivers
/// changes made on the keyboard itself (none in demo mode).
fn load(demo: bool) -> Result<(Device, Option<async_channel::Receiver<VendorEvent>>)> {
    let (mut kb, api, notify, notify_error, events) = if demo {
        let def = devices::find(2304)
            .or_else(|| devices::all().first())
            .ok_or_else(|| anyhow!("no models"))?;
        let m = MockTransport::new(def);
        let flag = m.streaming_flag();
        let stream = TravelStream::spawn(mock::stream_source(flag), None);
        (KeyboardClient::new(Box::new(m)), None, Some(stream), None, None)
    } else {
        let api = HidApi::new()?;
        let t = HidTransport::open(&api)?;
        let (tx, rx) = async_channel::unbounded();
        let (notify, error) = match TravelStream::open(&api, tx) {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(format!("{e:#}"))),
        };
        (
            KeyboardClient::new(Box::new(t)),
            Some(Arc::new(Mutex::new(api))),
            notify,
            error,
            Some(rx),
        )
    };
    let info = kb.identify()?;
    let def = match devices::find(info.device_id) {
        Some(d) => d.clone(),
        None => {
            let mut g = devices::generic(info.device_id);
            g.matrix = kb.get_key_matrix(0, 0)?.concat();
            g
        }
    };
    if let Some(c) = def.color_modes {
        kb.color_modes = ColorModes {
            rainbow: c.rainbow,
            fixed: c.fixed,
        };
    }
    if let Some(p) = &def.preset_colors {
        kb.preset_colors = p
            .iter()
            .filter_map(|(k, v)| Some((k.parse().ok()?, *v)))
            .collect();
    }
    let fresh = reload(&mut kb)?;
    let report_rate = if info.supports_report_rate {
        Some(kb.get_report_rate()?)
    } else {
        None
    };
    let defaults = def.default_actions();
    let layout = devices::build_layout(&defaults);
    let name = if demo {
        format!("{} (demo)", def.display_name)
    } else {
        def.display_name.clone()
    };
    let mut d = Device {
        name,
        fn_defaults: def.default_fn_actions(),
        defaults,
        layout,
        info,
        def,
        client: Arc::new(KeyboardWorker::new(kb)),
        api,
        notify,
        notify_error,
        profile: 0,
        options: fresh.options,
        debounce: 0,
        report_rate,
        macros: HashMap::new(),
        keys: vec![],
        committed: vec![],
        keymap: vec![],
        fnmap: vec![],
        fn_layer: 0,
        lighting: fresh.lighting,
        committed_lighting: fresh.lighting,
    };
    fresh.apply(&mut d);
    Ok((d, events))
}
