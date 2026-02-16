use std::sync::Arc;

use xilem::{Blob, EventLoop, WindowOptions, Xilem};

mod model;
mod view;

pub mod widgets;

use crate::model::AppState;

fn main() -> anyhow::Result<()> {
    let state = AppState::load(model::UserData::load_from_disk()?);
    let app = Xilem::new_simple(state, AppState::logic, WindowOptions::new("Scrolled Quran"))
        .with_font(Blob::new(Arc::new(view::assets::DIGITALKHATT_NEW_MADINA)))
        .with_font(Blob::new(Arc::new(view::assets::SURAH_NAMES)));
    app.run_in(EventLoop::with_user_event())?;
    Ok(())
}
