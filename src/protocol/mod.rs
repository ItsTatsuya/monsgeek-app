//! MonsGeek / Akko Hall-effect keyboard protocol (see docs/PROTOCOL.md).
//! 64-byte feature reports with report id 0; a checksum at byte 7 (or byte 8 for LED writes).

pub mod advanced;
pub mod hid;
pub mod macros;
pub mod mock;
pub mod userpic;
pub mod worker;

use anyhow::{bail, Result};
use std::thread::sleep;
use std::time::Duration;

pub const VENDOR_ID: u16 = 0x3151;
pub const CONFIG_USAGE_PAGE: u16 = 0xffff;
pub const CONFIG_USAGE: u16 = 0x02;
/// The collection that carries notifications and the travel stream.
pub const NOTIFY_USAGE: u16 = 0x01;
pub const REPORT_SIZE: usize = 64;
pub const PAGE_SIZE: usize = 56;
pub const MATRIX_KEYS: usize = 128;

pub mod cmd {
    pub const SET_RESET: u8 = 0x01;
    pub const SET_REPORT: u8 = 0x03;
    pub const SET_PROFILE: u8 = 0x04;
    pub const SET_DEBOUNCE: u8 = 0x06;
    pub const SET_LEDPARAM: u8 = 0x07;
    pub const SET_KBOPTION: u8 = 0x09;
    pub const SET_KEYMATRIX: u8 = 0x0a;
    pub const SET_MACRO: u8 = 0x0b;
    pub const SET_USERPIC: u8 = 0x0c;
    pub const SET_FN: u8 = 0x10;
    pub const SET_MAGNETISM_REPORT: u8 = 0x1b;
    pub const SET_CALIBRATE_REST: u8 = 0x1c;
    pub const SET_CALIBRATE_BOTTOM: u8 = 0x1e;
    pub const SET_MULTI_MAGNETISM: u8 = 0x65;
    pub const GET_REPORT: u8 = 0x83;
    pub const GET_PROFILE: u8 = 0x84;
    pub const GET_DEBOUNCE: u8 = 0x86;
    pub const GET_LEDPARAM: u8 = 0x87;
    pub const GET_KBOPTION: u8 = 0x89;
    pub const GET_KEYMATRIX: u8 = 0x8a;
    pub const GET_MACRO: u8 = 0x8b;
    pub const GET_USERPIC: u8 = 0x8c;
    pub const GET_USB_VERSION: u8 = 0x8f;
    pub const GET_FN: u8 = 0x90;
    pub const GET_MULTI_MAGNETISM: u8 = 0xe5;
    pub const GET_FEATURE_LIST: u8 = 0xe6;
}

/// Sub-tables of SET/GET_MULTI_MAGNETISM, indexed by matrix key index.
pub mod mag {
    pub const ACTUATION: u8 = 0;
    pub const RELEASE: u8 = 1;
    pub const RT_PRESS: u8 = 2;
    pub const RT_RELEASE: u8 = 3;
    pub const DKS_TRAVEL: u8 = 4;
    pub const MT_TIME: u8 = 5;
    pub const DEADZONE_BOTTOM: u8 = 6;
    pub const MODE: u8 = 7;
    /// DKS phase bytes are written per key through 8 and read back as a whole table through 10.
    pub const DKS_MODES: u8 = 8;
    pub const SNAP_PARTNER: u8 = 9;
    pub const DKS_MODES_READ: u8 = 10;
    pub const DEADZONE_TOP: u8 = 251;
    pub const SWITCH_TYPE: u8 = 252;
    pub const LIVE_TRAVEL: u8 = 254;
}

/// Bit 7 of the MODE table; the low 7 bits are the key mode.
pub const RT_FLAG: u8 = 0x80;

pub mod mode {
    pub const NORMAL: u8 = 0;
    pub const DKS: u8 = 2;
    pub const MOD_TAP: u8 = 3;
    pub const TOGGLE_HOLD: u8 = 4;
    pub const TOGGLE_TAP: u8 = 5;
    pub const SNAP_TAP: u8 = 7;
}

pub const REPORT_RATES: [u32; 7] = [8000, 4000, 2000, 1000, 500, 250, 125];

/// Delays the official driver uses around each transfer.
const PRE_SEND: Duration = Duration::from_millis(10);
const PRE_READ: Duration = Duration::from_millis(10);
const SETTLE: Duration = Duration::from_millis(100);
/// Above this many changed keys, rewrite the whole table instead of sending per-key commands.
const PER_KEY_LIMIT: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checksum {
    Bit7,
    Bit8,
}

/// Build a 64-byte feature report: command bytes, zero padding, checksum at byte 7 or 8.
pub fn frame(cmd: &[u8], checksum: Checksum) -> [u8; REPORT_SIZE] {
    let mut buf = [0u8; REPORT_SIZE];
    let n = cmd.len().min(REPORT_SIZE);
    buf[..n].copy_from_slice(&cmd[..n]);
    let at = match checksum {
        Checksum::Bit7 => 7,
        Checksum::Bit8 => 8,
    };
    let sum: u32 = buf[..at].iter().map(|&b| b as u32).sum();
    buf[at] = 255 - (sum & 0xff) as u8;
    buf
}

