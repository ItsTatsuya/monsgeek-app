//! Keyboard sync (on-board changes), lighting per profile and Fn layer, automatic profiles,
//! the polling-rate test, and the switch tester's Rapid Trigger model.

use super::{reload, AppView, Page, TesterTab};
use crate::config::{AutoRule, Config};
use crate::layer_lighting::{Engine, LayerLooks};
use crate::polling::{self, PollEstimate};
use crate::protocol::{KeyHe, Lighting, VendorEvent};
use crate::rt_model::{self, RtConfig, RtEvent, RtState};
use crate::win::{ForegroundWatcher, RawCapture, RawEvent, Tray, TrayEvent};
use gpui::Context;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Collapse rapid toggles (Fn + Right Alt mashing) into one lighting write.
const SETTLE: Duration = Duration::from_millis(150);
/// Lighting notifications this soon after our own write are our echo.
const ECHO: Duration = Duration::from_millis(1500);
/// Switch after the foreground app has settled, so alt-tabbing doesn't flood the keyboard.
const SWITCH_DELAY: Duration = Duration::from_millis(400);
/// Graph samples kept: about 4 s at 60 frames a second.
pub const HISTORY: usize = 240;
const LOG: usize = 12;
const MAX_POLL_TIMES: usize = 4000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TunerScope {
    Key,
    Same,
    All,
}

#[derive(Clone, Copy, Debug)]
pub struct LoggedEvent {
    /// Seconds since the tester opened.
    pub at: f32,
    pub event: RtEvent,
    pub depth: f32,
}

pub struct Tools {
    // Lighting per profile and Fn layer.
    pub engine: Engine,
    ll_gen: u64,
    quiet_until: Option<Instant>,
    // Automatic profiles.
    watcher: Option<ForegroundWatcher>,
    tray: Option<Tray>,
    pub current_app: Option<(String, String)>,
    pub recent_apps: Vec<(String, String)>,
    auto_gen: u64,
    pub rule_text: String,
    /// The main window, hidden to the tray on close while automatic profiles run.
    pub hwnd: isize,
    // Polling-rate test.
    capture: Option<RawCapture>,
    poll_gen: u64,
    pub poll_ready: bool,
    pub poll_error: Option<String>,
    pub poll_times: Vec<f64>,
    // Switch tester model.
    pub history: VecDeque<f32>,
    pub marks: VecDeque<Option<RtEvent>>,
    pub rt_state: RtState,
    rt_key: Option<(usize, RtConfig)>,
    pub log: VecDeque<LoggedEvent>,
    started: Instant,
    pub scope: TunerScope,
}

impl Tools {
    pub fn new(_config: &Config) -> Self {
        Self {
            engine: Engine::default(),
            ll_gen: 0,
            quiet_until: None,
            watcher: None,
            tray: None,
            current_app: None,
            recent_apps: Vec::new(),
            auto_gen: 0,
            rule_text: String::new(),
            hwnd: 0,
            capture: None,
            poll_gen: 0,
            poll_ready: false,
            poll_error: None,
            poll_times: Vec::new(),
            history: VecDeque::with_capacity(HISTORY),
            marks: VecDeque::with_capacity(HISTORY),
            rt_state: RtState::default(),
            rt_key: None,
            log: VecDeque::new(),
            started: Instant::now(),
            scope: TunerScope::Key,
        }
    }

    pub fn polling(&self) -> bool {
        self.capture.is_some()
    }

    pub fn estimate(&self) -> PollEstimate {
        polling::estimate(&self.poll_times)
    }
}

fn same_settings(a: &KeyHe, b: &KeyHe) -> bool {
    a.actuation == b.actuation
        && a.rapid_trigger == b.rapid_trigger
        && a.rt_press == b.rt_press
        && a.rt_release == b.rt_release
        && a.deadzone == b.deadzone
}

impl AppView {
    // ---- keyboard sync -----------------------------------------------------------------

