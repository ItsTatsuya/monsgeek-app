//! MonsGeek models (extracted from the official driver) and the keyboard drawing built from their matrices.

use crate::protocol::KeyAction;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDef {
    pub id: u32,
    #[serde(default)]
    #[allow(dead_code)]
    pub pid: u32,
    #[allow(dead_code)]
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub fn_layers: Option<u8>,
    #[serde(default)]
    pub travel_range: Option<[f32; 2]>,
    /// Default 4-byte action per matrix index (column-major, 6 rows).
    #[serde(default)]
    pub matrix: Vec<u8>,
    /// Factory Fn layer, same layout as `matrix`.
    #[serde(default)]
    pub fn_matrix: Option<Vec<u8>>,
    #[serde(default)]
    pub color_modes: Option<ColorModesDef>,
    /// Colour-mode nibble → 0xRRGGBB preset (what Fn + \ cycles through).
    #[serde(default)]
    pub preset_colors: Option<HashMap<String, u32>>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ColorModesDef {
    pub rainbow: u8,
    pub fixed: u8,
}

const RAW: &str = include_str!("../assets/monsgeek.json");

pub fn all() -> &'static [DeviceDef] {
    static DEVICES: OnceLock<Vec<DeviceDef>> = OnceLock::new();
    DEVICES.get_or_init(|| {
        serde_json::from_str::<Vec<DeviceDef>>(RAW)
            .expect("monsgeek.json")
            .into_iter()
            .filter(|d| !d.matrix.is_empty())
            .collect()
    })
}

pub fn find(id: u32) -> Option<&'static DeviceDef> {
    all().iter().find(|d| d.id == id)
}

/// For a model we don't know: the matrix comes from the keyboard's own base layer later.
pub fn generic(id: u32) -> DeviceDef {
    DeviceDef {
        id,
        pid: 0,
        name: "unknown".into(),
        display_name: format!("MonsGeek keyboard #{id}"),
        fn_layers: Some(2),
        travel_range: None,
        matrix: vec![],
        fn_matrix: None,
        color_modes: Some(ColorModesDef {
            rainbow: 7,
            fixed: 8,
        }),
        preset_colors: None,
    }
}

pub fn actions(m: &[u8]) -> Vec<KeyAction> {
    crate::protocol::to_actions(m)
}

impl DeviceDef {
    pub fn default_actions(&self) -> Vec<KeyAction> {
        actions(&self.matrix)
    }
    pub fn default_fn_actions(&self) -> Vec<KeyAction> {
        actions(self.fn_matrix.as_deref().unwrap_or(&[]))
    }
    pub fn fn_layer_count(&self) -> u8 {
        self.fn_layers.unwrap_or(1).max(1)
    }
    pub fn travel(&self) -> (f32, f32) {
        self.travel_range
            .map(|r| (r[0], r[1]))
            .unwrap_or((0.1, 3.4))
    }
}

#[derive(Clone, Debug)]
pub struct VisualKey {
    pub index: usize,
    pub base: KeyAction,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub keys: Vec<VisualKey>,
    pub width: f32,
    pub height: f32,
}

const ROWS: usize = 6;

fn hid_of(a: &KeyAction) -> i32 {
    if a[0] == 0 && a[1] == 0 && a[3] == 0 {
        a[2] as i32
    } else {
        -1
    }
}
fn is_fn(a: &KeyAction) -> bool {
    a[0] == 10 && a[1] == 1
}
/// Media keys, knobs and other firmware functions that sit in the matrix.
fn is_extra(a: &KeyAction) -> bool {
    a[0] != 0 && !is_fn(a)
}

fn width_of(a: &KeyAction, row: &[(usize, KeyAction)]) -> f32 {
    let hid = hid_of(a);
    let arrows = row.iter().any(|(_, b)| matches!(hid_of(b), 0x4f..=0x52));
    if matches!(hid, 0xe6 | 0xe7 | 0xe4 | 0x65) || is_fn(a) {
        return if arrows { 1.0 } else { 1.25 };
    }
    if hid == 0xe1 && row.iter().any(|(_, b)| hid_of(b) == 0x64) {
        return 1.25; // ISO left shift
    }
    match hid {
        0x2a => 2.0,
        0x2b | 0x31 => 1.5,
        0x39 => 1.75,
        0x28 | 0xe1 => 2.25,
        0xe5 => 2.75,
        0x2c => 6.25,
        0xe0 | 0xe3 | 0xe2 => 1.25,
        _ => 1.0,
    }
}

fn nav_pos(hid: i32) -> Option<(f32, f32)> {
    Some(match hid {
        0x46 => (0., 0.),
        0x47 => (0., 1.),
        0x48 => (0., 2.),
        0x49 => (1., 0.),
        0x4a => (1., 1.),
        0x4b => (1., 2.),
        0x4c => (2., 0.),
        0x4d => (2., 1.),
        0x4e => (2., 2.),
        0x52 => (4., 1.),
        0x50 => (5., 0.),
        0x51 => (5., 1.),
        0x4f => (5., 2.),
        _ => return None,
    })
}

fn num_pos(hid: i32) -> Option<(f32, f32, f32, f32)> {
    Some(match hid {
        0x53 => (1., 0., 1., 1.),
        0x54 => (1., 1., 1., 1.),
        0x55 => (1., 2., 1., 1.),
        0x56 => (1., 3., 1., 1.),
        0x5f => (2., 0., 1., 1.),
        0x60 => (2., 1., 1., 1.),
        0x61 => (2., 2., 1., 1.),
        0x57 => (2., 3., 1., 2.),
        0x5c => (3., 0., 1., 1.),
        0x5d => (3., 1., 1., 1.),
        0x5e => (3., 2., 1., 1.),
        0x59 => (4., 0., 1., 1.),
        0x5a => (4., 1., 1., 1.),
        0x5b => (4., 2., 1., 1.),
        0x58 => (4., 3., 1., 2.),
        0x62 => (5., 0., 2., 1.),
        0x63 => (5., 2., 1., 1.),
        _ => return None,
    })
}