/// A device that exchanges 64-byte feature reports (real HID or the mock).
pub trait Transport: Send {
    fn send_feature(&mut self, data: &[u8; REPORT_SIZE]) -> Result<()>;
    fn receive_feature(&mut self) -> Result<[u8; REPORT_SIZE]>;
    #[allow(dead_code)]
    fn name(&self) -> String;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceInfo {
    pub device_id: u32,
    pub firmware: u16,
    /// Raw units per millimetre for travel values.
    pub steps_per_mm: f32,
    pub supports_top_deadzone: bool,
    /// The official driver only offers a polling-rate choice from firmware 4.00; older firmware ignores SET_REPORT.
    pub supports_report_rate: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyHe {
    pub mode: u8,
    pub rapid_trigger: bool,
    pub actuation: f32,
    pub release: f32,
    pub rt_press: f32,
    pub rt_release: f32,
    pub deadzone: f32,
    pub top_deadzone: f32,
    pub switch_type: u8,
    pub snap_partner: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Lighting {
    pub effect: u8,
    /// 0..=4, higher is faster.
    pub speed: u8,
    /// 0..=4
    pub brightness: u8,
    pub option: u8,
    pub rainbow: bool,
    pub color: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbOptions {
    pub os: u8,
    pub fn_index: u8,
    pub anti_mistouch: bool,
    /// 0..=125 in steps of 25.
    pub rt_stability: u8,
    pub wasd_swap: bool,
}

pub type KeyAction = [u8; 4];

/// KBOPTION reports the OS as 0 win, 1 mac, 2 ios, 3 android; the Fn commands number them 0 win, 1 mac, 2 android, 3 ios.
pub fn fn_system(os: u8) -> u8 {
    [0, 1, 3, 2].get(os as usize).copied().unwrap_or(0)
}

/// Per-model LEDPARAM colour-mode codes (low nibble of byte 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorModes {
    pub rainbow: u8,
    pub fixed: u8,
}

impl Default for ColorModes {
    fn default() -> Self {
        Self {
            rainbow: 7,
            fixed: 8,
        }
    }
}

pub struct KeyboardClient {
    transport: Box<dyn Transport>,
    pub info: DeviceInfo,
    pub color_modes: ColorModes,
    /// Other colour-mode nibbles select a preset colour (what Fn + \ cycles through), as 0xRRGGBB.
    pub preset_colors: Vec<(u8, u32)>,
    stream_enabled: bool,
    key_settings_cache: Option<Vec<KeyHe>>,
}

impl KeyboardClient {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            info: DeviceInfo {
                device_id: 0,
                firmware: 0,
                steps_per_mm: 100.0,
                supports_top_deadzone: false,
                supports_report_rate: false,
            },
            color_modes: ColorModes::default(),
            preset_colors: Vec::new(),
            stream_enabled: false,
            key_settings_cache: None,
        }
    }

    #[allow(dead_code)]
    pub fn transport_name(&self) -> String {
        self.transport.name()
    }

    /// Send a GET command and return the 64-byte reply.
    pub fn query(&mut self, cmd: &[u8]) -> Result<[u8; REPORT_SIZE]> {
        sleep(PRE_SEND);
        self.transport.send_feature(&frame(cmd, Checksum::Bit7))?;
        sleep(PRE_READ);
        self.transport.receive_feature()
    }

    /// Send one or more SET frames, then give the firmware time to persist.
    pub fn write(
        &mut self,
        frames: &[Vec<u8>],
        checksum: Checksum,
        settle: Duration,
    ) -> Result<()> {
        for f in frames {
            sleep(PRE_SEND);
            self.transport.send_feature(&frame(f, checksum))?;
        }
        sleep(settle);
        Ok(())
    }

    pub fn query_pages(&mut self, header: impl Fn(u8) -> Vec<u8>, pages: u8) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(pages as usize * REPORT_SIZE);
        for p in 0..pages {
            out.extend_from_slice(&self.query(&header(p))?);
        }
        Ok(out)
    }

    // ---- identity ----------------------------------------------------------

    pub fn identify(&mut self) -> Result<DeviceInfo> {
        let r = self.query(&[cmd::GET_USB_VERSION])?;
        if r[0] != cmd::GET_USB_VERSION {
            bail!("Keyboard did not answer the identify command");
        }
        let device_id = u32::from_le_bytes([r[1], r[2], r[3], r[4]]);
        let firmware = u16::from_le_bytes([r[7], r[8]]);
        let mut steps = if firmware >= 0x0500 {
            200.0
        } else if firmware >= 0x0300 {
            100.0
        } else {
            10.0
        };
        let f = self.query(&[cmd::GET_FEATURE_LIST])?;
        if f[1] == 0xaa {
            steps = match f[2] {
                0 => 100.0,
                1 => 200.0,
                2 => 1000.0,
                _ => steps,
            };
        }
        self.info = DeviceInfo {
            device_id,
            firmware,
            steps_per_mm: steps,
            supports_top_deadzone: firmware >= 0x0400,
            supports_report_rate: firmware >= 0x0400,
        };
        Ok(self.info)
    }

