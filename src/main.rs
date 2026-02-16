use std::fs;

use xilem::{EventLoop, WindowOptions, Xilem};

mod model;
mod view;

fn main() -> anyhow::Result<()> {
    let state = model::AppState::load_ayahs(fs::read("assets/digital-khatt-v2.aba.bin")?);
    let app = Xilem::new_simple(
        state,
        model::AppState::logic,
        WindowOptions::new("Scrolled Quran"),
    )
    .with_font(fs::read("assets/DigitalKhattV2.otf")?);
    app.run_in(EventLoop::with_user_event())?;
    Ok(())
}
