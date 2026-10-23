//! Approximate renderings of the firmware lighting effects, for the keyboard preview.
//! The firmware's exact animations aren't documented; these match their look (direction, motion,
//! colour use).

use crate::devices::Layout;
use crate::protocol::Lighting;
use gpui::{hsla, rgb, Hsla};

/// Effects that react to key presses; the preview fakes presses when you aren't typing.
pub const REACTIVE: [u8; 3] = [5, 8, 19];

/// A key press, in seconds on the preview clock.
#[derive(Clone, Copy)]
pub struct Press {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub t: f32,
}

/// Key centres, 0..1 across the board, in layout order.
pub fn centres(layout: &Layout) -> Vec<(usize, f32, f32)> {
    layout
        .keys
        .iter()
        .map(|k| {
            (
                k.index,
                (k.x + k.w / 2.0) / layout.width,
                (k.y + k.h / 2.0) / layout.height,
            )
        })
        .collect()
}

/// Effects that change over time (and so need redrawing).
pub fn animated(effect: u8) -> bool {
    !matches!(effect, 0 | 1 | crate::protocol::userpic::USER_PICTURE_EFFECT)
}

fn frac(v: f32) -> f32 {
    v - v.floor()
}

/// Deterministic 0..1 noise per (a, b), so random effects don't flicker between frames.
fn hash(a: f32, b: f32) -> f32 {
    frac(((a * 127.1 + b * 311.7).sin() * 43758.547).abs())
}

fn hue(h: f32) -> Hsla {
    hsla(frac(h), 0.95, 0.6, 1.0)
}