    // ---- general settings --------------------------------------------------

    pub fn get_profile(&mut self) -> Result<u8> {
        Ok(self.query(&[cmd::GET_PROFILE])?[1])
    }

    pub fn set_profile(&mut self, profile: u8) -> Result<()> {
        self.key_settings_cache = None;
        self.write(
            &[vec![cmd::SET_PROFILE, profile]],
            Checksum::Bit7,
            Duration::from_millis(300),
        )
    }

    pub fn get_report_rate(&mut self) -> Result<u32> {
        let r = self.query(&[cmd::GET_REPORT])?;
        Ok(REPORT_RATES.get(r[2] as usize).copied().unwrap_or(1000))
    }

    pub fn set_report_rate(&mut self, hz: u32) -> Result<()> {
        let Some(i) = REPORT_RATES.iter().position(|&r| r == hz) else {
            bail!("Unsupported polling rate {hz}");
        };
        self.write(&[vec![cmd::SET_REPORT, 0, i as u8]], Checksum::Bit7, SETTLE)
    }

    pub fn get_debounce(&mut self) -> Result<u8> {
        Ok(self.query(&[cmd::GET_DEBOUNCE])?[1])
    }

    pub fn set_debounce(&mut self, ms: u8) -> Result<()> {
        self.write(&[vec![cmd::SET_DEBOUNCE, ms]], Checksum::Bit7, SETTLE)
    }

    pub fn get_options(&mut self) -> Result<KbOptions> {
        let r = self.query(&[cmd::GET_KBOPTION])?;
        Ok(KbOptions {
            os: r[1],
            fn_index: r[2],
            anti_mistouch: r[3] != 0,
            rt_stability: if r[4] > 5 { 0 } else { r[4] * 25 },
            wasd_swap: r[5] == 1,
        })
    }

    pub fn set_options(&mut self, o: &KbOptions) -> Result<()> {
        let f = vec![
            cmd::SET_KBOPTION,
            0,
            o.fn_index,
            o.anti_mistouch as u8,
            o.rt_stability / 25,
            o.wasd_swap as u8,
        ];
        self.write(&[f], Checksum::Bit7, SETTLE)
    }

    pub fn factory_reset(&mut self) -> Result<()> {
        self.key_settings_cache = None;
        self.write(
            &[vec![cmd::SET_RESET]],
            Checksum::Bit7,
            Duration::from_millis(2000),
        )
    }

    // ---- lighting ----------------------------------------------------------

    pub fn get_lighting(&mut self) -> Result<Lighting> {
        let r = self.query(&[cmd::GET_LEDPARAM])?;
        Ok(decode_lighting(&r, self.color_modes, &self.preset_colors))
    }

    pub fn set_lighting(&mut self, l: &Lighting) -> Result<()> {
        self.write(
            &[encode_lighting(l, self.color_modes)],
            Checksum::Bit8,
            SETTLE,
        )
    }

    // ---- key matrix (remapping) ---------------------------------------------

    pub fn get_key_matrix(&mut self, profile: u8, layer: u8) -> Result<Vec<KeyAction>> {
        let bytes = self.query_pages(|p| vec![cmd::GET_KEYMATRIX, profile, 0xff, p, layer], 8)?;
        Ok(to_actions(&bytes))
    }

    pub fn set_key(&mut self, profile: u8, layer: u8, index: u8, a: KeyAction) -> Result<()> {
        let f = vec![
            cmd::SET_KEYMATRIX,
            profile,
            index,
            0,
            0,
            1,
            layer,
            0,
            a[0],
            a[1],
            a[2],
            a[3],
        ];
        self.write(&[f], Checksum::Bit7, SETTLE)
    }

    /// Fn layers are global (not per profile): `layer` is the Fn layer index, `system` from fn_system().
    pub fn get_fn_matrix(&mut self, layer: u8, system: u8) -> Result<Vec<KeyAction>> {
        let bytes = self.query_pages(|p| vec![cmd::GET_FN, system, layer, 0xff, p], 8)?;
        Ok(to_actions(&bytes))
    }

    pub fn set_fn_key(&mut self, layer: u8, index: u8, a: KeyAction, system: u8) -> Result<()> {
        let f = vec![
            cmd::SET_FN,
            system,
            layer,
            index,
            0,
            0,
            0,
            0,
            a[0],
            a[1],
            a[2],
            a[3],
        ];
        self.write(&[f], Checksum::Bit7, SETTLE)
    }

    // ---- Hall-effect settings ------------------------------------------------