    pub(super) fn watch_keyboard(
        &mut self,
        events: Option<async_channel::Receiver<VendorEvent>>,
        cx: &mut Context<Self>,
    ) {
        let Some(events) = events else { return };
        let connection = self.connection_gen;
        cx.spawn(async move |this, cx| {
            while let Ok(e) = events.recv().await {
                let alive = this
                    .update(cx, |s, cx| {
                        if s.connection_gen != connection {
                            return false;
                        }
                        s.on_vendor_event(e, cx);
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

    /// Keep the pages in step with changes made on the keyboard: profile, Fn layer and lighting.
    fn on_vendor_event(&mut self, e: VendorEvent, cx: &mut Context<Self>) {
        let Some(d) = self.device.as_ref() else {
            return;
        };
        if self.transitioning {
            return;
        }
        let current = d.profile;
        let quiet = self.tools.quiet_until.is_some_and(|t| Instant::now() < t)
            || d.lighting != d.committed_lighting;
        match e {
            VendorEvent::Profile | VendorEvent::Sync => self.run(
                cx,
                "Reading keyboard changes",
                move |kb| {
                    let p = kb.get_profile()?;
                    let fresh = if p != current { Some(reload(kb)?) } else { None };
                    Ok((fresh, kb.get_options()?, kb.get_lighting()?))
                },
                move |s, (fresh, options, lighting), cx| {
                    let Some(d) = s.device.as_mut() else { return };
                    if let Some(fresh) = fresh {
                        // The keyboard switched profile itself; edits for the old one are moot.
                        s.key_gen += 1;
                        s.light_gen += 1;
                        fresh.apply(d);
                        s.selection.clear();
                        s.features.adv = None;
                        s.features.macro_draft = None;
                    } else if !quiet && d.lighting == d.committed_lighting {
                        d.lighting = lighting;
                        d.committed_lighting = lighting;
                    }
                    d.options = options;
                    s.schedule_layer_lighting(cx);
                },
            ),
            VendorEvent::Options => self.run(
                cx,
                "Reading keyboard changes",
                |kb| kb.get_options(),
                |s, o, cx| {
                    if let Some(d) = s.device.as_mut() {
                        d.options = o;
                    }
                    s.schedule_layer_lighting(cx);
                },
            ),
            VendorEvent::Lighting if !quiet => self.run(
                cx,
                "Reading lighting",
                |kb| kb.get_lighting(),
                |s, l, _| {
                    if let Some(d) = s.device.as_mut() {
                        if d.lighting == d.committed_lighting {
                            d.lighting = l;
                            d.committed_lighting = l;
                        }
                    }
                },
            ),
            VendorEvent::ReportRate if d.info.supports_report_rate => self.run(
                cx,
                "Reading polling rate",
                |kb| kb.get_report_rate(),
                |s, hz, _| {
                    if let Some(d) = s.device.as_mut() {
                        d.report_rate = Some(hz);
                    }
                },
            ),
            _ => {}
        }
    }

    pub(super) fn after_connect(&mut self, cx: &mut Context<Self>) {
        self.tools.engine.reset();
        self.schedule_layer_lighting(cx);
        self.schedule_auto_switch(cx);
    }

    // ---- lighting per profile and Fn layer ------------------------------------------------

    pub fn looks(&self) -> LayerLooks {
        let Some(d) = self.device.as_ref() else {
            return LayerLooks::default();
        };
        let fn_layers = d.def.fn_layer_count().max(1) as usize;
        self.config
            .layer_looks
            .get(&d.info.device_id)
            .cloned()
            .unwrap_or_else(|| LayerLooks::empty(fn_layers))
            .fitted(fn_layers)
    }

    pub fn set_looks(&mut self, looks: LayerLooks, cx: &mut Context<Self>) {
        let Some(id) = self.device.as_ref().map(|d| d.info.device_id) else {
            return;
        };
        self.config.layer_looks.insert(id, looks);
        self.config.save();
        self.schedule_layer_lighting(cx);
        cx.notify();
    }

    /// Store the lighting shown now as the look for a profile (`fn_layer` false) or Fn layer.
    pub fn set_look_slot(&mut self, fn_layer: bool, i: usize, look: Option<Lighting>, cx: &mut Context<Self>) {
        let mut looks = self.looks();
        let slots = if fn_layer { &mut looks.fn_layers } else { &mut looks.profiles };
        if let Some(slot) = slots.get_mut(i) {
            *slot = look;
        }
        self.set_looks(looks, cx);
    }

    /// Write the look for the current profile and Fn layer once switching settles.
    pub fn schedule_layer_lighting(&mut self, cx: &mut Context<Self>) {
        self.tools.ll_gen += 1;
        let gen = self.tools.ll_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            this.update(cx, |s, cx| {
                if s.tools.ll_gen == gen {
                    s.apply_layer_lighting(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn apply_layer_lighting(&mut self, cx: &mut Context<Self>) {
        if self.transitioning {
            return self.schedule_layer_lighting(cx);
        }
        let looks = self.looks();
        let Some(d) = self.device.as_mut() else {
            return;
        };
        if let Some(l) = self.tools.engine.decide(&looks, d.profile, d.options.fn_index, d.lighting) {
            self.tools.quiet_until = Some(Instant::now() + ECHO);
            d.lighting = l;
            self.flush_lighting(cx);
            cx.notify();
        }
    }

    // ---- automatic profiles ----------------------------------------------------------------

    pub fn edit_auto(&mut self, f: impl FnOnce(&mut crate::config::AutoProfiles), cx: &mut Context<Self>) {
        f(&mut self.config.auto_profiles);
        self.config.save();
        self.sync_auto(cx);
        self.schedule_auto_switch(cx);
        cx.notify();
    }

    /// Start or stop the foreground watcher and the tray icon to match the settings.
    pub fn sync_auto(&mut self, cx: &mut Context<Self>) {
        let a = &self.config.auto_profiles;
        let (enabled, tray) = (a.enabled, a.enabled && a.tray);
        if enabled && self.tools.watcher.is_none() {
            let (tx, rx) = async_channel::unbounded();
            self.tools.watcher = Some(ForegroundWatcher::start(tx));
            cx.spawn(async move |this, cx| {
                while let Ok((exe, title)) = rx.recv().await {
                    if this.update(cx, |s, cx| s.on_foreground(exe, title, cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
        } else if !enabled && self.tools.watcher.is_some() {
            self.tools.watcher = None;
            self.tools.current_app = None;
            self.tools.auto_gen += 1;
        }
        if tray && self.tools.tray.is_none() {
            let (tx, rx) = async_channel::unbounded();
            self.tools.tray = Some(Tray::start("Monsgeek App · automatic profiles on", tx));
            cx.spawn(async move |this, cx| {
                while let Ok(e) = rx.recv().await {
                    let r = this.update(cx, |s, cx| match e {
                        TrayEvent::Open => s.show_from_tray(cx),
                        TrayEvent::Quit => s.quit_from_tray(cx),
                    });
                    if r.is_err() {
                        break;
                    }
                }
            })
            .detach();
        } else if !tray && self.tools.tray.is_some() {
            self.tools.tray = None;
            // Never leave the window hidden without a way back.
            self.show_from_tray(cx);
        }
    }

    /// Like closing the window: wait until pending writes reach the keyboard, then quit
    /// (GPUI's own quit gives them only 100 ms).
    fn quit_from_tray(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let done = self.shutdown(cx);
        self.closing = true;
        self.transitioning = true;
        cx.spawn(async move |_, cx| {
            done.await;
            cx.update(|cx| cx.quit()).ok();
        })
        .detach();
        cx.notify();
    }

    /// Closing the window hides it to the tray while automatic profiles run there.
    pub fn hides_to_tray(&self) -> bool {
        self.tools.tray.is_some() && self.tools.hwnd != 0
    }

    fn on_foreground(&mut self, exe: String, title: String, cx: &mut Context<Self>) {
        self.tools.recent_apps.retain(|(e, _)| *e != exe);
        self.tools.recent_apps.insert(0, (exe.clone(), title.clone()));
        self.tools.recent_apps.truncate(20);
        self.tools.current_app = Some((exe, title));
        self.schedule_auto_switch(cx);
        cx.notify();
    }

    fn schedule_auto_switch(&mut self, cx: &mut Context<Self>) {
        self.tools.auto_gen += 1;
        let gen = self.tools.auto_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SWITCH_DELAY).await;
            this.update(cx, |s, cx| {
                if s.tools.auto_gen != gen || !s.config.auto_profiles.enabled || s.transitioning {
                    return;
                }
                let Some((exe, _)) = s.tools.current_app.as_ref() else { return };
                let target = s.config.auto_profiles.target(exe);
                if let (Some(p), Some(d)) = (target, s.device.as_ref()) {
                    if p != d.profile {
                        s.set_profile(p, cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn add_rule(&mut self, name: &str, cx: &mut Context<Self>) {
        let clean = name.trim().rsplit(['\\', '/']).next().unwrap_or("").trim().to_string();
        if clean.is_empty() {
            return;
        }
        let exe = if clean.to_lowercase().ends_with(".exe") { clean } else { format!("{clean}.exe") };
        if self.config.auto_profiles.rules.iter().any(|r| r.exe.eq_ignore_ascii_case(&exe)) {
            return;
        }
        let profile = if self.config.auto_profiles.default_profile == Some(1) { 0 } else { 1 };
        self.tools.rule_text.clear();
        self.features.edit = None;
        self.edit_auto(|a| a.rules.push(AutoRule { exe, profile }), cx);
    }

    // ---- polling-rate test -------------------------------------------------------------------

    pub fn set_tester_tab_tools(&mut self, leaving: TesterTab, cx: &mut Context<Self>) {
        if leaving == TesterTab::Polling {
            self.stop_polling_test(cx);
        }
    }

    pub fn start_polling_test(&mut self, cx: &mut Context<Self>) {
        self.stop_polling_test(cx);
        self.tools.poll_times.clear();
        self.tools.poll_error = None;
        self.tools.poll_ready = false;
        self.tools.poll_gen += 1;
        let gen = self.tools.poll_gen;
        // Only the MonsGeek keyboard (VID 0x3151); the demo has none, so it listens to every keyboard.
        let filter = (!self.demo).then_some("VID_3151");
        let (tx, rx) = async_channel::unbounded();
        self.tools.capture = Some(RawCapture::start(filter, tx));
        cx.spawn(async move |this, cx| {
            while let Ok(first) = rx.recv().await {
                // Take everything already queued so a burst costs one redraw.
                let mut batch = vec![first];
                while let Ok(e) = rx.try_recv() {
                    batch.push(e);
                }
                let alive = this
                    .update(cx, |s, cx| {
                        if s.tools.poll_gen != gen {
                            return false;
                        }
                        for e in batch {
                            match e {
                                RawEvent::Ready => s.tools.poll_ready = true,
                                RawEvent::Error(m) => s.tools.poll_error = Some(m),
                                RawEvent::Key(t) => s.tools.poll_times.push(t),
                            }
                        }
                        let n = s.tools.poll_times.len();
                        if n > MAX_POLL_TIMES {
                            s.tools.poll_times.drain(..n - MAX_POLL_TIMES);
                        }
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
        cx.notify();
    }

    pub fn stop_polling_test(&mut self, cx: &mut Context<Self>) {
        if self.tools.capture.take().is_some() {
            self.tools.poll_gen += 1;
            self.tools.poll_ready = false;
            cx.notify();
        }
    }

    // ---- switch tester model ----------------------------------------------------------------------

    /// The key the tester follows: the pinned one, else whichever is pressed deepest.
    pub fn tester_focus(&self) -> Option<usize> {
        if self.page != Page::Tester {
            return None;
        }
        if let [i] = self.selection[..] {
            return Some(i);
        }
        let d = self.device.as_ref()?;
        d.layout
            .keys
            .iter()
            .map(|k| k.index)
            .max_by(|&a, &b| self.travel[a].total_cmp(&self.travel[b]))
            .filter(|&i| self.travel[i] > 0.05)
    }

    pub fn tester_config(&self, focus: usize) -> Option<RtConfig> {
        let k = self.device.as_ref()?.keys.get(focus)?;
        Some(RtConfig::from_key(k, self.peaks[focus].max(3.4)))
    }

    pub(super) fn reset_tester_model(&mut self) {
        let t = &mut self.tools;
        t.history.clear();
        t.marks.clear();
        t.rt_state = RtState::default();
        t.rt_key = None;
        t.log.clear();
        t.started = Instant::now();
    }

    /// One 60 fps frame: extend the graph and step the prediction for the focused key.
    pub(super) fn tester_frame(&mut self) {
        let focus = self.tester_focus();
        let travel = focus.map_or(0.0, |i| self.travel[i]);
        let config = focus.and_then(|i| self.tester_config(i));
        // Start the prediction over when the focused key or its settings change
        // (full travel only grows with the peak, so it doesn't count as a change).
        let key = focus.zip(config).map(|(i, c)| (i, RtConfig { full_travel: 0.0, ..c }));
        let t = &mut self.tools;
        if key != t.rt_key {
            t.rt_key = key;
            t.rt_state = RtState::default();
            t.marks.iter_mut().for_each(|m| *m = None);
            t.log.clear();
        }
        let mut event = None;
        if let Some(c) = config {
            let (state, e) = rt_model::step(t.rt_state, travel, &c);
            t.rt_state = state;
            event = e;
            if let Some(e) = e {
                t.log.push_front(LoggedEvent {
                    at: t.started.elapsed().as_secs_f32(),
                    event: e,
                    depth: travel,
                });
                t.log.truncate(LOG);
            }
        }
        if t.history.len() == HISTORY {
            t.history.pop_front();
            t.marks.pop_front();
        }
        t.history.push_back(travel);
        t.marks.push_back(event);
    }

    /// Keys a settings edit applies to: the selection, or the tuner's scope in the tester.
    pub fn edit_targets(&self) -> Vec<usize> {
        if self.page != Page::Tester {
            return self.selection.clone();
        }
        let (Some(d), [pinned]) = (self.device.as_ref(), &self.selection[..]) else {
            return Vec::new();
        };
        let physical = d.layout.keys.iter().map(|k| k.index);
        match self.tools.scope {
            TunerScope::Key => vec![*pinned],
            TunerScope::All => physical.collect(),
            TunerScope::Same => {
                let k = d.keys[*pinned];
                physical.filter(|&i| same_settings(&d.keys[i], &k)).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_settings_compares_the_tuned_fields() {
        let k = KeyHe {
            mode: 0,
            rapid_trigger: true,
            actuation: 1.2,
            release: 1.2,
            rt_press: 0.3,
            rt_release: 0.3,
            deadzone: 0.2,
            top_deadzone: 0.0,
            switch_type: 1,
            snap_partner: 0,
        };
        assert!(same_settings(&k, &KeyHe { switch_type: 4, ..k }));
        assert!(!same_settings(&k, &KeyHe { rt_press: 0.2, ..k }));
    }
}
