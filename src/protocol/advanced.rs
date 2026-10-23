//! Dynamic keystroke (DKS) and mod-tap, from the driver's getMultiMagnetismInfo /
//! setMagnetismInfoSimple. Both keep their actions in the key matrix sub-layers (the
//! "sonProfile" byte of GET/SET_KEYMATRIX); the FUN60 PRO has 4 sub-layers per profile.
//!
//! DKS (mode 2):
//!   actions     key matrix sub-layers 0..3 of the key (sub-layer 0 is the base remap)
//!   trigger pt  Mag sub 4, u16 steps per key
//!   phases      written per key with Mag sub 8 (4 bytes, one per action); read back from
//!               Mag sub 10, 8 pages laid out as 4 × 128 bytes: action g of key i at [g * 128 + i]
//!   phase byte  2 bits per phase, phase p in bits 2p..2p+1. Phases: 0 press past the trigger
//!               point, 1 bottom out, 2 start of release, 3 release back past the trigger point.
//!               Cells: 0 nothing, 1 tap, 2 hold until the next phase, 3 hold through the next
//!               phase (a run of 3s ends in a 2).
//! Mod-tap (mode 3):
//!   hold action sub-layer 0, tap action sub-layer 1, hold time Mag sub 5 = ms / 10 (10–1000 ms)

use super::{cmd, mag, Checksum, KeyAction, KeyboardClient, MATRIX_KEYS};
use anyhow::Result;
use std::time::Duration;

pub const DKS_PHASES: [&str; 4] = ["Press", "Bottom out", "Release", "Release past"];
pub const DKS_ACTIONS: usize = 4;
pub const DKS_TRAVEL_RANGE: (f32, f32) = (0.5, 2.5);
pub const DKS_DEFAULT_TRAVEL: f32 = 0.7;
pub const MT_TIME_RANGE: (f32, f32) = (10.0, 1000.0);
pub const MT_DEFAULT_TIME: u16 = 200;

const EMPTY: KeyAction = [0, 0, 0, 0];

pub type DksCells = [u8; 4];

#[derive(Clone, Debug, PartialEq)]
pub struct DksConfig {
    /// Trigger point in mm.
    pub travel: f32,
    pub actions: [KeyAction; DKS_ACTIONS],
    pub cells: [DksCells; DKS_ACTIONS],
}

