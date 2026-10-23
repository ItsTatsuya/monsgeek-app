//! An in-memory keyboard that speaks the same feature-report protocol, for demo mode and tests.

use super::{cmd, mag, Transport, MATRIX_KEYS, PAGE_SIZE, REPORT_SIZE};
use crate::devices::DeviceDef;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

struct Profile {
    /// Key matrix sub-layers; 0 is the keymap, 1–3 hold DKS / mod-tap actions.
    layers: [Vec<u8>; 4],
    /// DKS phase bytes: action g of key i at g * 128 + i, as read through sub 10.
    dks: Vec<u8>,
    mag: HashMap<u8, Vec<u16>>,
    led: [u8; 7],
}

pub struct MockTransport {
    device_id: u32,
    firmware: u16,
    reply: [u8; REPORT_SIZE],
    profile: usize,
    profiles: Vec<Profile>,
    fn_layers: [Vec<u8>; 2],
    options: [u8; 5],
    debounce: u8,
    report_rate: u8,
    pictures: Vec<Vec<u8>>,
    macros: Vec<Vec<u8>>,
    /// Start of the simulated bottom calibration, while it's on.
    bottom_cal: Option<Instant>,
    streaming: Arc<AtomicBool>,
    pub log: Vec<Vec<u8>>,
}

/// One-byte tables; the rest are u16 per key.
fn narrow(sub: u8) -> bool {
    matches!(
        sub,
        mag::MODE | mag::SWITCH_TYPE | mag::SNAP_PARTNER | mag::DEADZONE_TOP | 5
    )
}

impl MockTransport {
    pub fn new(def: &DeviceDef) -> Self {
        let matrix: Vec<u8> = (0..MATRIX_KEYS * 4)
            .map(|i| def.matrix.get(i).copied().unwrap_or(0))
            .collect();
        let fn_default: Vec<u8> = (0..MATRIX_KEYS * 4)
            .map(|i| {
                def.fn_matrix
                    .as_ref()
                    .and_then(|m| m.get(i).copied())
                    .unwrap_or(0)
            })
            .collect();
        let present: Vec<bool> = (0..MATRIX_KEYS)
            .map(|i| matrix[i * 4..i * 4 + 4].iter().any(|&v| v != 0))
            .collect();
        let per = |v: u16| {
            present
                .iter()
                .map(|&p| if p { v } else { 0 })
                .collect::<Vec<_>>()
        };
        let profiles = (0..4)
            .map(|_| Profile {
                layers: [
                    matrix.clone(),
                    vec![0; MATRIX_KEYS * 4],
                    vec![0; MATRIX_KEYS * 4],
                    vec![0; MATRIX_KEYS * 4],
                ],
                dks: vec![0; MATRIX_KEYS * 4],
                led: [1, 0, 3, 0x08, 0x7c, 0x6c, 0xff],
                mag: HashMap::from([
                    (mag::MODE, per(0x80)),
                    (mag::ACTUATION, per(120)),
                    (mag::RELEASE, per(120)),
                    (mag::RT_PRESS, per(30)),
                    (mag::RT_RELEASE, per(30)),
                    (mag::DEADZONE_BOTTOM, per(20)),
                    (mag::SWITCH_TYPE, per(1)),
                    (mag::SNAP_PARTNER, per(0)),
                ]),
            })
            .collect();
        Self {
            device_id: def.id,
            firmware: 0x0309,
            reply: [0; REPORT_SIZE],
            profile: 0,
            profiles,
            fn_layers: [fn_default.clone(), fn_default],
            options: [0, 1, 0, 0, 0],
            debounce: 0,
            report_rate: 3,
            pictures: vec![vec![0; 384]; 5],
            macros: vec![vec![0xff; 256]; 64],
            bottom_cal: None,
            streaming: Arc::new(AtomicBool::new(false)),
            log: Vec::new(),
        }
    }

    /// Set while the host has the travel stream switched on.
    pub fn streaming_flag(&self) -> Arc<AtomicBool> {
        self.streaming.clone()
    }

    fn handle(&mut self, d: &[u8]) {
        self.handle_inner(d);
    }
}

