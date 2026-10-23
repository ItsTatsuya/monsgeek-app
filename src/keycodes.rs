//! Labels for key-matrix actions and the remap palette.

use crate::protocol::KeyAction;

/// HID keyboard usage (page 0x07) → short label.
pub fn hid_label(hid: u8) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z",
    ];
    const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
    const F: [&str; 12] = [
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    ];
    const F13: [&str; 12] = [
        "F13", "F14", "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
    ];
    Some(match hid {
        0x04..=0x1d => LETTERS[(hid - 0x04) as usize],
        0x1e..=0x27 => DIGITS[(hid - 0x1e) as usize],
        0x3a..=0x45 => F[(hid - 0x3a) as usize],
        0x68..=0x73 => F13[(hid - 0x68) as usize],
        0x28 => "Enter",
        0x29 => "Esc",
        0x2a => "Backspace",
        0x2b => "Tab",
        0x2c => "Space",
        0x2d => "-",
        0x2e => "=",
        0x2f => "[",
        0x30 => "]",
        0x31 => "\\",
        0x32 => "#",
        0x33 => ";",
        0x34 => "'",
        0x35 => "`",
        0x36 => ",",
        0x37 => ".",
        0x38 => "/",
        0x39 => "Caps",
        0x46 => "PrtSc",
        0x47 => "ScrLk",
        0x48 => "Pause",
        0x49 => "Ins",
        0x4a => "Home",
        0x4b => "PgUp",
        0x4c => "Del",
        0x4d => "End",
        0x4e => "PgDn",
        0x4f => "→",
        0x50 => "←",
        0x51 => "↓",
        0x52 => "↑",
        0x53 => "Num",
        0x54 => "N/",
        0x55 => "N*",
        0x56 => "N-",
        0x57 => "N+",
        0x58 => "NEnt",
        0x59 => "N1",
        0x5a => "N2",
        0x5b => "N3",
        0x5c => "N4",
        0x5d => "N5",
        0x5e => "N6",
        0x5f => "N7",
        0x60 => "N8",
        0x61 => "N9",
        0x62 => "N0",
        0x63 => "N.",
        0x64 => "ISO \\",
        0x65 => "Menu",
        0xe0 => "Ctrl",
        0xe1 => "Shift",
        0xe2 => "Alt",
        0xe3 => "Win",
        0xe4 => "RCtrl",
        0xe5 => "RShift",
        0xe6 => "RAlt",
        0xe7 => "RWin",
        _ => return None,
    })
}

/// Consumer-page usages used by the firmware's media keys.
pub fn consumer_label(usage: u16) -> Option<&'static str> {
    Some(match usage {
        0xe2 => "Mute",
        0xe9 => "Vol+",
        0xea => "Vol-",
        0xcd => "Play",
        0xb5 => "Next",
        0xb6 => "Prev",
        0xb7 => "Stop",
        0x6f => "Bright+",
        0x70 => "Bright-",
        0x192 => "Calc",
        0x18a => "Mail",
        0x194 => "My PC",
        0x223 => "Web",
        0x221 => "Search",
        0x183 => "Media",
        _ => return None,
    })
}