/// Colour and level (0..1) for each key, in layout order.
pub fn render(
    l: &Lighting,
    colorable: bool,
    keys: &[(usize, f32, f32)],
    t: f32,
    presses: &[Press],
) -> Vec<(Hsla, f32)> {
    let rate = 0.45 + l.speed as f32 * 0.35;
    let tt = t * rate;
    let multi = l.rainbow || !colorable;
    let base: Hsla =
        rgb(((l.color[0] as u32) << 16) | ((l.color[1] as u32) << 8) | l.color[2] as u32).into();
    // Colour for a key: the chosen colour, or a hue that drifts across the board.
    let tint = |x: f32, shift: f32| if multi { hue(x * 300.0 / 360.0 + shift) } else { base };
    let pick = |h: f32| if multi { hue(h) } else { base };
    let recent: Vec<Press> = presses.iter().copied().filter(|e| t - e.t < 2.5).collect();
    let row_of = |y: f32| (y * 5.0).round();

    let mut out: Vec<(Hsla, f32)> = Vec::with_capacity(keys.len());
    let mut each = |f: &mut dyn FnMut(usize, usize, f32, f32) -> (Hsla, f32)| {
        for (i, &(idx, x, y)) in keys.iter().enumerate() {
            let (c, v) = f(i, idx, x, y);
            out.push((c, v.clamp(0.0, 1.0)));
        }
    };

    match l.effect {
        0 => each(&mut |_, _, _, _| (base, 0.0)),
        1 => each(&mut |_, _, x, _| (tint(x, 0.0), 1.0)),
        2 => {
            // breathing
            let lvl = (tt * 1.6).sin().powi(2);
            each(&mut |_, _, _, _| (pick(tt * 0.05), 0.08 + lvl * 0.92))
        }
        // neon: the whole board cycles through the spectrum
        3 => each(&mut |_, _, _, _| (hue(tt * 0.12), 1.0)),
        4 => each(&mut |_, _, x, y| {
            // wave: 0 right, 1 left, 2 down, 3 up
            let pos = match l.option {
                1 => 1.0 - x,
                2 => y,
                3 => 1.0 - y,
                _ => x,
            };
            let ph = pos - tt * 0.5;
            if multi {
                (hue(ph), 1.0)
            } else {
                (base, 0.15 + 0.85 * (frac(ph) * std::f32::consts::PI).sin().powi(2))
            }
        }),
        5 | 24 => {
            // ripple: rings spread from each press; fireworks: bursts from random points
            let bursts: Vec<Press> = if l.effect == 5 {
                recent.clone()
            } else {
                (0..3)
                    .map(|i| {
                        let i = i as f32;
                        let n = (tt * 0.6 + i / 3.0).floor();
                        let side = if l.option == 1 { 0.25 } else { 0.75 };
                        Press {
                            index: usize::MAX,
                            x: (side + (hash(n, i) - 0.5) * 0.6).clamp(0.0, 1.0),
                            y: hash(n, i + 7.0) * 0.8 + 0.1,
                            t: (n - i / 3.0) / (0.6 * rate),
                        }
                    })
                    .collect()
            };
            each(&mut |_, _, x, y| {
                let mut best = 0.0;
                let mut col = tint(x, 0.0);
                for e in &bursts {
                    let age = (t - e.t) * rate;
                    if !(0.0..=2.0).contains(&age) {
                        continue;
                    }
                    let d = ((x - e.x) * 2.6).hypot(y - e.y);
                    let ring = (1.0 - (d - age * 0.9).abs() * 5.0).max(0.0) * (1.0 - age / 2.0);
                    if ring > best {
                        best = ring;
                        col = pick(hash(e.t, 0.0) + d * 0.3);
                    }
                }
                (col, best)
            })
        }
        6 => each(&mut |_, idx, _, _| {
            // raindrop: random keys flash and fade
            let k = idx as f32;
            let cycle = tt * 0.7 + hash(k, 0.0) * 10.0;
            let on = hash(k, cycle.floor()) > 0.75;
            (pick(hash(k, cycle.floor() + 3.0)), if on { 1.0 - frac(cycle) } else { 0.0 })
        }),
        7 => {
            // snake: a lit run following zigzag rows (option 0) or a spiral (option 1)
            let order = snake_order(keys, l.option == 1);
            let n = order.len().max(1) as f32;
            let head = (tt * 9.0) % n;
            let mut rank = vec![0.0; keys.len()];
            for (r, &i) in order.iter().enumerate() {
                rank[i] = r as f32;
            }
            each(&mut |i, _, _, _| {
                let behind = (head - rank[i] + n) % n;
                (pick(rank[i] / n), if behind < 7.0 { 1.0 - behind / 7.0 } else { 0.0 })
            })
        }
        8 | 19 => each(&mut |_, idx, x, _| {
            // reactive: pressed keys light and fade; inverse: lit board, pressed keys go dark
            let e = recent.iter().rev().find(|e| e.index == idx);
            let fade = e.map_or(0.0, |e| (1.0 - (t - e.t) * rate * 0.8).clamp(0.0, 1.0));
            let c = tint(x, e.map_or(0.0, |e| e.t * 0.1));
            (c, if l.effect == 8 { fade } else { 1.0 - fade })
        }),
        9 => each(&mut |_, _, x, _| {
            // converge: bars sweep in from both sides to the centre
            let d = (x - 0.5).abs() * 2.0;
            let ph = frac(tt * 0.45);
            (
                pick((tt * 0.45).floor() * 0.17),
                1.0 - (d - (1.0 - ph)).abs() * 6.0,
            )
        }),
        10 => each(&mut |_, _, x, y| {
            // sine wave: a band follows a moving sine curve
            let c = 0.5 + 0.38 * ((x * 2.0 - tt * 0.5) * std::f32::consts::PI).sin();
            (tint(x, tt * 0.05), 1.0 - (y - c).abs() * 4.0)
        }),
        11 => {
            // kaleidoscope: rings around the centre, outward (0) or inward (1), hue by angle
            let dir = if l.option == 1 { 1.0 } else { -1.0 };
            each(&mut |_, _, x, y| {
                let dx = (x - 0.5) * 2.6;
                let dy = y - 0.5;
                let d = dx.hypot(dy);
                let ang = dy.atan2(dx) / std::f32::consts::TAU;
                let ring = 0.35
                    + 0.65 * (frac(d * 1.5 + dir * tt * 0.4) * std::f32::consts::PI).sin().powi(2);
                (pick(ang + tt * 0.05), ring)
            })
        }
        12 => {
            // line wave: a vertical line sweeping back and forth
            let pos = (frac(tt * 0.3) * 2.0 - 1.0).abs();
            each(&mut |_, _, x, _| (tint(x, 0.0), 1.0 - (x - pos).abs() * 9.0))
        }
        14 => each(&mut |_, _, x, y| {
            // laser: a dot runs along each row at its own pace
            let row = row_of(y);
            let pos = frac(tt * (0.35 + hash(row, 0.0) * 0.3) + hash(row, 2.0));
            (pick(row / 6.0 + tt * 0.03), 1.0 - (x - pos).abs() * 10.0)
        }),
        15 => {
            // circle wave: a beam rotating around the centre, counter-clockwise (0) or clockwise (1)
            let dir = if l.option == 1 { 1.0 } else { -1.0 };
            each(&mut |_, _, x, y| {
                let ang = (y - 0.5).atan2((x - 0.5) * 2.6) / std::f32::consts::TAU;
                let sweep = frac(ang - dir * tt * 0.3);
                (pick(ang), 0.1 + 0.9 * (1.0 - sweep * 3.0).max(0.0))
            })
        }
        16 => each(&mut |_, idx, _, _| {
            // dazzle: every key twinkles its own colour
            let k = idx as f32;
            let n = (tt * 1.2 + hash(k, 0.0) * 5.0).floor();
            (pick(hash(k, n)), 0.35 + 0.65 * hash(k, n + 11.0))
        }),
        17 => each(&mut |_, _, x, y| {
            // rain down: drops fall down columns
            let col = (x * 16.0).round();
            let drop = frac(tt * (0.5 + hash(col, 0.0) * 0.4) + hash(col, 5.0));
            let d = drop * 1.3 - y;
            (pick(col / 16.0), if d > 0.0 && d < 0.35 { 1.0 - d / 0.35 } else { 0.0 })
        }),
        18 => each(&mut |_, _, x, y| {
            // meteor: streaks with fading tails cross each row
            let row = row_of(y);
            let head = frac(tt * 0.4 + hash(row, 9.0)) * 1.4;
            let d = head - x;
            (pick(row / 6.0), if d > 0.0 && d < 0.3 { 1.0 - d / 0.3 } else { 0.0 })
        }),
        23 => each(&mut |_, _, x, y| {
            // train: a block of keys travels along each row, alternating direction
            let row = row_of(y);
            let x = if row as i32 % 2 == 1 { 1.0 - x } else { x };
            let pos = frac(tt * 0.3 + row * 0.15);
            let d = frac(x - pos + 1.0);
            (tint(x, 0.0), if d < 0.18 { 1.0 } else { 0.04 })
        }),
        _ => each(&mut |_, _, x, _| (tint(x, 0.0), 1.0)),
    }
    out
}

/// Key order (indices into `keys`) for the snake effect: rows left↔right alternately, or a spiral inwards.
fn snake_order(keys: &[(usize, f32, f32)], spiral: bool) -> Vec<usize> {
    let mut order: Vec<usize> = (0..keys.len()).collect();
    if spiral {
        let ring = |i: usize| {
            let (_, x, y) = keys[i];
            (((x - 0.5) * 2.6).hypot(y - 0.5) * 4.0).round()
        };
        let ang = |i: usize| (keys[i].2 - 0.5).atan2(keys[i].1 - 0.5);
        order.sort_by(|&a, &b| {
            ring(b)
                .total_cmp(&ring(a))
                .then(ang(a).total_cmp(&ang(b)))
        });
    } else {
        let row = |i: usize| (keys[i].2 * 5.0).round() as i32;
        order.sort_by(|&a, &b| {
            let (ra, rb) = (row(a), row(b));
            ra.cmp(&rb).then(if ra % 2 == 1 {
                keys[b].1.total_cmp(&keys[a].1)
            } else {
                keys[a].1.total_cmp(&keys[b].1)
            })
        });
    }
    order
}