/// Input-report source for `TravelStream::spawn`: W/A/S/D rocking up and down while the stream is on.
pub fn stream_source(
    on: Arc<AtomicBool>,
) -> impl FnMut(&mut [u8]) -> Result<usize> + Send + 'static {
    {
        let start = Instant::now();
        let mut n = 0usize;
        move |buf: &mut [u8]| {
            std::thread::sleep(std::time::Duration::from_millis(2));
            if !on.load(Ordering::Relaxed) {
                return Ok(0);
            }
            let t = start.elapsed().as_secs_f32();
            let keys = [14usize, 9, 15, 21];
            n = (n + 1) % keys.len();
            let v = ((t * 2.2 + n as f32 * 1.6).sin().max(0.0).powi(2) * 380.0) as u16;
            buf[..5].copy_from_slice(&[5, 27, v as u8, (v >> 8) as u8, keys[n] as u8]);
            Ok(5)
        }
    }
}

impl MockTransport {
    fn handle_inner(&mut self, d: &[u8]) {
        let mut r = [0u8; REPORT_SIZE];
        r[0] = d[0];
        let p = self.profile;
        match d[0] {
            cmd::GET_USB_VERSION => {
                r[1..5].copy_from_slice(&self.device_id.to_le_bytes());
                r[7..9].copy_from_slice(&self.firmware.to_le_bytes());
            }
            cmd::GET_FEATURE_LIST => {}
            cmd::GET_PROFILE => r[1] = p as u8,
            cmd::SET_PROFILE => self.profile = (d[1] & 3) as usize,
            cmd::GET_REPORT => r[2] = self.report_rate,
            cmd::SET_REPORT => self.report_rate = d[2],
            cmd::GET_DEBOUNCE => r[1] = self.debounce,
            cmd::SET_DEBOUNCE => self.debounce = d[1],
            cmd::GET_LEDPARAM => r[1..8].copy_from_slice(&self.profiles[p].led),
            cmd::SET_LEDPARAM => self.profiles[p].led.copy_from_slice(&d[1..8]),
            cmd::GET_KBOPTION => r[1..6].copy_from_slice(&self.options),
            cmd::SET_KBOPTION => self.options = [self.options[0], d[2], d[3], d[4], d[5]],
            cmd::SET_RESET => {
                let fresh = Self::reset_from(self);
                *self = fresh;
            }
            cmd::SET_MAGNETISM_REPORT => self.streaming.store(d[1] != 0, Ordering::Relaxed),
            cmd::GET_KEYMATRIX | cmd::GET_FN => {
                let (src, page) = if d[0] == cmd::GET_KEYMATRIX {
                    (
                        &self.profiles[d[1] as usize & 3].layers[d[4] as usize & 3],
                        d[3] as usize,
                    )
                } else {
                    (&self.fn_layers[d[2] as usize & 1], d[4] as usize)
                };
                r.copy_from_slice(&src[page * 64..page * 64 + 64]);
            }
            cmd::SET_KEYMATRIX => {
                let i = d[2] as usize * 4;
                self.profiles[d[1] as usize & 3].layers[d[6] as usize & 3][i..i + 4]
                    .copy_from_slice(&d[8..12]);
            }
            cmd::GET_USERPIC => {
                let page = d[3] as usize * 64;
                r.copy_from_slice(&self.pictures[d[1] as usize % 5][page..page + 64]);
            }
            cmd::SET_USERPIC => {
                let pic = &mut self.pictures[d[1] as usize % 5];
                let start = d[3] as usize * PAGE_SIZE;
                let len = (d[4] as usize).min(PAGE_SIZE).min(384 - start.min(384));
                pic[start..start + len].copy_from_slice(&d[8..8 + len]);
            }
            cmd::GET_MACRO => {
                // Same quirk as fw 3.09: GET k reads macro 2k.
                let slot = &self.macros[(d[1] as usize * 2) % 64];
                let page = d[2] as usize * 64;
                r.copy_from_slice(&slot[page..page + 64]);
            }
            cmd::SET_MACRO => {
                let slot = &mut self.macros[d[1] as usize % 64];
                if d[2] == 0 {
                    slot.fill(0xff);
                }
                let start = d[2] as usize * PAGE_SIZE;
                if start + PAGE_SIZE <= slot.len() {
                    slot[start..start + PAGE_SIZE].copy_from_slice(&d[8..8 + PAGE_SIZE]);
                }
            }
            cmd::SET_CALIBRATE_REST => {}
            cmd::SET_CALIBRATE_BOTTOM => {
                self.bottom_cal = (d[1] != 0).then(Instant::now);
            }
            cmd::SET_FN => {
                let i = d[3] as usize * 4;
                self.fn_layers[d[2] as usize & 1][i..i + 4].copy_from_slice(&d[8..12]);
            }
            cmd::GET_MULTI_MAGNETISM => {
                let sub = d[1];
                if sub == mag::DKS_MODES_READ {
                    let page = d[3] as usize * 64;
                    r.copy_from_slice(&self.profiles[p].dks[page..page + 64]);
                    self.reply = r;
                    return;
                }
                let values: Vec<u16> = if sub == mag::LIVE_TRAVEL {
                    match self.bottom_cal {
                        Some(start) => simulated_calibration(start),
                        None => vec![0; MATRIX_KEYS],
                    }
                } else {
                    self.profiles[p]
                        .mag
                        .get(&sub)
                        .cloned()
                        .unwrap_or(vec![0; MATRIX_KEYS])
                };
                let bytes: Vec<u8> = if narrow(sub) {
                    values.iter().map(|&v| v as u8).collect()
                } else {
                    values.iter().flat_map(|v| v.to_le_bytes()).collect()
                };
                let page = d[3] as usize * 64;
                for (i, value) in r.iter_mut().enumerate() {
                    *value = bytes.get(page + i).copied().unwrap_or(0);
                }
            }
            cmd::SET_MULTI_MAGNETISM => {
                let sub = d[1];
                if sub == mag::DKS_MODES {
                    for g in 0..4 {
                        self.profiles[p].dks[g * MATRIX_KEYS + d[3] as usize] = d[8 + g];
                    }
                    self.reply = r;
                    return;
                }
                let wide = !narrow(sub);
                let table = self.profiles[p]
                    .mag
                    .entry(sub)
                    .or_insert_with(|| vec![0; MATRIX_KEYS]);
                let payload = &d[8..];
                if d[2] == 0 {
                    table[d[3] as usize] = if wide {
                        u16::from_le_bytes([payload[0], payload[1]])
                    } else {
                        payload[0] as u16
                    };
                } else if wide {
                    for b in (0..PAGE_SIZE).step_by(2) {
                        let k = (d[3] as usize * PAGE_SIZE + b) / 2;
                        if k < MATRIX_KEYS {
                            table[k] = u16::from_le_bytes([payload[b], payload[b + 1]]);
                        }
                    }
                } else {
                    for (b, &value) in payload.iter().take(PAGE_SIZE).enumerate() {
                        let k = d[3] as usize * PAGE_SIZE + b;
                        if k < MATRIX_KEYS {
                            table[k] = value as u16;
                        }
                    }
                }
            }
            _ => {}
        }
        self.reply = r;
    }