/// Firmware functions (from the driver's function table and the FUN60 manual's Fn chart).
pub fn function_label(a: &KeyAction) -> Option<(&'static str, &'static str)> {
    Some(match *a {
        [10, 1, 0, 0] => ("Fn", "Fn key"),
        [10, 1, 1, 0] => ("R-Fn", "Right Fn key"),
        [10, 3, 0, 0] => ("Win lock", "Disable / enable the Windows key"),
        [10, 5, 3, 0] => ("Mac/Win", "Hold 3 s to switch between Mac and Windows mode"),
        [10, 11, 0, 0] => ("Fn layer", "Switch to the other Fn layer"),
        [10, 12, 0, 0] => ("Light ⏻", "Backlight on / off"),
        [13, 1, 0, 0] => ("Effect", "Next lighting effect"),
        [13, 2, 1, 0] => ("Bright +", "Backlight brighter"),
        [13, 2, 2, 0] => ("Bright −", "Backlight dimmer"),
        [13, 3, 1, 0] => ("Speed −", "Lighting effect slower"),
        [13, 3, 2, 0] => ("Speed +", "Lighting effect faster"),
        [13, 5, 0, 0] => ("Colour −", "Previous backlight colour"),
        [13, 5, 1, 0] => ("Colour +", "Cycle backlight colour"),
        [8, 0, 1, 0] => ("Profile +", "Next profile"),
        [8, 0, 2, 0] => ("Profile −", "Previous profile"),
        [8, 0, 4, 0] => ("Profile 1", "Switch to profile 1"),
        [8, 0, 4, 1] => ("Profile 2", "Switch to profile 2"),
        [8, 0, 4, 2] => ("Profile 3", "Switch to profile 3"),
        [8, 0, 4, 3] => ("Profile 4", "Switch to profile 4"),
        [1, 0, 240, 0] => ("LMB", "Left mouse button"),
        [1, 0, 241, 0] => ("RMB", "Right mouse button"),
        [1, 0, 242, 0] => ("MMB", "Middle mouse button"),
        _ => return None,
    })
}

const MODIFIERS: [&str; 8] = [
    "Ctrl", "Shift", "Alt", "Win", "RCtrl", "RShift", "RAlt", "RWin",
];

fn key_name(hid: u8) -> String {
    hid_label(hid).map(str::to_string).unwrap_or_else(|| {
        if hid == 0 {
            String::new()
        } else {
            format!("0x{hid:02x}")
        }
    })
}

/// Short label for a 4-byte key-matrix action.
pub fn action_label(a: &KeyAction) -> String {
    if *a == [0, 0, 0, 0] {
        return "✕".into();
    }
    if let Some((l, _)) = function_label(a) {
        return l.into();
    }
    match a[0] {
        0 => {
            let mut parts: Vec<String> = MODIFIERS
                .iter()
                .enumerate()
                .filter(|(i, _)| a[1] & (1 << i) != 0)
                .map(|(_, m)| m.to_string())
                .collect();
            parts.push(key_name(a[2]));
            if a[3] != 0 {
                parts.push(key_name(a[3]));
            }
            parts.retain(|p| !p.is_empty());
            parts.join("+")
        }
        3 => consumer_label(u16::from_le_bytes([a[2], a[3]]))
            .unwrap_or("Media")
            .into(),
        9 => "Macro".into(),
        _ => format!("Fn {}.{}", a[0], a[1]),
    }
}

/// Longer description for the Fn shortcut list.
pub fn action_description(a: &KeyAction) -> String {
    if let Some((_, d)) = function_label(a) {
        return d.into();
    }
    match a[0] {
        3 => consumer_label(u16::from_le_bytes([a[2], a[3]]))
            .unwrap_or("Media key")
            .into(),
        0 => action_label(a),
        _ => format!("Firmware function {}.{}.{}.{}", a[0], a[1], a[2], a[3]),
    }
}

/// The HID key that types `ch` on a US layout, and whether Shift is needed.
pub fn char_to_hid(ch: char) -> Option<(u8, bool)> {
    const PLAIN: &[(char, u8)] = &[
        (' ', 0x2c),
        ('-', 0x2d),
        ('=', 0x2e),
        ('[', 0x2f),
        (']', 0x30),
        ('\\', 0x31),
        (';', 0x33),
        ('\'', 0x34),
        ('`', 0x35),
        (',', 0x36),
        ('.', 0x37),
        ('/', 0x38),
        ('0', 0x27),
        ('\n', 0x28),
        ('\t', 0x2b),
    ];
    const SHIFTED: &[(char, char)] = &[
        ('!', '1'),
        ('@', '2'),
        ('#', '3'),
        ('$', '4'),
        ('%', '5'),
        ('^', '6'),
        ('&', '7'),
        ('*', '8'),
        ('(', '9'),
        (')', '0'),
        ('_', '-'),
        ('+', '='),
        ('{', '['),
        ('}', ']'),
        ('|', '\\'),
        (':', ';'),
        ('"', '\''),
        ('~', '`'),
        ('<', ','),
        ('>', '.'),
        ('?', '/'),
    ];
    match ch {
        'a'..='z' => Some((0x04 + (ch as u8 - b'a'), false)),
        'A'..='Z' => Some((0x04 + (ch as u8 - b'A'), true)),
        '1'..='9' => Some((0x1e + (ch as u8 - b'1'), false)),
        _ => PLAIN
            .iter()
            .find(|(c, _)| *c == ch)
            .map(|&(_, h)| (h, false))
            .or_else(|| {
                let base = SHIFTED.iter().find(|(c, _)| *c == ch)?.1;
                Some((char_to_hid(base)?.0, true))
            }),
    }
}