/// Lay out a keyboard from its matrix alone: row = index % 6, column order = index / 6.
/// Compact boards trust the matrix rows; TKL/full-size boards place the nav cluster and numpad.
pub fn build_layout(base: &[KeyAction]) -> Layout {
    let present: Vec<(usize, KeyAction)> = base
        .iter()
        .enumerate()
        .filter(|(_, a)| a.iter().any(|&v| v != 0))
        .map(|(i, a)| (i, *a))
        .collect();
    let clustered = present.iter().any(|(_, a)| hid_of(a) == 0x46);
    let is_nav = |a: &KeyAction| clustered && nav_pos(hid_of(a)).is_some();
    let is_num = |a: &KeyAction| clustered && num_pos(hid_of(a)).is_some();

    let mut rows: Vec<Vec<(usize, KeyAction)>> = vec![vec![]; ROWS];
    for (i, a) in present
        .iter()
        .filter(|(_, a)| !is_nav(a) && !is_num(a) && !is_extra(a))
    {
        rows[i % ROWS].push((*i, *a));
    }
    let natural =
        |row: &[(usize, KeyAction)]| row.iter().map(|(_, a)| width_of(a, row)).sum::<f32>();
    let main_width = natural(&rows[1]).max(13.0);
    let flex = [
        None,
        Some(0x2a),
        Some(0x31),
        Some(0x28),
        Some(0xe5),
        Some(0x2c),
    ];
    let top = if clustered { 0.25 } else { 0.0 };
    let y_of = |r: f32| if r > 0.0 { r + top } else { r };

    let mut keys = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        let mut widths: Vec<f32> = row.iter().map(|(_, a)| width_of(a, row)).collect();
        let total: f32 = widths.iter().sum();
        if r > 0 {
            if let Some(at) = row.iter().position(|(_, a)| Some(hid_of(a)) == flex[r]) {
                widths[at] = (widths[at] + main_width - total).max(1.0);
            }
        }
        // F-row: split Esc / F1-F4 / F5-F8 / F9-F12 with the room that's left.
        let mut gap_before: HashMap<usize, f32> = HashMap::new();
        if r == 0 && total < main_width {
            let starts: Vec<usize> = (0..row.len())
                .filter(|&i| {
                    let h = hid_of(&row[i].1);
                    [0x3a, 0x3e, 0x42, 0x46].contains(&h)
                        || (h > 0x45 && i > 0 && hid_of(&row[i - 1].1) == 0x45)
                })
                .collect();
            let gap = if starts.is_empty() {
                0.0
            } else {
                ((main_width - total) / starts.len() as f32).min(1.0)
            };
            for s in starts {
                gap_before.insert(s, gap);
            }
        }
        let mut x = 0.0;
        for (i, (index, a)) in row.iter().enumerate() {
            x += gap_before.get(&i).copied().unwrap_or(0.0);
            keys.push(VisualKey {
                index: *index,
                base: *a,
                x,
                y: y_of(r as f32),
                w: widths[i],
                h: 1.0,
            });
            x += widths[i];
        }
    }

    if clustered {
        let nav_x = main_width + 0.25;
        let num_x = nav_x + 3.25;
        for (i, a) in present.iter().filter(|(_, a)| is_nav(a)) {
            let (r, c) = nav_pos(hid_of(a)).unwrap();
            keys.push(VisualKey {
                index: *i,
                base: *a,
                x: nav_x + c,
                y: y_of(r),
                w: 1.0,
                h: 1.0,
            });
        }
        for (i, a) in present.iter().filter(|(_, a)| is_num(a)) {
            let (r, c, w, h) = num_pos(hid_of(a)).unwrap();
            keys.push(VisualKey {
                index: *i,
                base: *a,
                x: num_x + c,
                y: y_of(r),
                w,
                h,
            });
        }
        let has_numpad = present.iter().any(|(_, a)| is_num(a));
        for (n, (i, a)) in present.iter().filter(|(_, a)| is_extra(a)).enumerate() {
            keys.push(VisualKey {
                index: *i,
                base: *a,
                x: if has_numpad { num_x } else { nav_x + 3.25 } + n as f32,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            });
        }
    }

    // Drop an empty F-row (60/65% boards) so the board doesn't float.
    let min_y = keys.iter().map(|k| k.y).fold(f32::MAX, f32::min);
    for k in &mut keys {
        k.y -= min_y;
    }
    let width = keys.iter().map(|k| k.x + k.w).fold(0.0, f32::max);
    let height = keys.iter().map(|k| k.y + k.h).fold(0.0, f32::max);
    Layout {
        keys,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layout_is_sane() {
        assert!(all().len() >= 50, "{} models", all().len());
        for d in all() {
            let l = build_layout(&d.default_actions());
            assert!(
                l.keys.len() > 55,
                "{} has {} keys",
                d.display_name,
                l.keys.len()
            );
            assert!(
                l.width >= 14.0 && l.width <= 23.0,
                "{} width {}",
                d.display_name,
                l.width
            );
        }
    }

    #[test]
    fn fun60_pro_is_known() {
        let d = find(2304).unwrap();
        assert_eq!(d.display_name, "FUN60 PRO");
        assert_eq!(d.fn_layer_count(), 2);
        let c = d.color_modes.unwrap();
        assert_eq!((c.rainbow, c.fixed), (7, 8));
    }
}
