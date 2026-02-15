use masonry::{layout::Length, parley};
use puthi::view::virtual_hscroll;
use xilem::{
    WidgetView,
    core::{Edit, MessageResult, ViewArgument},
    style::{Padding, Style},
    view::{FlexExt, FlexSpacer, flex_col, flex_row, slider, text_button},
};

use crate::model::{AppState, VerseKey, digital_khatt_aba::Verse};

mod label;
pub use label::*;

const DIGITALKHATT_NEW_MADINA: parley::FontStack<'_> = parley::FontStack::Single(
    parley::FontFamily::Named(std::borrow::Cow::Borrowed("DigitalKhatt New Madina")),
);

const LINE_HEIGHT_FACTOR: f32 = 1.8;

impl Verse {
    pub fn view<State: ViewArgument + 'static>(
        &self,
        font_size: f32,
    ) -> impl WidgetView<State> + use<State> {
        label(self.text.clone())
            .font(DIGITALKHATT_NEW_MADINA.clone())
            .text_size(font_size)
            .line_height(parley::LineHeight::FontSizeRelative(LINE_HEIGHT_FACTOR))
            .padding(Padding::left(font_size as f64 * 0.3))
    }
}

impl AppState {
    pub fn logic(state: &mut Self) -> impl WidgetView<Edit<Self>> + use<> {
        // font-size: 21.75px
        // line-height: 39.1167px
        // line-width: 381.117px

        let autoscroll_velocity =
            state.font_size as f64 * 263. / state.scroll_speed * state.is_scrolling as i8 as f64;
        flex_col((
            FlexSpacer::Flex(1.),
            virtual_hscroll(1..287, |state: &mut Self, idx| {
                state.db[&VerseKey {
                    ayah: idx as _,
                    ..state.anchor_verse
                }]
                    .view(state.font_size)
            })
            .left_to_right(false)
            .autoscroll_velocity(autoscroll_velocity)
            .jump_to(state.jump_to_verse.map(Into::into))
            .on_scroll(|state: &mut Self, std::ops::Range { start, .. }| {
                state.anchor_verse.ayah = start as _;
                state.jump_to_verse = None;
                MessageResult::Action(())
            })
            .height(Length::px((state.font_size * LINE_HEIGHT_FACTOR) as f64)),
            flex_col((
                FlexSpacer::Flex(1.),
                flex_row((
                    FlexSpacer::Flex(1.),
                    text_button("⏪", |state: &mut Self| {
                        state.jump_to_verse = Some((state.anchor_verse.ayah + 20).min(287));
                    }),
                    label(format!("{:1.1}", state.scroll_speed)),
                    slider(90., 270., state.scroll_speed, |state: &mut Self, s| {
                        state.scroll_speed = s;
                    }),
                    label(state.anchor_verse.to_string()),
                    text_button("⏯", |state: &mut Self| {
                        state.is_scrolling = !state.is_scrolling;
                    }),
                    label(format!("{:1.1}", state.font_size)),
                    slider(24., 64., state.font_size as _, |state: &mut Self, s| {
                        state.font_size = s as _;
                    }),
                    text_button("⏩", |state: &mut Self| {
                        state.jump_to_verse = Some((state.anchor_verse.ayah - 20).min(1));
                    }),
                    FlexSpacer::Flex(1.),
                )),
                FlexSpacer::Flex(1.),
            ))
            .flex(1.),
        ))
    }
}
