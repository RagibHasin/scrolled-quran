use std::borrow::Cow;

use masonry::layout::{Dim, Length};
use masonry::parley::{self, FontFamily, FontStack};
use xilem::WidgetView;
use xilem::core::{Arg, Edit, MessageResult, View, ViewArgument};
use xilem::style::{Padding, Style};
use xilem::view::{
    FlexExt, FlexSpacer, GridExt, button, flex_col, flex_row, grid, indexed_stack, slider,
    text_button,
};

mod label;
pub use label::*;

mod portal;
pub use portal::*;

mod virtual_hscroll;
pub use virtual_hscroll::*;

use crate::model;

pub mod assets {
    pub const DIGITALKHATT_NEW_MADINA: &[u8] = include_bytes!("../assets/DigitalKhattV2.otf");
    pub const SURAH_NAMES: &[u8] = include_bytes!("../assets/surah-name-v2.ttf");
}

const DIGITALKHATT_NEW_MADINA: FontStack<'_> =
    FontStack::Single(FontFamily::Named(Cow::Borrowed("DigitalKhatt New Madina")));
const SURAH_NAMES: FontStack<'_> =
    FontStack::Single(FontFamily::Named(Cow::Borrowed("surah-name-v2")));

const LINE_HEIGHT_FACTOR: f32 = 1.8;

impl model::AppState {
    pub fn logic(&mut self) -> impl WidgetView<Edit<Self>> + use<> {
        indexed_stack((
            self.index_view(),
            self.reader.as_mut().map(|(_, reader_state)| {
                reader_state
                    .view(&self.user_data.preferences)
                    .map_state(|state: &mut Self, ()| {
                        let (idx, reader_state) = state.reader.as_mut().unwrap();
                        (
                            reader_state,
                            &mut state.user_data.preferences,
                            &mut state.user_data.progress[*idx],
                        )
                    })
                    .map_action(|state: &mut Self, ()| state.user_data.save())
            }),
        ))
        .active(if self.reader.is_some() && !self.showing_index {
            1
        } else {
            0
        })
    }

    pub fn index_view(&mut self) -> impl WidgetView<Edit<Self>> + use<> {
        let n_columns = 6;

        let progress = self
            .user_data
            .progress
            .iter()
            .copied()
            .enumerate()
            .map(|(i, progress)| {
                progress
                    .view(Box::new(move |state: &mut Self| {
                        state.reader = Some((i, progress.reader()));
                        state.showing_index = false;
                    }))
                    .grid_pos(i as i32 % n_columns, i as i32 / n_columns)
            })
            .collect::<Vec<_>>();

        let surah_cards: [_; 114] = std::array::from_fn(|i| {
            surah_card(
                (i + 1) as _,
                Box::new(move |state: &mut Self| {
                    let new_progress = model::Progress {
                        last_on: jiff::Timestamp::now(),
                        surah: i as _,
                        ayah: 0,
                    };
                    state.reader = Some((state.user_data.progress.len(), new_progress.reader()));
                    state.showing_index = false;
                    state.user_data.progress.push(new_progress);
                }),
            )
            .boxed()
            .grid_pos(i as i32 % n_columns, i as i32 / n_columns)
        });

        const GAP: Length = Length::const_px(5.);
        portal(
            flex_col((
                grid(progress, n_columns, 1).gap(GAP),
                grid(surah_cards, n_columns, -(-114i32.div_euclid(n_columns)))
                    .gap(GAP)
                    .flex(1.),
            ))
            .gap(GAP)
            .width(Dim::Stretch),
        )
        .constrain_horizontal(true)
        .padding(GAP.get())
    }
}

