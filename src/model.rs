use std::sync::OnceLock;

use jiff::Timestamp;
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

pub fn surah_needs_basmalah(surah: u8) -> bool {
    !matches!(surah, 1 | 9)
}

pub fn page_of(surah: u8, ayah: u16) -> usize {
    if ayah == 0 {
        page_of(surah, 1)
    } else {
        match data::FIRST_AYAHS.binary_search(&(surah, ayah)) {
            Ok(p) => p + 1,
            Err(p) => p,
        }
    }
}

#[derive(Clone, Copy)]
pub enum PlaceOfRevelation {
    Makkah,
    Madinah,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Progress {
    last_on: Timestamp,
    surah: u8,
    ayah: u16,
}

impl Progress {
    pub fn new(surah: u8) -> Self {
        Progress {
            last_on: Timestamp::now(),
            surah,
            ayah: if surah_needs_basmalah(surah) { 0 } else { 1 },
        }
    }

    pub fn reader(self) -> ScrollingReader {
        let mut r = ScrollingReader {
            surah: self.surah,

            jump_to_ayah_index: None,
            is_scrolling: false,
        };
        r.jump_to_ayah_index = Some(r.ayah_to_index(self.ayah));
        r
    }

    pub fn last_on(&self) -> Timestamp {
        self.last_on
    }

    pub fn surah(&self) -> u8 {
        self.surah
    }

    pub fn ayah(&self) -> u16 {
        self.ayah
    }

    pub fn set_ayah(&mut self, ayah: u16) {
        self.ayah = ayah;
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

pub static USER_DATA_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();

impl UserData {
    pub fn save_path() -> &'static std::path::Path {
        USER_DATA_PATH
            .get_or_init(|| "reading.toml".into())
            .as_path()
    }

    pub fn load_from_disk() -> anyhow::Result<Self> {
        const NEW: UserData = UserData {
            preferences: Preferences {
                font_size: 36.,
                scroll_speed: 180.,
            },
            progress: Vec::new(),
        };

        let save_path = Self::save_path();
        if save_path.exists() {
            Ok(toml::from_slice::<UserData>(&std::fs::read(save_path)?)?)
        } else {
            NEW.save()?;
            Ok(NEW)
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        Ok(std::fs::write(
            Self::save_path(),
            toml::to_string_pretty(self)?,
        )?)
    }
}

pub struct ScrollingReader {
    pub surah: u8,

    pub jump_to_ayah_index: Option<usize>,
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

    pub fn is_ayah_on_page_boundary(&self, ayah: u16) -> Option<usize> {
        if ayah == 0 {
            self.is_ayah_on_page_boundary(1)
        } else {
            data::FIRST_AYAHS
                .binary_search(&(self.surah, ayah))
                .map(|p| p + 1)
                .ok()
        }
    }

    pub fn ayah_range(&self) -> std::ops::RangeInclusive<u16> {
        (!surah_needs_basmalah(self.surah) as _)..=data::SURAHS[self.surah as usize].ayahs
    }

    pub fn ayahs_count(&self) -> usize {
        data::SURAHS[self.surah as usize].ayahs as usize + surah_needs_basmalah(self.surah) as usize
    }

    pub fn index_to_ayah(&self, index: usize) -> u16 {
        (if surah_needs_basmalah(self.surah) {
            index
        } else {
            index + 1
        }) as _
    }

    pub fn ayah_to_index(&self, ayah: u16) -> usize {
        (if surah_needs_basmalah(self.surah) {
            ayah
        } else {
            ayah - 1
        }) as _
    }
}

pub enum Page {
    Index,
    About,
    Reader,
}

pub struct AppState {
    pub user_data: UserData,

    pub viewport_width: f64,
    pub page: Page,
    pub reader: Option<(usize, ScrollingReader)>,
}

impl AppState {
    pub fn load(user_data: UserData) -> Self {
        AppState {
            user_data,

            viewport_width: 1000.,
            page: Page::Index,
            reader: None,
        }
    }

    pub fn set_reader(&mut self, idx: usize, progress: Progress) {
        self.reader = Some((idx, progress.reader()));
        self.page = Page::Reader;
    }

    pub fn selected_progress_mut(&mut self) -> Option<&mut Progress> {
        self.reader
            .as_ref()
            .map(|(i, _)| &mut self.user_data.progress[*i])
    }
}