    fn get_mag_table(&mut self, sub: u8, wide: bool) -> Result<Vec<u16>> {
        let bytes = self.query_pages(
            |p| vec![cmd::GET_MULTI_MAGNETISM, sub, 1, p],
            if wide { 4 } else { 2 },
        )?;
        Ok((0..MATRIX_KEYS)
            .map(|i| {
                if wide {
                    u16::from_le_bytes([bytes[i * 2], bytes[i * 2 + 1]])
                } else {
                    bytes[i] as u16
                }
            })
            .collect())
    }

    pub fn get_key_settings(&mut self) -> Result<Vec<KeyHe>> {
        let s = self.info.steps_per_mm;
        let mm = |v: u16| v as f32 / s;
        let modes = self.get_mag_table(mag::MODE, false)?;
        let act = self.get_mag_table(mag::ACTUATION, true)?;
        let rel = self.get_mag_table(mag::RELEASE, true)?;
        let rtp = self.get_mag_table(mag::RT_PRESS, true)?;
        let rtr = self.get_mag_table(mag::RT_RELEASE, true)?;
        let dz = self.get_mag_table(mag::DEADZONE_BOTTOM, true)?;
        let sw = self.get_mag_table(mag::SWITCH_TYPE, false)?;
        let snap = self.get_mag_table(mag::SNAP_PARTNER, false)?;
        let top = if self.info.supports_top_deadzone {
            Some(self.get_mag_table(mag::DEADZONE_TOP, false)?)
        } else {
            None
        };
        let keys: Vec<_> = (0..MATRIX_KEYS)
            .map(|i| KeyHe {
                mode: modes[i] as u8 & 0x7f,
                rapid_trigger: modes[i] as u8 & RT_FLAG != 0,
                actuation: mm(act[i]),
                release: mm(rel[i]),
                rt_press: mm(rtp[i]),
                rt_release: mm(rtr[i]),
                deadzone: mm(dz[i]),
                top_deadzone: top.as_ref().map_or(0.0, |t| mm(t[i])),
                switch_type: sw[i] as u8,
                snap_partner: snap[i] as u8,
            })
            .collect();
        self.key_settings_cache = Some(keys.clone());
        Ok(keys)
    }

    /// Write the fields that differ between `prev` and `next`.
    pub fn apply_key_settings(&mut self, prev: &[KeyHe], next: &[KeyHe]) -> Result<()> {
        if prev.len() != MATRIX_KEYS || next.len() != MATRIX_KEYS {
            bail!("Expected {MATRIX_KEYS} key settings");
        }
        // Jobs may have been queued before earlier writes completed. Diff against
        // the last successful write, not the snapshot from when the UI queued it.
        if self.key_settings_cache.is_none() {
            self.get_key_settings()?;
        }
        let frames =
            key_settings_frames(self.key_settings_cache.as_deref().unwrap(), next, self.info);
        if frames.is_empty() {
            return Ok(());
        }
        let result = self.write(&frames, Checksum::Bit7, Duration::from_millis(200));
        self.key_settings_cache = result.as_ref().ok().map(|_| next.to_vec());
        result
    }

    /// Change one key's mode (keeping its Rapid Trigger flag) on top of what the keyboard holds.
    pub fn set_key_mode(&mut self, index: usize, mode: u8) -> Result<()> {
        if self.key_settings_cache.is_none() {
            self.get_key_settings()?;
        }
        let prev = self.key_settings_cache.clone().unwrap();
        let mut next = prev.clone();
        let Some(k) = next.get_mut(index) else {
            bail!("No key {index}");
        };
        k.mode = mode;
        self.apply_key_settings(&prev, &next)
    }

    // ---- live travel ---------------------------------------------------------

    /// Turn the 0x1B travel stream on or off. While on, the notify collection sends
    /// `[5, 27, lo, hi, index]` input reports whenever a key moves.
    pub fn set_travel_stream(&mut self, on: bool) -> Result<()> {
        self.write(
            &[vec![cmd::SET_MAGNETISM_REPORT, on as u8]],
            Checksum::Bit7,
            Duration::from_millis(20),
        )?;
        self.stream_enabled = on;
        Ok(())
    }

    #[allow(dead_code)] // fallback when the stream isn't available
    /// One snapshot of every key's travel in mm (about 85 ms; used when the stream isn't available).
    pub fn get_live_travel(&mut self) -> Result<Vec<f32>> {
        let s = self.info.steps_per_mm;
        Ok(self
            .get_raw_travel()?
            .into_iter()
            .map(|v| v as f32 / s)
            .collect())
    }

    /// Raw sub-254 value per key: travel in steps, or calibration progress while calibrating.
    pub fn get_raw_travel(&mut self) -> Result<Vec<u16>> {
        self.get_mag_table(mag::LIVE_TRAVEL, true)
    }

    // ---- calibration (driver's setJiaoZhunKaiGuan) -----------------------------

