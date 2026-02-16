use std::fmt;

use anyhow::Context;
use serde_with::{DeserializeFromStr, SerializeDisplay};

#[path = "flatbuffers/gen/ayahlist_generated.rs"]
mod ayahlist_fbs;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, SerializeDisplay, DeserializeFromStr,
)]
pub struct VerseKey {
    pub surah: u8,
    pub ayah: u16,
}

impl fmt::Display for VerseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.surah, self.ayah)
    }
}

impl std::str::FromStr for VerseKey {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (surah, ayah) = s
            .split_once(':')
            .ok_or(anyhow::format_err!("expected `surah_no:ayah_no`"))?;
        Ok(VerseKey {
            surah: surah.parse().with_context(|| format!("input: {surah}"))?,
            ayah: ayah.parse().with_context(|| format!("input: {ayah}"))?,
        })
    }
}

const STARTING_AYAH: [u16; 115] = [
    0, 0, 7, 293, 493, 669, 789, 954, 1160, 1235, 1364, 1473, 1596, 1707, 1750, 1802, 1901, 2029,
    2140, 2250, 2348, 2483, 2595, 2673, 2791, 2855, 2932, 3159, 3252, 3340, 3409, 3469, 3503, 3533,
    3606, 3660, 3705, 3788, 3970, 4058, 4133, 4218, 4272, 4325, 4414, 4473, 4510, 4545, 4583, 4612,
    4630, 4675, 4735, 4784, 4846, 4901, 4979, 5075, 5104, 5126, 5150, 5163, 5177, 5188, 5199, 5217,
    5229, 5241, 5271, 5323, 5375, 5419, 5447, 5475, 5495, 5551, 5591, 5622, 5672, 5712, 5758, 5800,
    5829, 5848, 5884, 5909, 5931, 5948, 5967, 5993, 6023, 6043, 6058, 6079, 6090, 6098, 6106, 6125,
    6130, 6138, 6146, 6157, 6168, 6176, 6179, 6188, 6193, 6197, 6204, 6207, 6213, 6216, 6221, 6225,
    6230,
];

pub struct AppState {
    pub data: &'static [u8],
    pub all_verses: flatbuffers::Vector<'static, flatbuffers::ForwardsUOffset<&'static str>>,

    pub anchor_verse: VerseKey,
    pub jump_to_verse: Option<u16>,

    pub font_size: f32,

    /// In second/page
    pub scroll_speed: f64,

    pub is_scrolling: bool,
}

impl AppState {
    pub fn load_ayahs(data: Vec<u8>) -> Self {
        let data = data.leak();
        let all_verses = ayahlist_fbs::root_as_ayah_list(data)
            .unwrap()
            .ayahs()
            .unwrap();
        AppState {
            data,
            all_verses,
            anchor_verse: VerseKey { surah: 2, ayah: 1 },
            jump_to_verse: None,
            font_size: 36.,
            scroll_speed: 180.,
            is_scrolling: true,
        }
    }

    pub fn ayah_text(&self, surah: u8, ayah: u16) -> &'static str {
        self.all_verses
            .get((STARTING_AYAH[surah as usize] + ayah - 1) as _)
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        let _ = unsafe { Box::from_raw(self.data as *const [u8] as *mut [u8]) };
    }
}
