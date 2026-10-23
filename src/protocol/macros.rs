//! Macro storage format, from the driver's macroEventToByte / buffToMacroEvents.
//!
//! A macro is `u16 repeat` followed by one record per key event:
//!   short: [usage, down<<7 | delay]          when 1 <= delay <= 127 ms
//!   long:  [usage, down<<7, delay lo, hi]    otherwise
//! and ends at the first all-zero 4-byte record. The delay is the pause *after* the event.
//! Usages 4..231 are keyboard keys, 240..244 mouse buttons, 249 a mouse move (read, never written).

use super::{cmd, PAGE_SIZE};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroStep {
    pub key: u8,
    pub down: bool,
    /// Pause after this event, ms.
    pub delay: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    pub repeat: u16,
    pub steps: Vec<MacroStep>,
}

impl Default for Macro {
    fn default() -> Self {
        Self {
            repeat: 1,
            steps: Vec::new(),
        }
    }
}

/// Key action byte 1 for a macro key: how the firmware plays it.
pub mod play {
    /// Play `repeat` times per press.
    pub const REPEAT: u8 = 0;
    /// Press to start looping, press again to stop.
    pub const TOGGLE: u8 = 1;
    /// Loop while the key is held.
    pub const WHILE_HELD: u8 = 2;
}

/// A key bound to macro n is `[9, mode, n, 0]`.
pub const MACRO_ACTION: u8 = 9;

/// Macro numbers the app uses. On fw 3.09 GET_MACRO k reads macro 2k, so odd macros can be
/// written and played but never read back. The app sticks to even numbers up to the official
/// driver's limit of 50.
pub const MACRO_INDICES: [u8; 25] = {
    let mut a = [0u8; 25];
    let mut i = 0;
    while i < 25 {
        a[i] = (i * 2) as u8;
        i += 1;
    }
    a
};

pub fn is_readable(n: u8) -> bool {
    n.is_multiple_of(2)
}

/// Number shown to people: 1, 2, 3… for macros 0, 2, 4…
#[allow(dead_code)]
pub fn number(n: u8) -> Option<u8> {
    is_readable(n).then_some(n / 2 + 1)
}

/// Four 56-byte pages, terminator included.
pub const MACRO_BYTES: usize = 4 * PAGE_SIZE;
const MOUSE_MOVE: u8 = 249;

pub fn encode(m: &Macro) -> Vec<u8> {
    let mut out = m.repeat.to_le_bytes().to_vec();
    for s in &m.steps {
        let flag = if s.down { 0x80 } else { 0 };
        if (1..=127).contains(&s.delay) {
            out.extend_from_slice(&[s.key, flag | s.delay as u8]);
        } else {
            out.extend_from_slice(&[s.key, flag, s.delay as u8, (s.delay >> 8) as u8]);
        }
    }
    out
}

/// Bytes a macro takes on the keyboard, including the 4-byte terminator.
pub fn size(m: &Macro) -> usize {
    encode(m).len() + 4
}

pub fn decode(b: &[u8]) -> Macro {
    if b.len() < 6 || b[..6].iter().all(|&v| v == 0xff) {
        return Macro::default();
    }
    let mut steps = Vec::new();
    let mut i = 2;
    while i + 4 <= b.len() {
        let (k, f, lo, hi) = (b[i], b[i + 1], b[i + 2], b[i + 3]);
        if k == 0 && f == 0 && lo == 0 && hi == 0 {
            break;
        }
        if k == MOUSE_MOVE {
            // [249, delay, dx, dy] or [249, 0, dx, dy, lo, hi]; mouse moves are skipped.
            i += if f != 0 { 4 } else { 6 };
            continue;
        }
        let short = f & 0x7f != 0;
        steps.push(MacroStep {
            key: k,
            down: f & 0x80 != 0,
            delay: if short {
                (f & 0x7f) as u16
            } else {
                u16::from_le_bytes([lo, hi])
            },
        });
        i += if short { 2 } else { 4 };
    }
    if steps.is_empty() {
        Macro::default()
    } else {
        Macro {
            repeat: u16::from_le_bytes([b[0], b[1]]),
            steps,
        }
    }
}

/// SET_MACRO frames; the last page carries the commit flag.
pub fn frames(n: u8, m: &Macro) -> Result<Vec<Vec<u8>>> {
    let data = encode(m);
    if data.len() + 4 > MACRO_BYTES {
        bail!("Macro is too long for the keyboard");
    }
    let pages = (data.len() + 4).div_ceil(PAGE_SIZE);
    Ok((0..pages)
        .map(|p| {
            let start = (p * PAGE_SIZE).min(data.len());
            let chunk = &data[start..((p + 1) * PAGE_SIZE).min(data.len())];
            let mut f = vec![
                cmd::SET_MACRO,
                n,
                p as u8,
                PAGE_SIZE as u8,
                (p == pages - 1) as u8,
                0,
                0,
                0,
            ];
            f.extend_from_slice(chunk);
            f.resize(8 + PAGE_SIZE, 0);
            f
        })
        .collect())
}

/// Keys pressed but never released by the end of the macro (they would stay stuck down).
pub fn held_at_end(steps: &[MacroStep]) -> Vec<u8> {
    let mut held: Vec<u8> = Vec::new();
    for s in steps {
        held.retain(|&k| k != s.key);
        if s.down {
            held.push(s.key);
        }
    }
    held
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_short_and_long_delays() {
        let m = Macro {
            repeat: 3,
            steps: vec![
                MacroStep {
                    key: 0x04,
                    down: true,
                    delay: 20,
                },
                MacroStep {
                    key: 0x04,
                    down: false,
                    delay: 500,
                },
                MacroStep {
                    key: 0xe0,
                    down: true,
                    delay: 0,
                },
            ],
        };
        let mut bytes = encode(&m);
        assert_eq!(&bytes[2..4], &[0x04, 0x80 | 20]);
        bytes.extend_from_slice(&[0, 0, 0, 0, 0xff, 0xff]);
        assert_eq!(decode(&bytes), m);
        assert_eq!(decode(&[0xff; 64]), Macro::default());
        assert_eq!(held_at_end(&m.steps), vec![0xe0]);
    }

    #[test]
    fn frames_are_paged_with_a_commit_flag() {
        let m = Macro {
            repeat: 1,
            steps: vec![
                MacroStep {
                    key: 5,
                    down: true,
                    delay: 10
                };
                40
            ],
        };
        let f = frames(4, &m).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[1][4], 1);
        assert_eq!(f[0][4], 0);
        assert!(f.iter().all(|x| x.len() == 64));
        let too_long = Macro {
            repeat: 1,
            steps: vec![
                MacroStep {
                    key: 5,
                    down: true,
                    delay: 1000
                };
                60
            ],
        };
        assert!(frames(0, &too_long).is_err());
    }
}