    /// Rest-position calibration: turn on, keep hands off for ~2 s, turn off.
    pub fn set_rest_calibration(&mut self, on: bool) -> Result<()> {
        self.write(
            &[vec![cmd::SET_CALIBRATE_REST, on as u8]],
            Checksum::Bit7,
            SETTLE,
        )
    }

    /// Bottom calibration: while on, each key is pressed fully down; turning it off saves.
    pub fn set_bottom_calibration(&mut self, on: bool) -> Result<()> {
        self.key_settings_cache = None;
        self.write(
            &[vec![cmd::SET_CALIBRATE_BOTTOM, on as u8]],
            Checksum::Bit7,
            SETTLE,
        )
    }

    // ---- macros ------------------------------------------------------------------

    /// Reads macro n (even n only, see macros::MACRO_INDICES).
    pub fn get_macro(&mut self, n: u8) -> Result<macros::Macro> {
        if !macros::is_readable(n) {
            bail!("Macro {n} can't be read back on this firmware");
        }
        let bytes = self.query_pages(|p| vec![cmd::GET_MACRO, n / 2, p], 4)?;
        Ok(macros::decode(&bytes))
    }

    pub fn set_macro(&mut self, n: u8, m: &macros::Macro) -> Result<()> {
        let frames = macros::frames(n, m)?;
        // The flash commit can take over half a second; give it time before the key binding is sent.
        self.write(&frames, Checksum::Bit7, Duration::from_millis(800))
    }
}

impl Drop for KeyboardClient {
    fn drop(&mut self) {
        if self.stream_enabled {
            let _ = self.set_travel_stream(false);
        }
    }
}

pub fn to_actions(bytes: &[u8]) -> Vec<KeyAction> {
    (0..MATRIX_KEYS)
        .map(|i| {
            let o = i * 4;
            [
                bytes.get(o).copied().unwrap_or(0),
                bytes.get(o + 1).copied().unwrap_or(0),
                bytes.get(o + 2).copied().unwrap_or(0),
                bytes.get(o + 3).copied().unwrap_or(0),
            ]
        })
        .collect()
}

pub fn decode_lighting(
    r: &[u8; REPORT_SIZE],
    modes: ColorModes,
    presets: &[(u8, u32)],
) -> Lighting {
    let mut color = [r[5], r[6], r[7]];
    // The firmware stores pure white as #FAFFFA.
    if color == [0xfa, 0xff, 0xfa] {
        color = [255, 255, 255];
    }
    let mode = r[4] & 0x0f;
    // Fn + \ leaves a preset index in the mode nibble instead of a custom colour.
    if mode != modes.fixed && mode != modes.rainbow {
        if let Some((_, c)) = presets.iter().find(|(m, _)| *m == mode) {
            color = [(c >> 16) as u8, (c >> 8) as u8, *c as u8];
        }
    }
    Lighting {
        effect: r[1],
        speed: 4u8.saturating_sub(r[2]),
        brightness: r[3],
        option: r[4] >> 4,
        rainbow: mode == modes.rainbow,
        color,
    }
}

pub fn encode_lighting(l: &Lighting, modes: ColorModes) -> Vec<u8> {
    let mut c = l.color;
    if c == [255, 255, 255] {
        c = [0xfa, 0xff, 0xfa];
    }
    // Effect 13 shows user picture `option`; the driver sends colour nibble 0 and colour 0,200,200 for it.
    let picture = l.effect == userpic::USER_PICTURE_EFFECT;
    if picture {
        c = [0, 200, 200];
    }
    let mode = (l.option << 4)
        | if picture {
            0
        } else if l.rainbow {
            modes.rainbow
        } else {
            modes.fixed
        };
    vec![
        cmd::SET_LEDPARAM,
        l.effect,
        4u8.saturating_sub(l.speed),
        l.brightness,
        mode,
        c[0],
        c[1],
        c[2],
    ]
}

