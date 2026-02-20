use std::borrow::Cow;

use masonry::layout::{Dim, Length};
use masonry::parley::{self, FontFamily, FontStack};
use masonry::properties::{Dimensions, Gap};
use xilem::core::{MessageResult, View};
use xilem::style::{self, Style as _};
use xilem::view::{
    FlexExt, FlexSpacer, GridExt, GridItem, MainAxisAlignment, button, flex_col, flex_row, grid,
    indexed_stack, label, portal, prose, resize_observer, slider, text_button,
};
use xilem::{TextAlign, WidgetView};

#[allow(unused)]
mod virtual_hscroll;
pub use virtual_hscroll::*;

use crate::model::{self, AppState};

pub mod assets {
    pub const DIGITALKHATT_NEW_MADINA: &[u8] = include_bytes!("../assets/DigitalKhattV2.otf");
    pub const SURAH_NAMES: &[u8] = include_bytes!("../assets/surah-name-v2.ttf");
    pub const ABOUT_TEXT: &str = include_str!("about.txt");
}

const DIGITALKHATT_NEW_MADINA: FontStack<'_> =
    FontStack::Single(FontFamily::Named(Cow::Borrowed("DigitalKhatt New Madina")));
const SURAH_NAMES: FontStack<'_> =
    FontStack::Single(FontFamily::Named(Cow::Borrowed("surah-name-v2")));

const LINE_HEIGHT_FACTOR: f32 = 2.;

const GAP: Length = Length::const_px(5.);

impl AppState {
    pub fn logic(&mut self) -> impl WidgetView<Self> + use<> {
        indexed_stack((
            self.index_view(),
            Self::about_view(),
            self.reader.as_mut().map(|(idx, reader_state)| {
                reader_state
                    .view(&self.user_data.preferences, self.user_data.progress[*idx])
                    .map_state(|state: &mut Self| state.reader.as_mut().unwrap())
                    // ReaderAction::Save => state.user_data.save().unwrap(),
                    .map_action(|state: &mut Self, action| match action {
                        ReaderAction::SetAyah(ayah) => {
                            state.selected_progress_mut().unwrap().set_ayah(ayah);
                            state.user_data.save().unwrap()
                        }
                        ReaderAction::SetScrollSpeed(s) => {
                            state.user_data.preferences.scroll_speed = s;
                            state.user_data.save().unwrap()
                        }
                        ReaderAction::SetFontSize(s) => {
                            state.user_data.preferences.font_size = s;
                            state.user_data.save().unwrap()
                        }
                        ReaderAction::Close => state.page = model::Page::Index,
                        ReaderAction::None => {}
                    })
            }),
        ))
        .active(match self.page {
            model::Page::Index => 0,
            model::Page::About => 1,
            model::Page::Reader if self.reader.is_none() => 0,
            model::Page::Reader => 2,
        })
    }

    fn about_view() -> impl WidgetView<Self> {
        flex_col((
            flex_row((
                flex_row(text_button("◀", |state: &mut Self| {
                    state.page = model::Page::Index
                }))
                .flex(1.),
                label("Scrolled Quran")
                    .text_size(24.)
                    .weight(xilem::FontWeight::BOLD),
                FlexSpacer::Flex(1.),
            )),
            portal(prose(assets::ABOUT_TEXT).text_alignment(TextAlign::Center))
                .constrain_horizontal(true),
        ))
        .gap(GAP)
        .padding(GAP.get())
    }

    fn index_view(&self) -> impl WidgetView<Self> + use<> {
        const MIN_CARD_WIDTH: f64 = 250.;
        let n_columns = ((self.viewport_width / MIN_CARD_WIDTH) as i32).max(1);
        let n_progress_rows = 2i32.max(-(-3i32.div_euclid(n_columns)));

        resize_observer(
            Box::new(|state: &mut Self, masonry::kurbo::Size { width, .. }| {
                state.viewport_width = width
            }),
            portal(
                flex_col((
                    flex_row((
                        label("Recently read"),
                        text_button("ℹ️", |state: &mut Self| state.page = model::Page::About),
                    ))
                    .main_axis_alignment(MainAxisAlignment::SpaceBetween),
                    grid(
                        self.user_data.progress_cards(n_columns, n_progress_rows),
                        n_columns,
                        n_progress_rows,
                    )
                    .gap(GAP),
                    flex_row(label("All surahs")),
                    grid(
                        Self::surah_cards(n_columns),
                        n_columns,
                        -(-114i32.div_euclid(n_columns)),
                    )
                    .gap(GAP),
                ))
                .gap(GAP)
                .width(Dim::Stretch),
            )
            .constrain_horizontal(true)
            .padding(GAP.get()),
        )
        .dims(Dimensions::MAX)
    }

    fn surah_cards(n_columns: i32) -> [GridItem<impl WidgetView<Self> + use<>, Self, ()>; 114] {
        std::array::from_fn(|i| {
            let surah = (i + 1) as _;
            surah_card(
                surah,
                Box::new(move |state: &mut Self| {
                    let new_progress = model::Progress::new(surah);
                    state.set_reader(state.user_data.progress.len(), new_progress);
                    state.user_data.progress.push(new_progress);
                }),
            )
            .boxed()
            .grid_pos(i as i32 % n_columns, i as i32 / n_columns)
        })
    }
}

