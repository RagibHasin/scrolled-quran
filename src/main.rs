use std::fs;

use xilem::{EventLoop, WindowOptions, Xilem};

use crate::model::AppState;

mod model;
mod view;

fn main() -> anyhow::Result<()> {
    let state = AppState::load(model::UserData::load_from_disk()?);
    let app = Xilem::new_simple(state, AppState::logic, WindowOptions::new("Scrolled Quran"))
        .with_font(fs::read("assets/DigitalKhattV2.otf")?)
        .with_font(fs::read("assets/surah-name-v2.ttf")?);
    app.run_in(EventLoop::with_user_event())?;
    Ok(())
}