/// Frames that turn `prev` into `next`: per-key commands for small edits, whole-table pages for large ones.
/// The last frame carries the commit flag (byte 4).
pub fn key_settings_frames(prev: &[KeyHe], next: &[KeyHe], info: DeviceInfo) -> Vec<Vec<u8>> {
    let spm = info.steps_per_mm;
    let steps = move |v: f32| (v * spm).round() as u16;
    type Get = Box<dyn Fn(&KeyHe) -> u16>;
    let mut fields: Vec<(u8, bool, Get)> = vec![
        (
            mag::MODE,
            false,
            Box::new(|k: &KeyHe| (k.mode | if k.rapid_trigger { RT_FLAG } else { 0 }) as u16),
        ),
        (
            mag::ACTUATION,
            true,
            Box::new(move |k: &KeyHe| steps(k.actuation)),
        ),
        (
            mag::RELEASE,
            true,
            Box::new(move |k: &KeyHe| steps(k.release)),
        ),
        (
            mag::RT_PRESS,
            true,
            Box::new(move |k: &KeyHe| steps(k.rt_press)),
        ),
        (
            mag::RT_RELEASE,
            true,
            Box::new(move |k: &KeyHe| steps(k.rt_release)),
        ),
        (
            mag::DEADZONE_BOTTOM,
            true,
            Box::new(move |k: &KeyHe| steps(k.deadzone)),
        ),
        (
            mag::SNAP_PARTNER,
            false,
            Box::new(|k: &KeyHe| k.snap_partner as u16),
        ),
        (
            mag::SWITCH_TYPE,
            false,
            Box::new(|k: &KeyHe| k.switch_type as u16),
        ),
    ];
    if info.supports_top_deadzone {
        fields.push((
            mag::DEADZONE_TOP,
            false,
            Box::new(move |k: &KeyHe| steps(k.top_deadzone)),
        ));
    }
    let mut frames = Vec::new();
    for (sub, wide, get) in &fields {
        let values: Vec<u16> = next.iter().map(get).collect();
        let changed: Vec<usize> = (0..values.len())
            .filter(|&i| values[i] != get(&prev[i]))
            .collect();
        if changed.is_empty() {
            continue;
        }
        if changed.len() <= PER_KEY_LIMIT {
            for i in changed {
                let v = values[i];
                let mut f = vec![cmd::SET_MULTI_MAGNETISM, *sub, 0, i as u8, 0, 0, 0, 0];
                if *wide {
                    f.extend_from_slice(&v.to_le_bytes());
                } else {
                    f.push(v as u8);
                }
                frames.push(f);
            }
        } else {
            let bytes: Vec<u8> = if *wide {
                values.iter().flat_map(|v| v.to_le_bytes()).collect()
            } else {
                values.iter().map(|&v| v as u8).collect()
            };
            for (p, chunk) in bytes.chunks(PAGE_SIZE).enumerate() {
                let mut f = vec![cmd::SET_MULTI_MAGNETISM, *sub, 1, p as u8, 0, 0, 0, 0];
                f.extend_from_slice(chunk);
                frames.push(f);
            }
        }
    }
    if let Some(last) = frames.last_mut() {
        last[4] = 1; // "last" flag: firmware commits to flash
    }
    frames
}

/// Notifications the keyboard sends when something changes on the board itself (Fn shortcuts).
/// They arrive as input report 5 on the notify collection; layout from the driver's parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VendorEvent {
    /// Profile switched (Fn + G/H/J/K).
    Profile,
    /// Polling rate switched.
    ReportRate,
    /// Fn layer switched (Fn + Right Alt), or the Mac/Windows mode changed: re-read KBOPTION.
    Options,
    /// Backlight changed on the board.
    Lighting,
    /// The keyboard finished applying an on-board change: re-read state
    /// (fw 3.09 reports profile switches only this way).
    Sync,
}

pub fn parse_vendor_report(buf: &[u8]) -> Option<VendorEvent> {
    // With or without the leading report id.
    let b = if buf.first() == Some(&5) && buf.len() > 3 { &buf[1..] } else { buf };
    let (a, x, c) = (*b.first()?, *b.get(1)?, *b.get(2)?);
    Some(match (a, x, c) {
        (1, _, _) => VendorEvent::Profile,
        (2, _, 0) => VendorEvent::ReportRate,
        (3, _, 8) | (3, _, 0) | (3, _, 2) => VendorEvent::Options,
        (4..=7, _, _) => VendorEvent::Lighting,
        (15, 0, _) => VendorEvent::Sync,
        _ => return None,
    })
}

/// A travel-stream input report: `[5, 27, lo, hi, index, …]` (or without the leading report id).
pub fn parse_travel_report(buf: &[u8]) -> Option<(usize, u16)> {
    let b = if buf.first() == Some(&5) && buf.get(1) == Some(&27) {
        &buf[1..]
    } else if buf.first() == Some(&27) {
        buf
    } else {
        return None;
    };
    if b.len() < 4 {
        return None;
    }
    let index = b[3] as usize;
    (index < MATRIX_KEYS).then(|| (index, u16::from_le_bytes([b[1], b[2]])))
}

#[cfg(test)]
mod tests {
    use super::mock::MockTransport;
    use super::*;

    #[test]
    fn checksums_match_captured_frames() {
        assert_eq!(frame(&[0x8f], Checksum::Bit7)[7], 0x70);
        assert_eq!(frame(&[0x83], Checksum::Bit7)[7], 0x7c);
        let f = frame(&[0x07, 1, 0, 4, 0x87, 0x57, 0x57, 0x57], Checksum::Bit8);
        assert_eq!(f[8], 255 - ((1 + 4 + 0x87 + 0x57 * 3 + 7) & 0xff) as u8);
    }

