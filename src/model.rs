use std::fmt;

use anyhow::Context;
use serde::Deserialize;
use serde_with::{DeserializeFromStr, SerializeDisplay};

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

pub mod digital_khatt_aba {
    use super::*;

    // use String as VerseKey;

    #[derive(Debug, Deserialize)]
    pub struct Verse {
        pub verse_key: VerseKey,
        pub text: String,
        // pub script_type: String,
        // pub font_family: String,
        // pub words: Vec<Word>,
        // pub page_number: u16,
        // pub juz_number: u16,
        // pub hizb_number: u16,
    }

    #[derive(Debug, Deserialize)]
    pub struct Word {
        // pub position: u16,
        // pub text: String,
        // pub location: String,
    }

    pub type Db = std::collections::BTreeMap<VerseKey, Verse>;
}

pub fn arabicize(i: u16) -> String {
    i.to_string()
        .replace('0', "٠")
        .replace('1', "١")
        .replace('2', "٢")
        .replace('3', "٣")
        .replace('4', "٤")
        .replace('5', "٥")
        .replace('6', "٦")
        .replace('7', "٧")
        .replace('8', "٨")
        .replace('9', "٩")
}

pub struct AppState {
    pub db: digital_khatt_aba::Db,

    pub anchor_verse: VerseKey,
    pub jump_to_verse: Option<u16>,

    pub font_size: f32,

    /// In second/page
    pub scroll_speed: f64,

    pub is_scrolling: bool,
}
