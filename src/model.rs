use serde::{Deserialize, Serialize};

#[path = "data.rs"]
pub mod data;

#[expect(dead_code)]
pub struct SurahInfo {
    pub name_ar: &'static str,
    pub name_en: &'static str,
    pub name_en_simple: &'static str,
    pub ayahs: u16,
    pub cumulative_ayahs: u16,
    pub revealed_in: PlaceOfRevelation,
}

#[derive(Clone, Copy)]
pub enum PlaceOfRevelation {
    Makkah,
    Madinah,
}

pub struct ScrollingReader {
    pub surah: u8,

    pub jump_to_ayah: Option<u16>,
    pub is_scrolling: bool,
}

impl ScrollingReader {
    pub fn ayah_text(&self, ayah: u16) -> &'static str {
        data::AYAHS[if ayah == 0 {
            0
        } else {
            (data::SURAHS[self.surah as usize].cumulative_ayahs + ayah) as usize
        }]
    }

    pub fn ayah_range(&self) -> std::ops::Range<i64> {
        (!self.has_basmalah() as _)..(data::SURAHS[self.surah as usize].ayahs as _)
    }

    pub fn has_basmalah(&self) -> bool {
        !matches!(self.surah, 1 | 9)
    }
}

pub struct AppState {
    pub user_data: UserData,

    pub reader: Option<(usize, ScrollingReader)>,
    pub showing_index: bool,
}

impl AppState {
    pub fn load(user_data: UserData) -> Self {
        AppState {
            user_data,

            reader: None,
            showing_index: true,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Progress {
    pub last_on: jiff::Timestamp,
    pub surah: u8,
    pub ayah: u16,
}

impl Progress {
    pub fn reader(self) -> ScrollingReader {
        ScrollingReader {
            surah: self.surah,

            jump_to_ayah: Some(self.ayah.saturating_sub(1)),
            is_scrolling: false,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Preferences {
    pub font_size: f32,

    /// In second/page
    pub scroll_speed: f64,
}

#[derive(Serialize, Deserialize)]
pub struct UserData {
    pub preferences: Preferences,
    pub progress: Vec<Progress>,
}

impl UserData {
    pub fn load_from_disk() -> anyhow::Result<Self> {
        Ok(toml::from_slice::<UserData>(&std::fs::read(
            "reading.toml",
        )?)?)
    }

    pub fn save(&self) {
        std::fs::write("reading.toml", toml::to_string_pretty(self).unwrap()).unwrap()
    }
}

#[test]
fn gen_sample_progress() {
    let sample_data = UserData {
        preferences: Preferences {
            font_size: 30.,
            scroll_speed: 180.,
        },
        progress: vec![
            Progress {
                last_on: jiff::Timestamp::now(),
                surah: 2,
                ayah: 69,
            },
            Progress {
                last_on: jiff::Timestamp::now(),
                surah: 54,
                ayah: 19,
            },
        ],
    };

    let sample_ser = toml::to_string_pretty(&sample_data).unwrap();
    eprintln!("{sample_ser}");
}