    #[test]
    fn travel_reports_parse() {
        assert_eq!(
            parse_travel_report(&[5, 27, 0x5e, 0x01, 14, 0]),
            Some((14, 350))
        );
        assert_eq!(parse_travel_report(&[27, 21, 0, 71]), Some((71, 21)));
        assert_eq!(parse_travel_report(&[5, 15, 1, 0]), None);
        assert_eq!(parse_travel_report(&[5, 27, 0, 0, 200]), None);
        assert_eq!(parse_vendor_report(&[5, 1, 2, 0]), Some(VendorEvent::Profile));
        assert_eq!(parse_vendor_report(&[3, 1, 8, 0]), Some(VendorEvent::Options));
        assert_eq!(parse_vendor_report(&[5, 15, 0, 0]), Some(VendorEvent::Sync));
        assert_eq!(parse_vendor_report(&[15, 1, 0]), None);
        assert_eq!(parse_vendor_report(&[5, 27, 1, 0, 14]), None);
    }

    #[test]
    fn lighting_round_trip_uses_model_codes() {
        let modes = ColorModes {
            rainbow: 7,
            fixed: 8,
        };
        let l = Lighting {
            effect: 1,
            speed: 3,
            brightness: 4,
            option: 0,
            rainbow: true,
            color: [255, 255, 255],
        };
        let f = encode_lighting(&l, modes);
        assert_eq!(f[4] & 0x0f, 7);
        assert_eq!(&f[5..8], &[0xfa, 0xff, 0xfa]);
        let mut r = [0u8; REPORT_SIZE];
        r[..8].copy_from_slice(&f);
        assert_eq!(decode_lighting(&r, modes, &[]), l);
        // A preset nibble from Fn + \ decodes to its colour.
        r[4] = 2;
        assert_eq!(
            decode_lighting(&r, modes, &[(2, 0xff0000)]).color,
            [255, 0, 0]
        );
    }

    #[test]
    fn small_and_large_edits_frame_differently() {
        let info = DeviceInfo {
            device_id: 1,
            firmware: 0x309,
            steps_per_mm: 100.0,
            supports_top_deadzone: false,
            supports_report_rate: false,
        };
        let k = KeyHe {
            mode: 0,
            rapid_trigger: false,
            actuation: 1.2,
            release: 1.2,
            rt_press: 0.3,
            rt_release: 0.3,
            deadzone: 0.2,
            top_deadzone: 0.0,
            switch_type: 1,
            snap_partner: 0,
        };
        let prev = vec![k; MATRIX_KEYS];
        let mut next = prev.clone();
        next[9].actuation = 0.35;
        let f = key_settings_frames(&prev, &next, info);
        assert_eq!(f.len(), 1);
        assert_eq!(
            &f[0][..4],
            &[cmd::SET_MULTI_MAGNETISM, mag::ACTUATION, 0, 9]
        );
        assert_eq!(&f[0][8..10], &35u16.to_le_bytes());
        assert_eq!(f[0][4], 1, "commit flag on the last frame");
        let all: Vec<KeyHe> = prev
            .iter()
            .map(|k| KeyHe {
                actuation: 2.5,
                ..*k
            })
            .collect();
        let f = key_settings_frames(&prev, &all, info);
        assert_eq!(f.len(), 5, "256 bytes in 56-byte pages");
        assert!(f.iter().all(|x| x[2] == 1));
        assert_eq!(f.iter().filter(|x| x[4] == 1).count(), 1);
    }

    #[test]
    fn mock_round_trips() {
        let def = crate::devices::find(2304).unwrap();
        let mut kb = KeyboardClient::new(Box::new(MockTransport::new(def)));
        let info = kb.identify().unwrap();
        assert_eq!(info.device_id, 2304);
        assert_eq!(info.steps_per_mm, 100.0);

        let before = kb.get_key_settings().unwrap();
        let mut after = before.clone();
        after[9].actuation = 0.35;
        after[15].rapid_trigger = true;
        after[15].rt_press = 0.05;
        kb.apply_key_settings(&before, &after).unwrap();
        let read = kb.get_key_settings().unwrap();
        assert!((read[9].actuation - 0.35).abs() < 1e-4);
        assert!(read[15].rapid_trigger);
        assert!((read[15].rt_press - 0.05).abs() < 1e-4);
        assert_eq!(read[20].actuation, before[20].actuation);

        kb.set_key(0, 0, 9, [0, 0, 0x29, 0]).unwrap();
        assert_eq!(kb.get_key_matrix(0, 0).unwrap()[9], [0, 0, 0x29, 0]);

        let opts = kb.get_options().unwrap();
        kb.set_fn_key(1, 14, [0, 0, 0x52, 0], fn_system(opts.os))
            .unwrap();
        assert_eq!(kb.get_fn_matrix(1, 0).unwrap()[14], [0, 0, 0x52, 0]);
        assert_ne!(kb.get_fn_matrix(0, 0).unwrap()[14], [0, 0, 0x52, 0]);

        kb.color_modes = ColorModes {
            rainbow: 7,
            fixed: 8,
        };
        let l = Lighting {
            effect: 2,
            speed: 3,
            brightness: 2,
            option: 0,
            rainbow: false,
            color: [10, 20, 30],
        };
        kb.set_lighting(&l).unwrap();
        assert_eq!(kb.get_lighting().unwrap(), l);

        kb.set_debounce(3).unwrap();
        assert_eq!(kb.get_debounce().unwrap(), 3);
        kb.set_profile(2).unwrap();
        assert_eq!(kb.get_profile().unwrap(), 2);
        assert_ne!(kb.get_lighting().unwrap(), l);
        kb.set_profile(0).unwrap();
        assert_eq!(kb.get_lighting().unwrap(), l);
    }