/// Label for a usage stored in a macro (keyboard keys and mouse buttons).
pub fn macro_key_label(usage: u8) -> String {
    match usage {
        240..=244 => ["LMB", "RMB", "MMB", "Mouse 4", "Mouse 5"][(usage - 240) as usize].into(),
        _ => key_name(usage),
    }
}

pub struct PaletteGroup {
    pub name: &'static str,
    pub keys: Vec<(String, KeyAction)>,
}

pub fn palette() -> Vec<PaletteGroup> {
    let hid = |codes: &[u8]| {
        codes
            .iter()
            .map(|&c| (key_name(c), [0, 0, c, 0]))
            .collect::<Vec<_>>()
    };
    let range = |a: u8, b: u8| (a..=b).collect::<Vec<u8>>();
    let mut numbers = range(0x1e, 0x27);
    numbers.extend_from_slice(&[
        0x2d, 0x2e, 0x2f, 0x30, 0x31, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x64,
    ]);
    let mut function = range(0x3a, 0x45);
    function.extend(range(0x68, 0x73));
    let consumer = [
        0xcd, 0xb6, 0xb5, 0xb7, 0xe2, 0xe9, 0xea, 0x6f, 0x70, 0x192, 0x223, 0x194u16,
    ];
    let functions: Vec<KeyAction> = vec![
        [10, 3, 0, 0],
        [10, 11, 0, 0],
        [10, 12, 0, 0],
        [13, 1, 0, 0],
        [13, 2, 1, 0],
        [13, 2, 2, 0],
        [13, 3, 1, 0],
        [13, 3, 2, 0],
        [13, 5, 1, 0],
        [8, 0, 4, 0],
        [8, 0, 4, 1],
        [8, 0, 4, 2],
        [8, 0, 4, 3],
    ];
    vec![
        PaletteGroup {
            name: "Letters",
            keys: hid(&range(0x04, 0x1d)),
        },
        PaletteGroup {
            name: "Numbers & symbols",
            keys: hid(&numbers),
        },
        PaletteGroup {
            name: "Function",
            keys: hid(&function),
        },
        PaletteGroup {
            name: "Navigation",
            keys: hid(&[
                0x29, 0x2b, 0x39, 0x28, 0x2a, 0x2c, 0x49, 0x4c, 0x4a, 0x4d, 0x4b, 0x4e, 0x52, 0x51,
                0x50, 0x4f, 0x46, 0x47, 0x48, 0x65,
            ]),
        },
        PaletteGroup {
            name: "Modifiers",
            keys: hid(&range(0xe0, 0xe7)),
        },
        PaletteGroup {
            name: "Media",
            keys: consumer
                .iter()
                .map(|&u| {
                    (
                        consumer_label(u).unwrap().to_string(),
                        [3, 0, u as u8, (u >> 8) as u8],
                    )
                })
                .collect(),
        },
        PaletteGroup {
            name: "Keyboard",
            keys: functions.iter().map(|a| (action_label(a), *a)).collect(),
        },
        PaletteGroup {
            name: "Special",
            keys: vec![
                ("Disable".into(), [0, 0, 0, 0]),
                ("Fn".into(), [10, 1, 0, 0]),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(action_label(&[13, 2, 1, 0]), "Bright +");
        assert_eq!(action_label(&[0, 0, 224, 82]), "Ctrl+↑");
        assert_eq!(action_label(&[0, 1, 0x06, 0]), "Ctrl+C");
        assert_eq!(action_label(&[3, 0, 0xe9, 0]), "Vol+");
        assert_eq!(action_label(&[0, 0, 0, 0]), "✕");
    }
}