impl model::UserData {
    fn progress_cards(
        &self,
        n_columns: i32,
        n_progress_rows: i32,
    ) -> Vec<GridItem<impl WidgetView<AppState> + use<>, AppState, ()>> {
        let mut progress = self
            .progress
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();
        progress.sort_unstable_by_key(|(_, p)| p.last_on());
        progress
            .into_iter()
            .enumerate()
            .take((n_progress_rows * n_columns) as usize)
            .map(|(display_idx, (i, progress))| {
                progress
                    .view(Box::new(move |state: &mut AppState| {
                        state.set_reader(i, progress);
                    }))
                    .grid_pos(
                        display_idx as i32 % n_columns,
                        display_idx as i32 / n_columns,
                    )
            })
            .collect()
    }
}

impl model::Progress {
    fn view<State: 'static, F: Fn(&mut State) + Send + Sync + 'static>(
        self,
        callback: F,
    ) -> impl WidgetView<State> + use<State, F> {
        generic_surah_card(self.surah(), format!("At ayah {}", self.ayah()), callback)
    }
}

fn surah_card<State: 'static, F: Fn(&mut State) + Send + Sync + 'static>(
    surah: u8,
    callback: F,
) -> impl WidgetView<State> {
    generic_surah_card(
        surah,
        format!("{} Ayahs", model::data::SURAHS[surah as usize].ayahs),
        callback,
    )
}

fn generic_surah_card<State: 'static, F: Fn(&mut State) + Send + Sync + 'static>(
    surah: u8,
    text: String,
    callback: F,
) -> impl WidgetView<State> {
    button(
        flex_row((
            label(surah.to_string()).text_size(28.),
            flex_col((
                label(format!("surah{surah:03}"))
                    .font(SURAH_NAMES)
                    .text_size(32.),
                label(model::data::SURAHS[surah as usize].name_en),
            ))
            .gap(Gap::ZERO),
            label(text).text_alignment(TextAlign::End),
        ))
        .main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .gap(Gap::ZERO),
        callback,
    )
}

type ReaderState = (usize, model::ScrollingReader);

pub enum ReaderAction {
    SetAyah(u16),
    SetScrollSpeed(f64),
    SetFontSize(f32),
    Close,
    None,
}

impl model::ScrollingReader {
    fn ayah_view<State: 'static, Action: 'static>(
        &self,
        ayah: u16,
    ) -> impl WidgetView<State, Action> + use<State, Action> {
        label(self.ayah_text(ayah))
            .font(DIGITALKHATT_NEW_MADINA.clone())
            .text_size(self.font_size)
            .enable_hinting(false)
            .line_height(parley::LineHeight::FontSizeRelative(LINE_HEIGHT_FACTOR))
            .padding(style::Padding::left(self.font_size as f64 * 0.3))
    }

    fn view(
        &self,
        pref: &model::Preferences,
        progress: model::Progress,
    ) -> impl WidgetView<ReaderState, ReaderAction> + use<> {
        // font-size: 21.75px
        // line-height: 39.1167px
        // line-width: 381.117px

        let info = flex_col((flex_row((
            flex_row(text_button("◀", |_| ReaderAction::Close)).flex(1.),
            label(format!("surah{:03}", self.surah))
                .font(SURAH_NAMES)
                .text_size(32.),
            label(progress.ayah().to_string())
                .text_alignment(TextAlign::End)
                .flex(1.),
        )),))
        .flex(1.);

        let ayah_range = self.ayah_range();
        let controls = flex_col((
            slider(
                ayah_range.start as _,
                (ayah_range.end - 1) as _,
                progress.ayah() as _,
                |(_, state): &mut ReaderState, i| {
                    let ayah = i as _;
                    state.jump_to_ayah = Some(ayah);
                    ReaderAction::SetAyah(ayah)
                },
            )
            .step(1.),
            flex_row((
                flex_row((
                    label(format!("{:1.1}", pref.scroll_speed)),
                    slider(90., 270., pref.scroll_speed, |_, s| {
                        ReaderAction::SetScrollSpeed(s)
                    })
                    .step(0.5)
                    .flex(1.),
                ))
                .flex(1.),
                text_button("⏯", |(_, state): &mut ReaderState| {
                    state.is_scrolling = !state.is_scrolling;
                    ReaderAction::None
                }),
                flex_row((
                    slider(
                        24.,
                        64.,
                        pref.font_size as _,
                        |(_, state): &mut ReaderState, s| {
                            state.font_size = s as _;
                            ReaderAction::SetFontSize(state.font_size)
                        },
                    )
                    .step(0.5)
                    .flex(1.),
                    label(format!("{:1.1}", pref.font_size)),
                ))
                .flex(1.),
            )),
        ))
        .main_axis_alignment(MainAxisAlignment::End)
        .flex(1.);

        const PAGE_FONTSIZE_RATIO: f64 = 262.5;
        let autoscroll_velocity = pref.font_size as f64 * PAGE_FONTSIZE_RATIO / pref.scroll_speed
            * self.is_scrolling as i8 as f64;

        flex_col((
            info,
            virtual_hscroll(ayah_range, |(_, state): &mut ReaderState, ayah| {
                state.ayah_view(ayah as _)
            })
            .left_to_right(false)
            .autoscroll_velocity(autoscroll_velocity)
            .jump_to(self.jump_to_ayah.map(Into::into))
            .on_scroll(
                |(_, state): &mut ReaderState, std::ops::Range { start, .. }| {
                    state.jump_to_ayah = None;
                    MessageResult::Action(ReaderAction::SetAyah(start as _))
                },
            )
            .height(Length::px((pref.font_size * LINE_HEIGHT_FACTOR) as f64)),
            controls,
        ))
        .padding(16.)
    }
}