    #[test]
    fn queued_edits_use_the_last_successful_device_state() {
        let def = crate::devices::find(2304).unwrap();
        let mut kb = KeyboardClient::new(Box::new(MockTransport::new(def)));
        kb.identify().unwrap();
        let initial = kb.get_key_settings().unwrap();
        let mut changed = initial.clone();
        changed[9].actuation = 0.35;
        kb.apply_key_settings(&initial, &changed).unwrap();
        // The second UI edit was queued before the first result arrived. Its
        // baseline is stale, and returning the slider to its initial value must
        // still undo the first write.
        kb.apply_key_settings(&initial, &initial).unwrap();
        assert_eq!(kb.get_key_settings().unwrap(), initial);
        kb.set_profile(1).unwrap();
        kb.apply_key_settings(&initial, &changed).unwrap();
        kb.set_profile(0).unwrap();
        assert_eq!(kb.get_key_settings().unwrap(), initial);
    }

    #[test]
    fn key_mode_writes_on_top_of_queued_edits() {
        let def = crate::devices::find(2304).unwrap();
        let mut kb = KeyboardClient::new(Box::new(MockTransport::new(def)));
        kb.identify().unwrap();
        let initial = kb.get_key_settings().unwrap();
        let mut edited = initial.clone();
        edited[9].actuation = 0.35;
        kb.apply_key_settings(&initial, &edited).unwrap();
        // The UI's baseline still predates the edit; the mode write must not undo it.
        kb.set_key_mode(14, mode::DKS).unwrap();
        let read = kb.get_key_settings().unwrap();
        assert_eq!(read[14].mode, mode::DKS);
        assert_eq!(read[14].rapid_trigger, initial[14].rapid_trigger);
        assert!((read[9].actuation - 0.35).abs() < 1e-4);
        // The cache follows the write, so a later diff only sends real changes.
        kb.apply_key_settings(&initial, &read).unwrap();
        assert_eq!(kb.get_key_settings().unwrap(), read);
    }

    #[test]
    fn releasing_client_stops_the_travel_stream() {
        use std::sync::atomic::Ordering;
        let mock = MockTransport::new(crate::devices::find(2304).unwrap());
        let streaming = mock.streaming_flag();
        let mut kb = KeyboardClient::new(Box::new(mock));
        kb.set_travel_stream(true).unwrap();
        assert!(streaming.load(Ordering::Relaxed));
        drop(kb);
        assert!(!streaming.load(Ordering::Relaxed));
    }

    #[test]
    fn retry_recovers_a_partially_written_settings_table() {
        use std::sync::{
            atomic::{AtomicI32, Ordering},
            Arc,
        };
        struct FailingTransport {
            mock: MockTransport,
            fail_after: Arc<AtomicI32>,
        }
        impl Transport for FailingTransport {
            fn send_feature(&mut self, data: &[u8; REPORT_SIZE]) -> Result<()> {
                if data[0] == cmd::SET_MULTI_MAGNETISM {
                    let remaining = self.fail_after.load(Ordering::Relaxed);
                    if remaining == 0 {
                        self.fail_after.store(-1, Ordering::Relaxed);
                        bail!("simulated USB failure");
                    }
                    if remaining > 0 {
                        self.fail_after.fetch_sub(1, Ordering::Relaxed);
                    }
                }
                self.mock.send_feature(data)
            }
            fn receive_feature(&mut self) -> Result<[u8; REPORT_SIZE]> {
                self.mock.receive_feature()
            }
            fn name(&self) -> String {
                "Failing test keyboard".into()
            }
        }
        let fail_after = Arc::new(AtomicI32::new(-1));
        let transport = FailingTransport {
            mock: MockTransport::new(crate::devices::find(2304).unwrap()),
            fail_after: fail_after.clone(),
        };
        let mut kb = KeyboardClient::new(Box::new(transport));
        kb.identify().unwrap();
        let initial = kb.get_key_settings().unwrap();
        let mut changed = initial.clone();
        changed[9].actuation = 0.35;
        changed[9].release = 0.45;
        fail_after.store(1, Ordering::Relaxed);
        assert!(kb.apply_key_settings(&initial, &changed).is_err());
        assert!(kb.key_settings_cache.is_none());
        kb.apply_key_settings(&initial, &initial).unwrap();
        assert_eq!(kb.get_key_settings().unwrap(), initial);
    }
}
