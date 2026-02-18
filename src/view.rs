use std::borrow::Cow;

use masonry::parley::{self, FontFamily, FontStack};
use masonry::properties::{Dimensions, Gap};
use masonry::{
    layout::{Dim, Length},
    properties::ThumbRadius,
};
use xilem::core::{Arg, Edit, MessageResult, View, ViewArgument};
use xilem::style::{Padding, Style};
use xilem::view::{
    FlexExt, GridExt, GridItem, MainAxisAlignment, button, flex_col, flex_row, grid, indexed_stack,
    resize_observer, slider, text_button,
};
use xilem::{TextAlign, WidgetView};

#[allow(unused)]
mod label;
pub use label::*;

#[allow(unused)]
mod portal;
pub use portal::*;

#[allow(unused)]
mod virtual_hscroll;
pub use virtual_hscroll::*;

use crate::model::{self, AppState};

pub mod assets {
    pub const DIGITALKHATT_NEW_MADINA: &[u8] = include_bytes!("../assets/DigitalKhattV2.otf");
    pub const SURAH_NAMES: &[u8] = include_bytes!("../assets/surah-name-v2.ttf");
}

const DIGITALKHATT_NEW_MADINA: FontStack<'_> =
    FontStack::Single(FontFamily::Named(Cow::Borrowed("DigitalKhatt New Madina")));
const SURAH_NAMES: FontStack<'_> =
    FontStack::Single(FontFamily::Named(Cow::Borrowed("surah-name-v2")));

const LINE_HEIGHT_FACTOR: f32 = 2.;

impl AppState {
    pub fn logic(&mut self) -> impl WidgetView<Edit<Self>> + use<> {
        indexed_stack((
            self.index_view(),
            self.reader.as_mut().map(|(idx, reader_state)| {
                reader_state
                    .view(&self.user_data.preferences, self.user_data.progress[*idx])
                    .map_state(|state: &mut Self, ()| {
                        let (idx, reader_state) = state.reader.as_mut().unwrap();
                        (
                            reader_state,
                            &mut state.user_data.preferences,
                            &mut state.user_data.progress[*idx],
                        )
                    })
                    .map_action(|state: &mut Self, action| match action {
                        ReaderAction::Save => state.user_data.save().unwrap(),
                        ReaderAction::Close => state.showing_index = true,
                        ReaderAction::None => {}
                    })
            }),
        ))
        .active(if self.reader.is_some() && !self.showing_index {
            1
        } else {
            0
        })
    }

    fn index_view(&self) -> impl WidgetView<Edit<Self>> + use<> {
        const MIN_CARD_WIDTH: f64 = 250.;
        let n_columns = ((self.viewport_width / MIN_CARD_WIDTH) as i32).max(1);
        let n_progress_rows = 2i32.max(-(-3i32.div_euclid(n_columns)));

        const GAP: Length = Length::const_px(5.);
        resize_observer(
            Box::new(|state: &mut Self, masonry::kurbo::Size { width, .. }| {
                state.viewport_width = width
            }),
            portal(
                flex_col((
                    grid(
                        self.user_data.progress_cards(n_columns, n_progress_rows),
                        n_columns,
                        n_progress_rows,
                    )
                    .gap(GAP),
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

    fn surah_cards(
        n_columns: i32,
    ) -> [GridItem<impl WidgetView<Edit<Self>> + use<>, Edit<Self>, ()>; 114] {
        std::array::from_fn(|i| {
            let surah = (i + 1) as _;
            surah_card(
                surah,
                Box::new(move |state: &mut Self| {
                    let new_progress = model::Progress::new(surah);
                    state.reader = Some((state.user_data.progress.len(), new_progress.reader()));
                    state.showing_index = false;
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
    ) -> Vec<GridItem<impl WidgetView<Edit<AppState>> + use<>, Edit<AppState>, ()>> {
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
                        state.reader = Some((i, progress.reader()));
                        state.showing_index = false;
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
    fn view<State: ViewArgument, F: Fn(Arg<'_, State>) + Send + Sync + 'static>(
        self,
        callback: F,
    ) -> impl WidgetView<State> + use<State, F> {
        generic_surah_card(self.surah(), format!("At ayah {}", self.ayah()), callback)
    }
}

fn surah_card<State: ViewArgument, F: Fn(Arg<'_, State>) + Send + Sync + 'static>(
    surah: u8,
    callback: F,
) -> impl WidgetView<State> {
    generic_surah_card(
        surah,
        format!("{} Ayahs", model::data::SURAHS[surah as usize].ayahs),
        callback,
    )
}

fn generic_surah_card<State: ViewArgument, F: Fn(Arg<'_, State>) + Send + Sync + 'static>(
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

type ReaderState = (
    Edit<model::ScrollingReader>,
    Edit<model::Preferences>,
    Edit<model::Progress>,
);

pub enum ReaderAction {
    Save,
    Close,
    None,
}

impl model::ScrollingReader {
    fn ayah_view<State: ViewArgument, Action: 'static>(
        &self,
        ayah: u16,
        font_size: f32,
    ) -> impl WidgetView<State, Action> + use<State, Action> {
        label(self.ayah_text(ayah))
            .font(DIGITALKHATT_NEW_MADINA.clone())
            .text_size(font_size)
            .enable_hinting(false)
            .line_height(parley::LineHeight::FontSizeRelative(LINE_HEIGHT_FACTOR))
            .padding(Padding::left(font_size as f64 * 0.3))
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
                |(state, _, progress): Arg<ReaderState>, i| {
                    let ayah = i as _;
                    progress.set_ayah(ayah);
                    state.jump_to_ayah = Some(ayah);
                    ReaderAction::Save
                },
            )
            .step(1.)
            .prop(ThumbRadius(3.)),
            flex_row((
                flex_row((
                    label(format!("{:1.1}", pref.scroll_speed)),
                    slider(
                        90.,
                        270.,
                        pref.scroll_speed,
                        |(_, pref, _): Arg<ReaderState>, s| {
                            pref.scroll_speed = s;
                            ReaderAction::Save
                        },
                    )
                    .step(0.5)
                    .flex(1.),
                ))
                .flex(1.),
                text_button("⏯", |(state, ..): Arg<ReaderState>| {
                    state.is_scrolling = !state.is_scrolling;
                    ReaderAction::None
                }),
                flex_row((
                    slider(
                        24.,
                        64.,
                        pref.font_size as _,
                        |(_, pref, _): Arg<ReaderState>, s| {
                            pref.font_size = s as _;
                            ReaderAction::Save
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
            virtual_hscroll(ayah_range, |(state, pref, _): Arg<ReaderState>, ayah| {
                state.ayah_view(ayah as _, pref.font_size)
            })
            .left_to_right(false)
            .autoscroll_velocity(autoscroll_velocity)
            .jump_to(self.jump_to_ayah.map(Into::into))
            .on_scroll(
                |(state, _, progress): Arg<ReaderState>, std::ops::Range { start, .. }| {
                    progress.set_ayah(start as _);
                    state.jump_to_ayah = None;
                    MessageResult::Action(ReaderAction::Save)
                },
            )
            .height(Length::px((pref.font_size * LINE_HEIGHT_FACTOR) as f64)),
            controls,
        ))
        .padding(16.)
    }
}