impl DksConfig {
    /// Starting point for a key new to DKS: its own action held for the whole press.
    pub fn fresh(base: KeyAction) -> Self {
        Self {
            travel: DKS_DEFAULT_TRAVEL,
            actions: [base, EMPTY, EMPTY, EMPTY],
            cells: [[3, 3, 2, 0], [0; 4], [0; 4], [0; 4]],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModTapConfig {
    pub hold: KeyAction,
    pub tap: KeyAction,
    /// Hold time in ms.
    pub time: u16,
}

pub fn encode_cells(c: DksCells) -> u8 {
    ((c[3] & 3) << 6) | ((c[2] & 3) << 4) | ((c[1] & 3) << 2) | (c[0] & 3)
}

pub fn decode_cells(b: u8) -> DksCells {
    [b & 3, (b >> 2) & 3, (b >> 4) & 3, (b >> 6) & 3]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub from: usize,
    /// Phase where a held action is released; None for a tap.
    pub to: Option<usize>,
}

impl Segment {
    fn end(&self) -> usize {
        self.to.unwrap_or(self.from + 1)
    }
}

/// The taps and holds in one action's cells.
pub fn segments(c: DksCells) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < 4 {
        match c[i] {
            1 => out.push(Segment { from: i, to: None }),
            2 => out.push(Segment {
                from: i,
                to: Some(i + 1),
            }),
            3 => {
                let mut j = i;
                while j < 3 && c[j] == 3 {
                    j += 1;
                }
                out.push(Segment {
                    from: i,
                    to: Some((j + 1).min(3)),
                });
                i = j;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

pub fn from_segments(segs: &[Segment]) -> DksCells {
    let mut c = [0u8; 4];
    for s in segs {
        match s.to {
            None => c[s.from] = 1,
            Some(to) => {
                for cell in c.iter_mut().take(to - 1).skip(s.from) {
                    *cell = 3;
                }
                c[to - 1] = 2;
            }
        }
    }
    c
}

/// Click on a phase: empty → tap → hold to the next phase → hold one further … → empty.
/// Clicking inside an existing hold edits that hold.
pub fn cycle_cell(c: DksCells, phase: usize) -> DksCells {
    let segs = segments(c);
    let hit = segs
        .iter()
        .position(|s| s.from == phase || s.to.is_some_and(|to| phase > s.from && phase < to));
    let others: Vec<Segment> = segs
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != hit)
        .map(|(_, s)| *s)
        .collect();
    let free = |from: usize, to: usize| others.iter().all(|s| s.end() <= from || s.from >= to);
    let Some(hit) = hit.map(|i| segs[i]) else {
        if free(phase, phase + 1) {
            let mut all = others.clone();
            all.push(Segment {
                from: phase,
                to: None,
            });
            return from_segments(&all);
        }
        return c;
    };
    let next = hit.to.map_or(hit.from + 1, |to| to + 1);
    let mut all = others.clone();
    if next <= 3 && free(hit.from, next) {
        all.push(Segment {
            from: hit.from,
            to: Some(next),
        });
    }
    from_segments(&all)
}

/// Per-key magnetism write, as the driver's _sendMagnetismInfoSimpleCMD.
fn mag_frame(sub: u8, index: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = vec![cmd::SET_MULTI_MAGNETISM, sub, 0, index, 0, 0, 0, 0];
    f.extend_from_slice(payload);
    f
}

impl KeyboardClient {
    fn mag_pages(&mut self, sub: u8, pages: u8) -> Result<Vec<u8>> {
        self.query_pages(|p| vec![cmd::GET_MULTI_MAGNETISM, sub, 1, p], pages)
    }

    fn layer_actions(&mut self, profile: u8, index: usize, count: usize) -> Result<Vec<KeyAction>> {
        (0..count)
            .map(|l| Ok(self.get_key_matrix(profile, l as u8)?[index]))
            .collect()
    }

    pub fn read_dks(&mut self, profile: u8, index: usize) -> Result<DksConfig> {
        let travel = self.mag_pages(mag::DKS_TRAVEL, 4)?;
        let modes = self.mag_pages(mag::DKS_MODES_READ, 8)?;
        let raw = u16::from_le_bytes([travel[index * 2], travel[index * 2 + 1]]);
        let actions = self.layer_actions(profile, index, DKS_ACTIONS)?;
        Ok(DksConfig {
            travel: if raw == 0 {
                DKS_DEFAULT_TRAVEL
            } else {
                raw as f32 / self.info.steps_per_mm
            },
            actions: [actions[0], actions[1], actions[2], actions[3]],
            cells: std::array::from_fn(|g| decode_cells(modes[g * MATRIX_KEYS + index])),
        })
    }

    pub fn write_dks(&mut self, profile: u8, index: usize, cfg: &DksConfig) -> Result<()> {
        for (l, a) in cfg.actions.iter().enumerate() {
            self.set_key(profile, l as u8, index as u8, *a)?;
        }
        let t = (cfg.travel * self.info.steps_per_mm).round() as u16;
        let cells: Vec<u8> = cfg.cells.iter().map(|c| encode_cells(*c)).collect();
        let mut frames = vec![
            mag_frame(mag::DKS_TRAVEL, index as u8, &t.to_le_bytes()),
            mag_frame(mag::DKS_MODES, index as u8, &cells),
        ];
        frames.last_mut().unwrap()[4] = 1; // commit
        self.write(&frames, Checksum::Bit7, Duration::from_millis(200))
    }

    pub fn read_mod_tap(&mut self, profile: u8, index: usize) -> Result<ModTapConfig> {
        let times = self.mag_pages(mag::MT_TIME, 2)?;
        let actions = self.layer_actions(profile, index, 2)?;
        Ok(ModTapConfig {
            hold: actions[0],
            tap: actions[1],
            time: if times[index] == 0 {
                MT_DEFAULT_TIME
            } else {
                times[index] as u16 * 10
            },
        })
    }

    pub fn write_mod_tap(&mut self, profile: u8, index: usize, cfg: &ModTapConfig) -> Result<()> {
        self.set_key(profile, 0, index as u8, cfg.hold)?;
        self.set_key(profile, 1, index as u8, cfg.tap)?;
        let time = (cfg.time as f32).clamp(MT_TIME_RANGE.0, MT_TIME_RANGE.1);
        let mut f = mag_frame(mag::MT_TIME, index as u8, &[(time / 10.0).round() as u8]);
        f[4] = 1;
        self.write(&[f], Checksum::Bit7, Duration::from_millis(200))
    }

    /// Undo DKS / mod-tap layer data: base action back in sub-layer 0, the other sub-layers emptied.
    pub fn clear_layers(&mut self, profile: u8, index: usize, base: KeyAction) -> Result<()> {
        self.set_key(profile, 0, index as u8, base)?;
        for l in 1..DKS_ACTIONS as u8 {
            self.set_key(profile, l, index as u8, EMPTY)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::mock::MockTransport;

    #[test]
    fn cells_cycle_through_tap_and_holds() {
        let c = cycle_cell([0; 4], 0);
        assert_eq!(c, [1, 0, 0, 0]);
        let c = cycle_cell(c, 0);
        assert_eq!(c, [2, 0, 0, 0]);
        let c = cycle_cell(c, 0);
        assert_eq!(c, [3, 2, 0, 0]);
        let c = cycle_cell(c, 1); // inside the hold edits it
        assert_eq!(c, [3, 3, 2, 0]);
        let c = cycle_cell(c, 0);
        assert_eq!(c, [0, 0, 0, 0]);
        assert_eq!(decode_cells(encode_cells([3, 3, 2, 1])), [3, 3, 2, 1]);
        assert_eq!(segments([3, 3, 2, 0]), vec![Segment { from: 0, to: Some(3) }]);
    }

    #[test]
    fn dks_and_mod_tap_round_trip_on_the_mock() {
        let def = crate::devices::find(2304).unwrap();
        let mut kb = KeyboardClient::new(Box::new(MockTransport::new(def)));
        kb.identify().unwrap();
        let mut dks = DksConfig::fresh([0, 0, 0x1a, 0]);
        dks.actions[1] = [0, 0, 0xe1, 0];
        dks.cells[1] = [0, 1, 0, 0];
        dks.travel = 1.25;
        kb.write_dks(0, 14, &dks).unwrap();
        let read = kb.read_dks(0, 14).unwrap();
        assert_eq!(read.actions, dks.actions);
        assert_eq!(read.cells, dks.cells);
        assert!((read.travel - 1.25).abs() < 1e-4);

        let mt = ModTapConfig {
            hold: [0, 0, 0xe0, 0],
            tap: [0, 0, 0x29, 0],
            time: 250,
        };
        kb.write_mod_tap(0, 9, &mt).unwrap();
        assert_eq!(kb.read_mod_tap(0, 9).unwrap(), mt);
        kb.clear_layers(0, 9, [0, 0, 0x04, 0]).unwrap();
        assert_eq!(kb.get_key_matrix(0, 1).unwrap()[9], EMPTY);
        assert_eq!(kb.get_key_matrix(0, 0).unwrap()[9], [0, 0, 0x04, 0]);
    }
}
