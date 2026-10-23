//! Per-key colours ("user pictures"), from the driver's _setLightPic / _getLightPic.
//!
//! A picture is 3 bytes (R, G, B) per matrix index.
//!   SET_USERPIC [0x0C, picture, 0xFF, page, length, last, 0, chk] + 56 bytes, 7 pages (126 keys)
//!   GET_USERPIC [0x8C, picture, 0xFF, page], 6 pages of 64 bytes (128 keys)
//! Lighting effect 13 shows a picture; LEDPARAM's option nibble picks which one.

use super::{cmd, Checksum, KeyboardClient, Lighting, MATRIX_KEYS, PAGE_SIZE};
use anyhow::Result;
use std::time::Duration;

pub type Rgb = [u8; 3];

/// LightUserPicture in the driver's LightList.
pub const USER_PICTURE_EFFECT: u8 = 13;
const SET_BYTES: usize = 378;
const GET_PAGES: u8 = 6;

// Every MonsGeek keyboard offers 5 pictures, except these older boards with 3.
const THREE_PICTURES: [u32; 8] = [969, 983, 1200, 1215, 1308, 1419, 1674, 1933];

pub fn picture_count(device_id: u32) -> u8 {
    if THREE_PICTURES.contains(&device_id) {
        3
    } else {
        5
    }
}

/// Lighting that shows `picture`. Like the driver's showPictureFrame, the speed byte is 0
/// (speed 4 here, since the byte is 4 - speed).
pub fn picture_lighting(current: Lighting, picture: u8) -> Lighting {
    Lighting {
        effect: USER_PICTURE_EFFECT,
        option: picture,
        speed: 4,
        brightness: if current.brightness == 0 { 4 } else { current.brightness },
        ..current
    }
}

pub fn decode_picture(bytes: &[u8]) -> Vec<Rgb> {
    (0..MATRIX_KEYS)
        .map(|i| std::array::from_fn(|c| bytes.get(i * 3 + c).copied().unwrap_or(0)))
        .collect()
}

pub fn picture_frames(picture: u8, colors: &[Rgb]) -> Vec<Vec<u8>> {
    let mut bytes = vec![0u8; SET_BYTES];
    for (i, c) in colors.iter().enumerate() {
        if i * 3 + 2 < SET_BYTES {
            bytes[i * 3..i * 3 + 3].copy_from_slice(c);
        }
    }
    let pages = SET_BYTES.div_ceil(PAGE_SIZE);
    (0..pages)
        .map(|p| {
            let chunk = &bytes[p * PAGE_SIZE..((p + 1) * PAGE_SIZE).min(SET_BYTES)];
            let mut f = vec![
                cmd::SET_USERPIC,
                picture,
                0xff,
                p as u8,
                chunk.len() as u8,
                (p == pages - 1) as u8,
                0,
                0,
            ];
            f.extend_from_slice(chunk);
            f.resize(8 + PAGE_SIZE, 0);
            f
        })
        .collect()
}

impl KeyboardClient {
    pub fn get_picture(&mut self, picture: u8) -> Result<Vec<Rgb>> {
        let bytes =
            self.query_pages(|p| vec![cmd::GET_USERPIC, picture, 0xff, p], GET_PAGES)?;
        Ok(decode_picture(&bytes))
    }

    pub fn set_picture(&mut self, picture: u8, colors: &[Rgb]) -> Result<()> {
        self.write(
            &picture_frames(picture, colors),
            Checksum::Bit7,
            Duration::from_millis(300),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::mock::MockTransport;

    #[test]
    fn pictures_round_trip_on_the_mock() {
        let def = crate::devices::find(2304).unwrap();
        let mut kb = KeyboardClient::new(Box::new(MockTransport::new(def)));
        let mut colors = vec![[0u8; 3]; MATRIX_KEYS];
        colors[0] = [255, 0, 0];
        colors[70] = [1, 2, 3];
        colors[125] = [9, 9, 9];
        kb.set_picture(2, &colors).unwrap();
        let read = kb.get_picture(2).unwrap();
        assert_eq!(read[0], [255, 0, 0]);
        assert_eq!(read[70], [1, 2, 3]);
        assert_eq!(read[125], [9, 9, 9]);
        assert_eq!(kb.get_picture(1).unwrap()[0], [0, 0, 0]);
    }

    #[test]
    fn showing_a_picture_matches_the_driver_frame() {
        let current = Lighting {
            effect: 2,
            speed: 1,
            brightness: 3,
            option: 0,
            rainbow: true,
            color: [10, 20, 30],
        };
        let l = picture_lighting(current, 2);
        let f = crate::protocol::encode_lighting(&l, crate::protocol::ColorModes::default());
        assert_eq!(f, vec![cmd::SET_LEDPARAM, USER_PICTURE_EFFECT, 0, 3, 2 << 4, 0, 200, 200]);
        assert_eq!(picture_lighting(Lighting { brightness: 0, ..current }, 0).brightness, 4);
    }
}