impl model::Progress {
    pub fn view<State: ViewArgument, F: Fn(Arg<'_, State>) + Send + Sync + 'static>(
        self,
        callback: F,
    ) -> impl WidgetView<State> + use<State, F> {
        button(
            flex_row((
                flex_col((
                    label(format!("surah{:03}", self.surah))
                        .font(SURAH_NAMES)
                        .text_size(30.)
                        .flex(1.),
                    label(model::data::SURAHS[self.surah as usize].name_en).flex(1.),
                )),
                label(format!("At ayah {}", self.ayah))
                    .text_alignment(xilem::TextAlign::End)
                    .flex(1.),
            )),
            callback,
        )
    }
}

pub fn surah_card<State: ViewArgument, F: Fn(Arg<'_, State>) + Send + Sync + 'static>(
    surah: u8,
    callback: F,
) -> impl WidgetView<State> {
    button(
        flex_row((
            label(surah.to_string())
                .text_size(24.)
                .width(Length::px(60.)),
            flex_col((
                label(format!("surah{surah:03}"))
                    .font(SURAH_NAMES)
                    .text_size(30.)
                    .flex(1.),
                label(model::data::SURAHS[surah as usize].name_en).flex(1.),
            )),
            label(format!(
                "{} Ayahs",
                model::data::SURAHS[surah as usize].ayahs
            ))
            .text_alignment(xilem::TextAlign::End)
            .flex(1.),
        )),
        callback,
    )
}

type ReaderState = (
    Edit<model::ScrollingReader>,
    Edit<model::Preferences>,
    Edit<model::Progress>,
);

impl model::ScrollingReader {
    pub fn ayah_view<State: ViewArgument>(
        &self,
        ayah: u16,
        font_size: f32,
    ) -> impl WidgetView<State> + use<State> {
        label(self.ayah_text(ayah))
            .font(DIGITALKHATT_NEW_MADINA.clone())
            .text_size(font_size)
            .enable_hinting(false)
            .line_height(parley::LineHeight::FontSizeRelative(LINE_HEIGHT_FACTOR))
            .padding(Padding::left(font_size as f64 * 0.3))
    }

    pub fn view(&mut self, pref: &model::Preferences) -> impl WidgetView<ReaderState> + use<> {
        // font-size: 21.75px
        // line-height: 39.1167px
        // line-width: 381.117px

        let autoscroll_velocity =
            pref.font_size as f64 * 263. / pref.scroll_speed * self.is_scrolling as i8 as f64;
        flex_col((
            FlexSpacer::Flex(1.),
            virtual_hscroll(
                self.ayah_range(),
                |(state, pref, _): Arg<ReaderState>, ayah| {
                    state.ayah_view(ayah as _, pref.font_size)
                },
            )
            .left_to_right(false)
            .autoscroll_velocity(autoscroll_velocity)
            .jump_to(self.jump_to_ayah.map(Into::into))
            .on_scroll(
                |(state, _, progress): Arg<ReaderState>, std::ops::Range { start, .. }| {
                    progress.ayah = start as _;
                    state.jump_to_ayah = None;
                    MessageResult::Action(())
                },
            )
            .height(Length::px((pref.font_size * LINE_HEIGHT_FACTOR) as f64)),
            flex_col((
                FlexSpacer::Flex(1.),
                flex_row((
                    FlexSpacer::Flex(1.),
                    label(format!("{:1.1}", pref.scroll_speed)),
                    slider(
                        90.,
                        270.,
                        pref.scroll_speed,
                        |(_, pref, _): Arg<ReaderState>, s| {
                            pref.scroll_speed = s;
                        },
                    ),
                    text_button("⏯", |(state, ..): Arg<ReaderState>| {
                        state.is_scrolling = !state.is_scrolling;
                    }),
                    label(format!("{:1.1}", pref.font_size)),
                    slider(
                        24.,
                        64.,
                        pref.font_size as _,
                        |(_, pref, _): Arg<ReaderState>, s| {
                            pref.font_size = s as _;
                        },
                    ),
                    FlexSpacer::Flex(1.),
                )),
                FlexSpacer::Flex(1.),
            ))
            .flex(1.),
        ))
    }
}