    fn reset_from(old: &Self) -> Self {
        let def = crate::devices::find(old.device_id)
            .cloned()
            .unwrap_or_else(|| crate::devices::generic(old.device_id));
        let mut fresh = Self::new(&def);
        fresh.streaming = old.streaming.clone();
        fresh.log = old.log.clone();
        fresh
    }
}

/// Demo calibration: keys get pressed one after another, about 4 per second.
/// A few keys stop short of the bottom, like a half-hearted press.
fn simulated_calibration(start: Instant) -> Vec<u16> {
    let s = start.elapsed().as_secs_f32();
    (0..MATRIX_KEYS)
        .map(|i| {
            let depth = if i % 9 == 4 { 250.0 } else { 360.0 };
            ((s * 1.5 - i as f32 * 0.08).clamp(0.0, 1.0) * depth) as u16
        })
        .collect()
}

impl Transport for MockTransport {
    fn send_feature(&mut self, data: &[u8; REPORT_SIZE]) -> Result<()> {
        self.log.push(data.to_vec());
        self.handle(data);
        Ok(())
    }

    fn receive_feature(&mut self) -> Result<[u8; REPORT_SIZE]> {
        Ok(self.reply)
    }

    fn name(&self) -> String {
        "Demo keyboard".into()
    }
}
