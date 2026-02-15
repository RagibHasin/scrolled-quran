use std::fs;

use xilem::{EventLoop, WindowOptions, Xilem};

mod model;
mod view;

fn main() -> anyhow::Result<()> {
    let state = model::AppState {
        db: serde_json::from_reader(fs::File::open("assets/digital-khatt-v2.aba.json")?)?,
        anchor_verse: model::VerseKey { surah: 2, ayah: 1 },
        jump_to_verse: None,
        font_size: 36.,
        scroll_speed: 180.,
        is_scrolling: true,
    };
    let app = Xilem::new_simple(
        state,
        model::AppState::logic,
        WindowOptions::new("Scrolled Quran"),
    )
    .with_font(fs::read("assets/DigitalKhattV2.otf")?);
    app.run_in(EventLoop::with_user_event())?;
    Ok(())
}
