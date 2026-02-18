use std::sync::Arc;

use xilem::{Blob, EventLoopBuilder, WindowOptions, Xilem};

mod model;
mod view;

pub mod widgets;

use crate::model::AppState;

pub use model::USER_DATA_PATH;

pub fn run(event_loop: EventLoopBuilder) -> Result<(), anyhow::Error> {
    let app = Xilem::new_simple(
        AppState::load(model::UserData::load_from_disk()?),
        AppState::logic,
        WindowOptions::new("Scrolled Quran"),
    )
    .with_font(Blob::new(Arc::new(view::assets::DIGITALKHATT_NEW_MADINA)))
    .with_font(Blob::new(Arc::new(view::assets::SURAH_NAMES)));
    app.run_in(event_loop)?;
    Ok(())
}
