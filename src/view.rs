use std::borrow::Cow;

use masonry::{
    layout::{Dim, Length},
    parley::{self, FontFamily, FontStack},
};
use puthi::view::virtual_hscroll;
use xilem::{
    WidgetView,
    core::{Arg, Edit, MessageResult, View, ViewArgument},
    style::{Padding, Style},
    view::{
        FlexExt, FlexSpacer, GridExt, button, flex_col, flex_row, grid, indexed_stack, portal,
        slider, text_button,
    },
};

mod label;
pub use label::*;

use crate::model;

const DIGITALKHATT_NEW_MADINA: FontStack<'_> =
    FontStack::Single(FontFamily::Named(Cow::Borrowed("DigitalKhatt New Madina")));
const SURAH_NAMES: FontStack<'_> =
    FontStack::Single(FontFamily::Named(Cow::Borrowed("surah-name-v2")));

const LINE_HEIGHT_FACTOR: f32 = 1.8;

type ReaderWithPref = (Edit<model::ScrollingReader>, Edit<model::Preferences>);

impl model::ScrollingReader {
    pub fn verse_view<State: ViewArgument>(
        &self,
        ayah: u16,
        font_size: f32,
    ) -> impl WidgetView<State> + use<State> {
        label(self.ayah_text(ayah))
            .font(DIGITALKHATT_NEW_MADINA.clone())
            .text_size(font_size)
            .line_height(parley::LineHeight::FontSizeRelative(LINE_HEIGHT_FACTOR))
            .padding(Padding::left(font_size as f64 * 0.3))
    }

    pub fn view(&mut self, pref: &model::Preferences) -> impl WidgetView<ReaderWithPref> + use<> {
        // font-size: 21.75px
        // line-height: 39.1167px
        // line-width: 381.117px

        let autoscroll_velocity =
            pref.font_size as f64 * 263. / pref.scroll_speed * self.is_scrolling as i8 as f64;
        flex_col((
            FlexSpacer::Flex(1.),
            virtual_hscroll(
                self.ayah_range(),
                |(state, pref): Arg<ReaderWithPref>, ayah| {
                    state.verse_view(ayah as _, pref.font_size)
                },
            )
            .left_to_right(false)
            .autoscroll_velocity(autoscroll_velocity)
            .jump_to(self.jump_to_ayah.map(Into::into))
            .on_scroll(
                |(state, _): Arg<ReaderWithPref>, std::ops::Range { start, .. }| {
                    state.anchor_ayah = start as _;
                    state.jump_to_ayah = None;
                    MessageResult::Action(())
                },
            )
            .height(Length::px((pref.font_size * LINE_HEIGHT_FACTOR) as f64)),
            flex_col((
                FlexSpacer::Flex(1.),
                flex_row((
                    FlexSpacer::Flex(1.),
                    text_button("⏪", |(state, _): Arg<ReaderWithPref>| {
                        state.jump_to_ayah = Some((state.anchor_ayah + 20).min(287));
                    }),
                    label(format!("{:1.1}", pref.scroll_speed)),
                    slider(
                        90.,
                        270.,
                        pref.scroll_speed,
                        |(_, pref): Arg<ReaderWithPref>, s| {
                            pref.scroll_speed = s;
                        },
                    ),
                    // label(state.anchor_verse.to_string()),
                    text_button("⏯", |(state, _): Arg<ReaderWithPref>| {
                        state.is_scrolling = !state.is_scrolling;
                    }),
                    label(format!("{:1.1}", pref.font_size)),
                    slider(
                        24.,
                        64.,
                        pref.font_size as _,
                        |(_, pref): Arg<ReaderWithPref>, s| {
                            pref.font_size = s as _;
                        },
                    ),
                    text_button("⏩", |(state, _): Arg<ReaderWithPref>| {
                        state.jump_to_ayah = Some((state.anchor_ayah - 20).min(1));
                    }),
                    FlexSpacer::Flex(1.),
                )),
                FlexSpacer::Flex(1.),
            ))
            .flex(1.),
        ))
    }
}

impl model::AppState {
    pub fn index_view(state: &mut Self) -> impl WidgetView<Edit<Self>> + use<> {
        let n_columns = 6;

        let progress = state
            .user_data
            .progress
            .iter()
            .enumerate()
            .map(|(i, p)| {
                p.view()
                    .grid_pos(i as i32 % n_columns, i as i32 / n_columns)
            })
            .collect::<Vec<_>>();

        let surah_cards: [_; 114] = std::array::from_fn(|i| {
            surah_card(
                (i + 1) as _,
                Box::new(move |state: &mut Self| {
                    state.reader = Some(model::ScrollingReader::at(i as _, 1));
                    state.showing_index = false;
                }),
            )
            .boxed()
            .grid_pos(i as i32 % n_columns, i as i32 / n_columns)
        });

        portal(
            flex_col((
                grid(progress, n_columns, 1),
                grid(surah_cards, n_columns, -(-114i32.div_euclid(n_columns))).flex(1.),
            ))
            .width(Dim::Stretch),
        )
    }

    pub fn logic(&mut self) -> impl WidgetView<Edit<Self>> + use<> {
        indexed_stack((
            Self::index_view(self),
            self.reader.as_mut().map(|r| {
                r.view(&self.user_data.preferences)
                    .map_state(|state: &mut Self, ()| {
                        (
                            state.reader.as_mut().unwrap(),
                            &mut state.user_data.preferences,
                        )
                    })
            }),
        ))
        .active(if self.reader.is_some() && !self.showing_index {
            1
        } else {
            0
        })
    }
}

impl model::Progress {
    pub fn view<State: ViewArgument>(&self) -> impl WidgetView<State> + use<State> {
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
        ))
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
