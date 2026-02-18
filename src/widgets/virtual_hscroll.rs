// Copyright 2025 the Xilem Authors and the Druid Authors
// SPDX-License-Identifier: Apache-2.0

#![warn(missing_docs)]

use std::collections::HashMap;
use std::ops::Range;

use xilem::dpi::PhysicalPosition;

use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, ChildrenIds, ComposeCtx, EventCtx, KeyboardEvent, LayoutCtx,
    MeasureCtx, NewWidget, PaintCtx, PointerEvent, PointerScrollEvent, PropertiesMut,
    PropertiesRef, RegisterCtx, TextEvent, Update, UpdateCtx, Widget, WidgetMut, WidgetPod,
};
use masonry::kurbo::{Axis, Point, Size, Vec2};
use masonry::layout::{LenDef, LenReq, SizeDef};
use masonry::util::debug_panic;
use masonry::widgets::VirtualScrollAction;

#[expect(missing_docs)]
#[derive(Debug)]
pub enum VirtualHScrollAction {
    ActiveRange(VirtualScrollAction),
    VisibleRange(Range<i64>),
}

/// A (vertical) virtual scrolling widget.
///
/// Virtual scrolling is a technique to improve performance when scrolling through long lists, by
/// only loading (and therefore laying out, drawing, processing for event handling), the items visible to the user.
///
/// Each child of the virtual scroll widget has a signed 64 bit id (i.e. an `i64`), and items are laid out
/// in order of these ids.
/// The widget keeps track of which of these ids are loaded, and requests that more are loaded.
/// The widget requires these ids to be dense (that is, if it has a child with ids 1 and 3, it must have a child
/// with id 2).
///
/// This widget works in close coordinate with the [driver](crate::doc::creating_app#the-driver) to
/// load the children; that is, the driver must provide the children when requested.
/// See [usage](#usage) for more details.
///
/// The Masonry example `virtual_fizzbuzz` shows how to use this widget.
/// It creates an infinitely explorable implementation of the game [Fizz buzz](https://en.wikipedia.org/wiki/Fizz_buzz).
///
/// # Usage
///
/// When you create the virtual scroll, you specify the initial "anchor"; that is an id for which the item will be on-screen.
/// If only a subset of ids are valid, then the valid range of ids widget *must* be set.
///
/// The widget will send a [`VirtualScrollAction`] whenever the children it requires to be loaded (the active children) changes.
/// To handle this, the driver must [add](Self::add_child) widgets for the ids which are in `target` but not in
/// `old_active`, and [remove](Self::remove_child) those which are in `old_active` but not in `target`.
/// (`VirtualScroll` does not remove the children itself to enable cleanup by the driver before the
/// children get removed).
/// You also need to call [`VirtualHScroll::will_handle_action`] with this action, which allows the
/// `VirtualScroll` controller to know which children it expects to be valid. This avoids issues caused by
/// things going out of sync.
/// The docs for [`VirtualScrollAction`] include an example demonstrating this.
///
/// It is invalid to not provide all items requested.
/// For items which have not yet loaded, you should either:
/// 1) Provide a placeholder
/// 2) Restrict the valid range to exclude them
///
/// This widget avoids panicking and infinite loops in these cases, but this widget is not designed to
/// handle them, and so arbitrarily janky behaviour may occur.
///
/// As a special case, it is not possible to have an item with id [`i64::MAX`].
/// This is because of the internal use of exclusive ranges.
///
/// # Caveats
///
/// This widget has been developed as an minimum viable solution, and so there are a number of known issues with it.
/// These are discussed below.
///
/// ## Transforms
///
/// Widgets can be [transformed](WidgetMut::set_transform) arbitrarily from where their parent lays them out.
/// This interacts poorly with virtual scrolling, because an item which would be visible due to its
/// transform can be devirtualised, as its layout rectangle is far enough off-screen.
/// Currently, the virtual scrolling controller ignores this case.
/// The long term plan is for each child to be clipped to a reasonable range around itself.
/// The details of how large this clipping area will be have not been decided.
///
/// This will mean that once this is done, the behaviour with transformed widgets will be consistent but not
/// necessarily intuitive (that is, for a given row on screen, the displayed content will always be the same,
/// but some widgets with transforms might not be visible - in the worst case, completely hidden).
// TODO: Implement this.
///
/// ## Focus
///
/// Currently, this widget does not correctly handle focused child widgets.
/// This means that if (for example) the user is typing in a text box in a virtual scroll, and scrolls down,
/// continuing to type will stop working.
///
/// ## Accessibility
///
/// A proper virtual scrolling list needs accessibility support (such as for scrolling, but
/// also to ensure that focus does not get trapped, that the correct set of items are reported,
/// if/that there are more items following, etc.).
///
/// This widget currently exposes basic scrolling semantics (such as `scroll_x` and
/// `ScrollUp`/`ScrollDown` actions) and handles those actions. However, full accessibility
/// behavior for a virtualized list has not yet been designed, and will be a follow-up.
///
/// ## Scrollbars
///
/// There is not yet any integration with scrollbars for this widget.
/// This is planned; however there is no universally correct scrollbar implementation for virtual scrolling.
/// This widget will support user-provided scrollbar types, through some yet-to-be-determined mechanism.
/// There will also be provided implementations of reasonable scrollbar kinds.
///
/// ## Scroll Gestures
///
/// Like [`Portal`](crate::widgets::Portal), this widget does not handle scroll gestures (i.e. with
/// touch screens).
///
/// # Valid range
///
/// Scrolling at the end of the valid range is locked, however it is not currently supported to lock scrolling
/// such that the bottom of the last item cannot be above the bottom of the `VirtualScroll`.
/// That is, it is always possible to scroll past the loaded items to the background (if the user
/// reaches the end of the valid range).
///
/// If the valid range is empty, i.e. the start and the end are equal, then there is jank which we haven't
/// resolved. However, this case should not cause crashes.
pub struct VirtualHScroll {
    /// The range of items in the "id" space which are able to be used.
    ///
    /// This is used to cap scrolling; items outside of this range will never be loaded[^1][^2][^3].
    /// For example, in an email program, this would be `[id_of_most_recent_email]..=[id_of_oldest_email]`
    /// (note that the id of the oldest email might not be known; as soon as it is known, `id_of_oldest_email`
    /// can be set).
    ///
    /// The default is `i64::MIN..i64::MAX`. Note that this *is* exclusive of the item with id `i64::MAX`.
    /// That additional item being missing allows for using half-open ranges in all of this code,
    /// which makes our lives much easier.
    ///
    /// [^1]: The exact interaction with a "drag down to refresh" feature has not been scrutinised.
    /// [^2]: Currently, we lock the bottom of the range to the bottom of the final item. This should be configurable.
    /// [^3]: Behaviour when the range is shrunk to something containing the active range has not been considered.
    valid_range: Range<i64>,

    /// The range in the id space which is "active", i.e. which the virtual scrolling has decided
    /// are in the range of the viewport and should be shown on screen.
    /// Note that `items` is not necessarily dense in these; that is, if an
    /// item has not been provided by the application, we don't fall over.
    /// This is still an invalid state, but we handle it as well as we can.
    active_range: Range<i64>,
    /// Whether the most recent request we sent out was handled.
    /// If it hasn't been handled, we won't send a new one.
    action_handled: bool,

    /// The range in the id space which is "visible" in the viewport.
    visible_range: Range<i64>,

    /// All children of the virtual scroller.
    items: HashMap<i64, WidgetPod<dyn Widget>>,
    /// Widths of all children of the virtual scroller.
    placed_items: HashMap<i64, (f64, f64)>,
    // TODO: Handle focus even if the focused item scrolls off-screen.
    // TODO: Maybe this should be the focused items and its two neighbours, so tab focusing works?
    // focused_item: Option<(i64, WidgetPod<dyn Widget>)>,

    // Question: For a given scroll position, should the anchor always be the same?
    // Answer: Let's say yes for now, and re-evaluate if it becomes necessary.
    //  - Reason to not have this is that it adds some potential worst-case performance issues if scrolling up/down
    anchor_index: i64,
    /// The amount the user has scrolled from the anchor point, in logical pixels.
    scroll_offset_from_anchor: f64,

    /// The average width of items, determined experimentally.
    /// This is used if there are no items to determine the mean item width otherwise. This approach means:
    /// 1) For the easy case where every item is the same width (e.g. email), we get the right answer
    /// 2) For slightly harder cases, we get as sensible a result as is reasonable, without requiring a complex API
    ///    to get the needed information.
    mean_item_width: f64,

    /// The width of the current anchor.
    /// Used to determine if scrolling will require a relayout (because the anchor will have changed if the user has scrolled past it).
    anchor_width: f64,

    left_to_right: bool,

    autoscroll_velocity: f64,

    /// We don't want to spam warnings about not being dense, but we want the user to be aware of it.
    warned_not_dense: bool,
    /// We don't want to spam warnings about missing an action, but we want the user to be aware of it.
    missed_actions_count: u32,
}

impl std::fmt::Debug for VirtualHScroll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualScroll")
            .field("valid_range", &self.valid_range)
            .field("active_range", &self.active_range)
            .field("action_handled", &self.action_handled)
            .field("missed_action_count", &self.missed_actions_count)
            .field("items", &self.items.keys().collect::<Vec<_>>())
            .field("anchor_index", &self.anchor_index)
            .field("scroll_offset_from_anchor", &self.scroll_offset_from_anchor)
            .field("mean_item_width", &self.mean_item_width)
            .field("anchor_width", &self.anchor_width)
            .field("left_to_right", &self.left_to_right)
            .field("autoscroll_velocity", &self.autoscroll_velocity)
            .field("warned_not_dense", &self.warned_not_dense)
            .finish()
    }
}

// --- MARK: BUILDERS
impl VirtualHScroll {
    /// Creates a new virtual scrolling list.
    ///
    /// The item at `initial_anchor` will have its top aligned with the top of
    /// the scroll area to start with.
    ///
    /// Note that it is not possible to add children before the widget is "live".
    /// This is for simplicity, as the set of the children which should be loaded has
    /// not yet been determined.
    pub fn new(initial_anchor: i64) -> Self {
        Self {
            valid_range: i64::MIN..i64::MAX,
            // This range starts intentionally empty, as no items have been loaded.
            active_range: initial_anchor..initial_anchor,
            visible_range: initial_anchor..initial_anchor,
            action_handled: true,
            missed_actions_count: 0,
            items: HashMap::default(),
            placed_items: HashMap::default(),
            anchor_index: initial_anchor,
            scroll_offset_from_anchor: 0.0,
            mean_item_width: DEFAULT_MEAN_ITEM_WIDTH,
            left_to_right: true,
            autoscroll_velocity: 0.,
            anchor_width: DEFAULT_MEAN_ITEM_WIDTH,
            warned_not_dense: false,
        }
    }

    /// Sets the range of child ids which are valid.
    ///
    /// Note that this is a half-open range, so the end id of the range is not valid.
    ///
    /// # Panics
    ///
    /// If `valid_range.start >= valid_range.end`.
    /// Note that other empty ranges are fine, although the exact behaviour hasn't been carefully validated.
    #[track_caller]
    pub fn with_valid_range(mut self, valid_range: Range<i64>) -> Self {
        self.valid_range = valid_range;
        self.validate_valid_range();
        self
    }

    /// Sets the direction in which children are laid out.
    pub fn with_direction(mut self, left_to_right: bool) -> Self {
        self.left_to_right = left_to_right;
        self
    }

    /// Sets the auto-scroll velocity.
    pub fn with_autoscroll_velocity(mut self, autoscroll_velocity: f64) -> Self {
        self.autoscroll_velocity = autoscroll_velocity;
        self
    }
}

// --- MARK: METHODS
impl VirtualHScroll {
    /// The number of currently active children in this widget.
    ///
    /// This is intended for sanity-checking of higher-level processes (i.e. so that inconsistencies can be caught early).
    #[expect(
        clippy::len_without_is_empty,
        reason = "The only time the VirtualScroll unloads all children is when given an empty valid range."
    )]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    fn validate_valid_range(&mut self) {
        if self.valid_range.end < self.valid_range.start {
            debug_panic!(
                "Expected valid range to not have end less than its start, got {:?}",
                self.valid_range
            );
            // In release mode, we don't want this to take down the program;
            // an empty range is supported.
            self.valid_range = self.valid_range.start..self.valid_range.start;
        }
    }

    /// Ensures that the correct follow-up passes are requested after the scroll position changes.
    ///
    /// `size` is the current viewport's size.
    fn post_scroll(&mut self, size: Size) -> PostScrollResult {
        // We only lock scrolling if we're *exactly* at the end of the range, because
        // if the valid range has changed "during" an active scroll, we still want to handle
        // that scroll (specifically, in case it happens to scroll us back into the active
        // range "naturally")
        if self.anchor_index + 1 == self.valid_range.end {
            self.cap_scroll_range_end(self.anchor_width, size.width);
        }
        if self.anchor_index == self.valid_range.start {
            self.cap_scroll_range_start();
        }
        if self.scroll_offset_from_anchor < 0.
            || self.scroll_offset_from_anchor >= self.anchor_width
        {
            PostScrollResult::Layout
        } else {
            PostScrollResult::NoLayout
        }
    }

    /// A wrapper to use [`post_scroll`](Self::post_scroll) in event methods.
    fn event_post_scroll(&mut self, ctx: &mut EventCtx<'_>) {
        match self.post_scroll(ctx.content_box_size()) {
            PostScrollResult::Layout => ctx.request_layout(),
            PostScrollResult::NoLayout => {}
        }
        ctx.request_compose();
    }

    /// A wrapper to use [`post_scroll`](Self::post_scroll) in update methods.
    fn update_post_scroll(&mut self, ctx: &mut UpdateCtx<'_>) {
        match self.post_scroll(ctx.content_box_size()) {
            PostScrollResult::Layout => {
                ctx.request_layout();
            }
            PostScrollResult::NoLayout => {}
        }
        ctx.request_compose();
    }

    /// Locks scrolling so that:
    /// 1) Every part of the last valid item can be seen.
    /// 2) The last item never scrolls completely out of view (currently, the bottom of the last item can be halfway down the screen)
    ///
    /// Ideally, this would be configurable (so that e.g. the bottom of the last item aligns with
    /// the bottom of the viewport), but that requires more care, since it effectively changes what the last valid anchor is.
    fn cap_scroll_range_end(&mut self, anchor_width: f64, viewport_width: f64) {
        let max_scroll = (anchor_width - viewport_width / 2.).max(0.0);
        self.scroll_offset_from_anchor = self.scroll_offset_from_anchor.min(max_scroll);
    }

    /// Locks scrolling so that the top of the first valid item doesn't go above the top of the virtual scrolling area.
    fn cap_scroll_range_start(&mut self) {
        self.scroll_offset_from_anchor = self.scroll_offset_from_anchor.max(0.0);
    }

    fn direction_appropriate(&mut self, delta: f64) -> f64 {
        if self.left_to_right { delta } else { -delta }
    }
}

enum PostScrollResult {
    Layout,
    NoLayout,
}

// --- MARK: WIDGETMUT
impl VirtualHScroll {
    /// Indicates that `action` is about to be handled by the driver (which is calling this method).
    ///
    /// This is required because if multiple actions stack up, `VirtualScroll` would assume that they have all been handled.
    /// In particular, this method existing allows layout operations to happen after each individual action is handled, which
    /// achieves several things:
    /// - It improves robustness, by allowing layout methods to know exactly which indices are valid.
    /// - It makes writing drivers easier, as the safety rails in `VirtualScroll` can be more precise.
    // (It also simplifies writing tests)
    // TODO: This could instead take ownership of the action, and return some kind of `{to_remove, to_add}` iterator index pair.
    pub fn will_handle_action(this: &mut WidgetMut<'_, Self>, action: &VirtualScrollAction) {
        if this.widget.active_range != action.old_active {
            debug_panic!(
                "Handling a VirtualScrollAction with the wrong range; got {:?}, expected {:?} for widget {}.\n\
                Maybe this has been routed to the wrong `VirtualHScroll`?",
                action.old_active,
                this.widget.active_range,
                this.ctx.widget_id(),
            );
        }
        this.widget.action_handled = true;
        if this.widget.missed_actions_count > 0 {
            // Avoid spamming the "handling single action delay" warning.
            this.widget.missed_actions_count = 1;
        }
        this.widget.active_range = action.target.clone();
        this.ctx.request_layout();
    }

    /// Add the child widget for the given index.
    ///
    /// This should be done only in the handling of a [`VirtualScrollAction`].
    /// This must be called after [`VirtualHScroll::will_handle_action`].
    #[track_caller]
    pub fn add_child(this: &mut WidgetMut<'_, Self>, idx: i64, child: NewWidget<dyn Widget>) {
        // TODO: Maybe just warn?
        debug_assert!(
            this.widget.action_handled,
            "You must call `will_handle_action` before `add_child`."
        );
        debug_assert!(
            this.widget.active_range.contains(&idx),
            "`add_child` should only be called with an index requested by the controller."
        );
        this.ctx.children_changed();
        if this.widget.items.insert(idx, child.to_pod()).is_some() {
            tracing::warn!("Tried to add child {idx} twice to VirtualScroll");
        };
    }

    /// Removes the child widget with id `idx`.
    ///
    /// This will log an error if there was no child at the given index.
    /// This should only happen if the driver does not meet the usage contract.
    ///
    /// This should be done only in the handling of a [`VirtualScrollAction`].
    /// This must be called after [`VirtualHScroll::will_handle_action`].
    ///
    /// Note that if you are changing the valid range, you should *not* remove any active children
    /// outside of that range; instead the controller will send an action removing those children.
    #[track_caller]
    pub fn remove_child(this: &mut WidgetMut<'_, Self>, idx: i64) {
        // TODO: Maybe just warn?
        debug_assert!(
            this.widget.action_handled,
            "You must call `will_handle_action` before `remove_child`."
        );
        debug_assert!(
            !this.widget.active_range.contains(&idx),
            "`remove_child` should only be called with an index which is not active."
        );
        let child = this.widget.items.remove(&idx);
        if let Some(child) = child {
            this.ctx.remove_child(child);
        } else if !this.widget.warned_not_dense {
            // If we have already warned because there's a density problem, don't duplicate it with this error.
            tracing::error!(
                "Tried to remove child ({idx}) which has already been removed or was never added."
            );
        }
    }

    /// Returns mutable reference to the child widget at `idx`.
    ///
    /// # Panics
    ///
    /// If the widget at `idx` is not in the scroll area.
    #[track_caller]
    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>, idx: i64) -> WidgetMut<'t, dyn Widget> {
        let child = this.widget.items.get_mut(&idx).unwrap_or_else(|| {
            panic!(
                "`VirtualHScroll::child_mut` called with non-present index {idx}.\n\
                Active range is {:?}.",
                &this.widget.active_range
            )
        });

        this.ctx.get_mut(child)
    }

    /// Sets the valid range of ids.
    ///
    /// That is, the children which the virtual scrolling area will request within.
    /// Runtime equivalent of [`with_valid_range`](Self::with_valid_range).
    ///
    /// # Panics
    ///
    /// If `valid_range.start >= valid_range.end`.
    /// Note that other empty ranges are fine, although the exact behaviour hasn't been carefully validated.
    pub fn set_valid_range(this: &mut WidgetMut<'_, Self>, range: Range<i64>) {
        this.widget.valid_range = range;
        this.widget.validate_valid_range();
        this.ctx.request_layout();
    }

    /// Sets the direction in which children are laid out.
    pub fn set_direction(this: &mut WidgetMut<'_, Self>, left_to_right: bool) {
        this.widget.left_to_right = left_to_right;
        this.ctx.request_layout();
    }

    /// Sets the auto-scroll velocity.
    pub fn set_autoscroll_velocity(this: &mut WidgetMut<'_, Self>, autoscroll_velocity: f64) {
        this.widget.autoscroll_velocity = autoscroll_velocity;
        this.ctx.request_anim_frame();
    }

    /// Forcefully aligns the top of the item at `idx` with the top of the
    /// virtual scroll area.
    ///
    /// That is, scroll to the item at `idx`, losing any scroll progress by the user.
    ///
    /// This method is mostly useful for tests, but can be used outside of tests
    /// (for example, in certain scrollbar schemes).
    pub fn overwrite_anchor(this: &mut WidgetMut<'_, Self>, idx: i64) {
        this.widget.anchor_index = idx;
        this.widget.scroll_offset_from_anchor = 0.;
        this.ctx.request_layout();
    }
}

/// We assume that by default, virtual scrolling items are at least ~30 logical pixels tall (two lines of text + a bit).
/// Because we load the visible page, and a page above and below that, a safety margin of 2 effectively applies.
///
/// We start by guessing too large, because we expect to end up in a fixed-point loop, so if we have loaded
/// too few items, that will be sorted relatively quickly.
const DEFAULT_MEAN_ITEM_WIDTH: f64 = 180.;

// --- MARK: IMPL WIDGET
impl Widget for VirtualHScroll {
    type Action = VirtualHScrollAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if let PointerEvent::Scroll(PointerScrollEvent { delta, .. }) = event {
            let size = ctx.content_box_size();
            // TODO - Remove reference to scale factor.
            // See https://github.com/linebender/xilem/issues/1264
            let scale_factor = ctx.get_scale_factor();
            let line_px = PhysicalPosition {
                x: 120.0 * scale_factor,
                y: 120.0 * scale_factor,
            };
            let page_px = PhysicalPosition {
                x: size.width * scale_factor,
                y: size.height * scale_factor,
            };

            let delta_px = delta.to_pixel_delta(line_px, page_px);
            let logical_delta_px = delta_px.to_logical::<f64>(scale_factor);
            let delta = -if logical_delta_px.x != 0. {
                logical_delta_px.x
            } else {
                logical_delta_px.y
            };
            self.scroll_offset_from_anchor += self.direction_appropriate(delta);
            self.event_post_scroll(ctx);
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        // We use an unreasonably large delta (logical pixels) here to allow testing that the case
        // where the scrolling "jumps" the area is handled correctly.
        // In future, this manual testing would be achieved through use of a scrollbar.
        const DELTA_PAGE: f64 = 2000.;

        const DELTA_LINE: f64 = 20.;

        // To get to this state, you currently need to press "tab" to focus this widget in the
        // example.
        let TextEvent::Keyboard(keyboard_event) = event else {
            return;
        };

        match keyboard_event {
            KeyboardEvent {
                state: KeyState::Down,
                key: Key::Named(NamedKey::PageDown),
                ..
            } => {
                self.scroll_offset_from_anchor += DELTA_PAGE;
                self.event_post_scroll(ctx);
                ctx.set_handled();
            }
            KeyboardEvent {
                state: KeyState::Down,
                key: Key::Named(NamedKey::PageUp),
                ..
            } => {
                self.scroll_offset_from_anchor -= DELTA_PAGE;
                self.event_post_scroll(ctx);
                ctx.set_handled();
            }
            KeyboardEvent {
                state: KeyState::Down,
                key: Key::Named(NamedKey::ArrowLeft),
                ..
            } => {
                self.scroll_offset_from_anchor += self.direction_appropriate(DELTA_LINE);
                self.event_post_scroll(ctx);
                ctx.set_handled();
            }
            KeyboardEvent {
                state: KeyState::Down,
                key: Key::Named(NamedKey::ArrowRight),
                ..
            } => {
                self.scroll_offset_from_anchor -= self.direction_appropriate(DELTA_LINE);
                self.event_post_scroll(ctx);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if matches!(
            event.action,
            accesskit::Action::ScrollLeft | accesskit::Action::ScrollRight
        ) {
            let unit = if let Some(accesskit::ActionData::ScrollUnit(unit)) = &event.data {
                *unit
            } else {
                accesskit::ScrollUnit::Item
            };
            let amount = match unit {
                accesskit::ScrollUnit::Item => self.anchor_width,
                accesskit::ScrollUnit::Page => ctx.content_box_size().width,
            };
            if event.action == accesskit::Action::ScrollLeft {
                self.scroll_offset_from_anchor -= self.direction_appropriate(amount);
            } else {
                self.scroll_offset_from_anchor += self.direction_appropriate(amount);
            }
            self.event_post_scroll(ctx);
            ctx.set_handled();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        interval: u64,
    ) {
        if self.autoscroll_velocity == 0. {
            return;
        }

        let delta = interval as f64 * 1e-9 * self.autoscroll_velocity;
        self.scroll_offset_from_anchor -= self.direction_appropriate(delta);
        self.update_post_scroll(ctx);
        ctx.request_anim_frame();
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        // TODO: Register in id order
        for child in self.items.values_mut() {
            ctx.register_child(child);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if let Update::RequestPanToChild(target) = event {
            let new_pos_x = compute_pan_range(
                0.0..ctx.content_box_size().width,
                target.min_x()..target.max_x(),
            )
            .start;
            self.scroll_offset_from_anchor += new_pos_x;
            self.update_post_scroll(ctx);
        }
    }

    fn measure(
        &mut self,
        _ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        _axis: Axis,
        len_req: LenReq,
        _cross_length: Option<f64>,
    ) -> f64 {
        // Our preferred size is a const square in logical pixels.
        //
        // It is not clear that a data-derived result would be better.
        // We definitely can't load all the children to calculate our unclipped size.
        //
        // If we would base it on the currently loaded items, then the preferred size
        // would fluctuate all over the place. The UI experience would be miserable,
        // with our viewport size frequently changing as the user is scrolling.
        //
        // Perhaps it would be worth it to always keep some first N items in memory and
        // derive our preferred size always from those. That way it would be much more stable.
        // We could also detect if we have a defined size via props and then unload those items.
        // Still, we would run into complexities with ensuring they are loaded in time for measure.
        //
        // So, for now, we just use a simple O(1) default.
        const DEFAULT_LENGTH: f64 = 100.;

        // TODO: Remove HACK: Until scale factor rework happens, just pretend it's always 1.0.
        //       https://github.com/linebender/xilem/issues/1264
        let scale = 1.0;

        match len_req {
            LenReq::MinContent | LenReq::MaxContent => DEFAULT_LENGTH * scale,
            LenReq::FitContent(space) => space,
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.set_clip_path(size.to_rect());
        // The number of loaded items before the anchor
        let mut width_before_anchor = 0.;
        let mut total_width = 0.;
        let mut count = 0_u64;
        let mut first_item: Option<i64> = None;
        let mut last_item: Option<i64> = None;

        // Calculate the sizes of all children
        for (idx, child) in &mut self.items {
            if !self.active_range.contains(idx) {
                // We stash any children which we have which are outside of the active range.
                // This is because we have asked the driver to remove them, but it hasn't gotten
                // around to it yet.
                // N.B. although `LayoutCtx::set_stashed` is documented as being "TODO" for removal, this
                // is nearly impossible to handle correctly without using this method; in particular, if a
                // driver is delayed in being called (such as if layout is run twice in the passes loop).
                ctx.set_stashed(child, true);
                continue;
            }
            first_item = first_item.map(|it| it.min(*idx)).or(Some(*idx));
            last_item = last_item.map(|it| it.max(*idx)).or(Some(*idx));
            let auto_size = SizeDef::fit(size).with_width(LenDef::MaxContent);
            let child_size = ctx.compute_size(child, auto_size, size.into());
            ctx.run_layout(child, child_size);
            if *idx < self.anchor_index {
                width_before_anchor += child_size.width;
            }
            total_width += child_size.width;
            count += 1;
        }

        let mean_item_width = if count > 0 {
            total_width / count as f64
        } else {
            self.mean_item_width
        };
        let mean_item_width = if !mean_item_width.is_finite() || mean_item_width < 0.01 {
            tracing::warn!(
                "Got an unreasonable mean item width {mean_item_width} in virtual scrolling"
            );
            DEFAULT_MEAN_ITEM_WIDTH
        } else {
            mean_item_width
        };
        self.mean_item_width = mean_item_width;

        // Determine the new anchor
        loop {
            if self.scroll_offset_from_anchor < 0. {
                if self.anchor_index <= self.valid_range.start {
                    self.cap_scroll_range_start();
                    break;
                }
                self.anchor_index -= 1;
                let new_anchor_width = if self.active_range.contains(&self.anchor_index) {
                    let new_anchor = self.items.get(&self.anchor_index);
                    if let Some(new_anchor) = new_anchor {
                        ctx.child_size(new_anchor).width
                    } else {
                        // We don't treat missing items inside the set of loaded items as having a width.
                        // This avoids potential infinite loops (from adding a new
                        // item increasing the mean item size, causing that new item to become unloaded)
                        break;
                    }
                } else {
                    // In theory, even for inactive items which haven't been removed, we could
                    // get their prior width.
                    // However, we choose not to do this to make behaviour predictable; we don't
                    // want there to be any advantage to not removing items which should be removed.
                    mean_item_width
                };

                // We know that this will eventually become larger than zero because:
                // 1) `mean_item_width` has been validated to be greater than zero
                // 2) There are a finite number of items which might have zero width (the items in the active_range)
                // Therefore, the else block of the original area will always be entered if we reach this point.
                self.scroll_offset_from_anchor += new_anchor_width;
                width_before_anchor -= new_anchor_width;
            } else {
                let anchor_width = if self.active_range.contains(&self.anchor_index) {
                    let current_anchor = self.items.get(&self.anchor_index);
                    if let Some(anchor_pod) = current_anchor {
                        ctx.child_size(anchor_pod).width
                    } else {
                        break;
                    }
                } else {
                    mean_item_width
                };

                // We only ever subtract a from `scroll_offset_from_anchor` less than
                // or equal to its current value.
                // Therefore: In this half of the loop, we never make `self.scroll_offset_from_anchor < 0.`,
                // so we never re-enter the first half of the loop.
                if self.scroll_offset_from_anchor >= anchor_width {
                    self.anchor_index += 1;
                    // `anchor_width` is definitely eventually greater than zero here because:
                    // 1) `mean_item_width` has been validated to be greater than zero
                    // 2) There are a finite number of items which might have zero width (the items in the active_range)
                    // Therefore, this block will always eventually reach its else condition, ending the loop.
                    self.scroll_offset_from_anchor -= anchor_width;
                    width_before_anchor += anchor_width;
                } else {
                    break;
                }
            }
        }
        let at_valid_end = self.anchor_index + 1 >= self.valid_range.end;
        if at_valid_end {
            self.anchor_index = self.valid_range.end - 1;
        }
        if self.anchor_index < self.valid_range.start {
            self.anchor_index = self.valid_range.start;
            // If even after applying the "stored" scroll, we're outside the valid range, cap it.
            self.scroll_offset_from_anchor = 0.;
        }
        self.anchor_width = if let Some(anchor) = self
            .items
            .get(&self.anchor_index)
            .filter(|_| self.active_range.contains(&self.anchor_index))
        {
            ctx.child_size(anchor).width
        } else {
            mean_item_width
        };
        if at_valid_end {
            self.scroll_offset_from_anchor = f64::INFINITY;
            self.cap_scroll_range_end(self.anchor_width, size.width);
        }

        // Load a page and a half above the screen
        let cutoff_start = size.width * 1.5;
        // Load a page and a half below the screen (note that this cutoff "includes" the screen)
        // We also need to allow scrolling *at least* to the top of the current anchor; therefore, we load items sufficiently
        // that scrolling the bottom of the anchor to the top of the screen, we still have the desired margin
        let cutoff_end = size.width * 2.5 + self.anchor_width;

        let mut item_crossing_start = None;
        let mut item_crossing_end = self.active_range.start;
        let mut x = -width_before_anchor;
        let mut was_dense = true;
        // We lay all of the active items out (even though some of them will be made inactive
        // after layout is done)
        for idx in self.active_range.clone() {
            if x <= -cutoff_start {
                item_crossing_start = Some(idx);
            }
            if x <= cutoff_end {
                item_crossing_end = idx;
            }
            let item = self.items.get_mut(&idx);
            if let Some(item) = item {
                let item_size = ctx.child_size(item);
                let placed_x = if self.left_to_right {
                    x
                } else {
                    -x - item_size.width
                };
                ctx.place_child(item, Point::new(placed_x, 0.));
                self.placed_items.insert(idx, (placed_x, item_size.width));
                // TODO: Padding/gap?
                x += item_size.width;
            } else {
                was_dense = false;
                // We expect the virtual scrolling to be dense; we are designed
                // to handle the non-dense case gracefully, but it is a bug in your
                // component/app if the results are not dense.
                if !self.warned_not_dense {
                    self.warned_not_dense = true;
                    tracing::error!(
                        "Virtual Scrolling items in {:?} ({}) not dense.\n\
                        Expected to be dense in {:?}, but missing {idx}",
                        ctx.widget_id(),
                        self.type_name(),
                        self.active_range,
                    );
                }
            }
        }
        if was_dense {
            // For each time we have the falling edge of becoming not dense, we want to warn.
            self.warned_not_dense = false;
        }
        // We only send an updated request if the driver has actioned the previous request.
        if self.action_handled {
            let target_range = if self.active_range.contains(&self.anchor_index) {
                let start = if let Some(item_crossing_start) = item_crossing_start {
                    item_crossing_start
                } else {
                    let number_needed =
                        ((cutoff_start - width_before_anchor) / mean_item_width).ceil() as i64;
                    // Previous versions of this code had a positive feedback loop, if the driver
                    // refused to give items for ranges it claimed to support (such as if the
                    // valid_range were misconfigured).
                    // Ideally, we'd warn in this situation, but it isn't feasible
                    // to know if we're here because:
                    // 1) The driver is misbehaving; OR
                    // 2) We've reran the passes for some other reason.
                    let start_anchor = first_item.unwrap_or(self.anchor_index);
                    start_anchor - number_needed
                };
                let end = if x >= cutoff_end {
                    item_crossing_end + 1
                } else {
                    // `x` is the end of the last loaded item
                    let number_needed = ((cutoff_end - x) / mean_item_width).ceil() as i64;
                    let end_anchor = last_item.unwrap_or(self.anchor_index);
                    end_anchor + number_needed + 1 /* End index is exclusive, whereas `end_anchor` is "included" */
                };
                start..end
            } else {
                // We've jumped a huge distance in view space (see `Self::overwrite_anchor`)
                // Handle that sanely.
                let start = self.anchor_index - (cutoff_start / mean_item_width).ceil() as i64;
                let end = self.anchor_index + (cutoff_end / mean_item_width).ceil() as i64;
                start..end
            };

            let target_range = if self.valid_range.is_empty() {
                self.valid_range.clone()
            } else {
                // Avoid requesting invalid items by clamping to the valid range
                let start = target_range
                    .start
                    // target_range.start is inclusive whereas valid_range.end is exclusive; convert between the two.
                    .clamp(self.valid_range.start, self.valid_range.end - 1);
                let end = target_range
                    .end
                    .clamp(self.valid_range.start, self.valid_range.end);
                start..end
            };

            if self.active_range != target_range {
                ctx.submit_action::<Self::Action>(VirtualHScrollAction::ActiveRange(
                    VirtualScrollAction {
                        old_active: self.active_range.clone(),
                        target: target_range,
                    },
                ));
                self.action_handled = false;
            }
        }

        // TODO: We should still try and find a way to detect infinite loops;
        // our pattern for this should avoid it, but if that assessment is wrong, the outcome would be very bad
        // (a driver which didn't correctly set `valid_range` would be one cause).
    }

    fn compose(&mut self, ctx: &mut ComposeCtx<'_>) {
        let content_width = ctx.content_box_size().width;
        let x = -self.direction_appropriate(self.scroll_offset_from_anchor)
            + if self.left_to_right {
                0.
            } else {
                content_width
            };
        let translation = Vec2::new(x, 0.);
        let mut visible_start = None;
        let mut visible_end = None;
        for idx in self.active_range.clone() {
            if let Some(child) = self.items.get_mut(&idx) {
                if self.autoscroll_velocity != 0. {
                    ctx.set_animated_child_scroll_translation(child, translation);
                } else {
                    ctx.set_child_scroll_translation(child, translation);
                }
                let (placed_x, item_width) = *self.placed_items.get(&idx).unwrap();
                let a = placed_x + x;
                let b = a + item_width;
                let visible = (a > 0. && a < content_width) || (b > 0. && b < content_width);
                if visible {
                    if visible_start.is_none() {
                        visible_start = Some(idx);
                    }
                    visible_end = Some(idx);
                }
            }
        }
        let visible_start = visible_start.unwrap_or(self.active_range.start);
        let visible_end = visible_end.unwrap_or(visible_start);
        let visible_range = visible_start..visible_end;
        if visible_range != self.visible_range {
            ctx.submit_action::<Self::Action>(VirtualHScrollAction::VisibleRange(visible_range));
        }
    }

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        _scene: &mut xilem::vello::Scene,
    ) {
        // We run these checks in `paint` as they are outside of the pass-based fixedpoint loop
        if !self.action_handled {
            if self.missed_actions_count == 0 {
                tracing::warn!(
                    "VirtualScroll got to painting without its action (i.e. it's request for items to be loaded) being handled.\n\
                    This means that there was a delay in handling its action for some reason.\n\
                    Maybe your driver only handles one action at a time?"
                );
            }
            if self.missed_actions_count > 10 {
                debug_panic!(
                    "VirtualScroll's action is being missed repeatedly being handled.\n\
                    Note that to handle an action, you must call `VirtualHScroll::will_handle_action` with the action."
                );
                // In release mode, re-send the action, which will hopefully get things unstuck.
                self.action_handled = true;
            }
            self.missed_actions_count += 1;
        }
    }

    fn accessibility_role(&self) -> accesskit::Role {
        accesskit::Role::ScrollView
    }

    fn accessibility(
        &mut self,
        ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut accesskit::Node,
    ) {
        node.set_clips_children();
        node.set_orientation(accesskit::Orientation::Vertical);
        if self.valid_range.start == i64::MIN {
            // Even when we support infinite scroll in both directions, we need
            // to set scroll_x somehow, so the platform adapter can know when
            // scrolling happened and fire the appropriate platform event;
            // this is particularly important on Android. Here, we assume that
            // in practice, the anchor index is in range for an f64.
            // TBD: Is there a better way to do this?
            if self.anchor_index != i64::MIN && self.anchor_index != i64::MAX {
                let x = (self.anchor_index as f64) * self.mean_item_width
                    + self.scroll_offset_from_anchor;
                node.set_scroll_x(x);
            }
        } else {
            node.set_scroll_x_min(0.0);
            let x = (((self.anchor_index - self.valid_range.start) as f64) * self.mean_item_width
                + self.scroll_offset_from_anchor)
                .max(0.);
            node.set_scroll_x(x);
            if self.valid_range.end != i64::MAX {
                let y_max = (((self.valid_range.end - self.valid_range.start) as f64)
                    * self.mean_item_width)
                    .max(0.);
                node.set_scroll_x_max(y_max);
            }
        }
        if self.anchor_index != self.valid_range.start || self.scroll_offset_from_anchor > 0. {
            node.add_action(accesskit::Action::ScrollUp);
        }
        let at_end = self.anchor_index + 1 == self.valid_range.end && {
            let max_scroll = (self.anchor_width - ctx.content_box_size().width / 2.).max(0.0);
            self.scroll_offset_from_anchor >= max_scroll
        };
        if !at_end {
            node.add_action(accesskit::Action::ScrollDown);
        }
        node.add_child_action(accesskit::Action::ScrollIntoView);
    }

    fn children_ids(&self) -> ChildrenIds {
        let mut items = self
            .items
            .iter()
            .map(|(index, pod)| (*index, pod.id()))
            .collect::<Vec<_>>();
        items.sort_unstable_by_key(|(index, _)| *index);
        items.into_iter().map(|(_, id)| id).collect()
    }

    fn accepts_text_input(&self) -> bool {
        false
    }

    fn accepts_focus(&self) -> bool {
        // Our focus behaviour is not carefully designed.
        // There are a few things to consider:
        // - We want this widget to accept e.g. pagedown events, even when there is no focusable child
        // - We want the keyboard focus to be able to "escape" the virtual list, rather than be trapped.
        // See also the caveat in the main docs for this widget.
        // This is true for now to allow PageDown events to be handled.
        true
    }

    // TODO: Optimise using binary search?
    // fn find_widget_under_pointer(..);

    fn get_debug_text(&self) -> Option<String> {
        Some(format!("{self:#?}"))
    }
}

/// Optimisation for:
/// ```
/// let old_range = 0i64..10;
/// let new_range = 0i64..10;
/// for idx in old_range {
///     if !new_range.contains(&idx) {
///         // ...
///     }
/// }
/// ```
/// as an iterator
#[allow(
    dead_code,
    reason = "Plan to expose this publicly in `VirtualScrollAction`, keep its tests around"
)]
fn opt_iter_difference(
    old_range: &Range<i64>,
    new_range: &Range<i64>,
) -> std::iter::Chain<Range<i64>, Range<i64>> {
    (old_range.start..(new_range.start.min(old_range.end)))
        .chain(new_range.end.max(old_range.start)..old_range.end)
}

pub(crate) fn compute_pan_range(mut viewport: Range<f64>, target: Range<f64>) -> Range<f64> {
    // if either range contains the other, the viewport doesn't move
    if target.start <= viewport.start && viewport.end <= target.end {
        return viewport;
    }
    if viewport.start <= target.start && target.end <= viewport.end {
        return viewport;
    }

    // we compute the length that we need to "fit" in our viewport
    let target_width = f64::min(viewport.end - viewport.start, target.end - target.start);
    let viewport_width = viewport.end - viewport.start;

    // Because of the early returns, there are only two cases to consider: we need
    // to move the viewport "left" or "right"
    if viewport.start >= target.start {
        viewport.start = target.end - target_width;
        viewport.end = viewport.start + viewport_width;
    } else {
        viewport.end = target.start + target_width;
        viewport.start = viewport.end - viewport_width;
    }

    viewport
}

// // --- MARK: TESTS
// #[cfg(test)]
// mod tests {
//     use std::collections::HashSet;

//     use masonry::kurbo::{Size, Vec2};
//     use masonry::parley::StyleProperty;

//     use super::opt_iter_difference;
//     use masonry::core::{NewWidget, Widget, WidgetId, WidgetMut};
//     use masonry::testing::{TestHarness, assert_render_snapshot};
//     use masonry::theme::default_property_set;
//     use masonry::widgets::Label;

//     use super::*;

//     #[test]
//     #[expect(
//         clippy::reversed_empty_ranges,
//         reason = "Testing technically possible behaviour"
//     )]
//     fn opt_iter_difference_equiv() {
//         let ranges = [
//             5..10,
//             7..15,
//             -10..7,
//             // Negative ranges are empty; those should be respected.
//             // The optimised version does actually do more than is needed if the new range is negative
//             // However, we don't expect negative ranges to be common (only supported for robustness), so
//             // we don't care if they aren't handled as performantly as possible, so long as it doesn't miss anything
//             20..10,
//             12..17,
//         ];
//         for old_range in &ranges {
//             for new_range in &ranges {
//                 let opt_result = opt_iter_difference(old_range, new_range).collect::<HashSet<_>>();
//                 let mut naive_result = HashSet::new();
//                 for idx in old_range.clone() {
//                     if !new_range.contains(&idx) {
//                         naive_result.insert(idx);
//                     }
//                 }
//                 assert_eq!(
//                     opt_result, naive_result,
//                     "The optimised version of differences should be equivalent to the trivially \
//                     correct method, but wasn't for {old_range:?} and {new_range:?}"
//                 );
//             }
//         }
//     }

//     #[test]
//     fn sensible_driver() {
//         let widget = VirtualHScroll::new(0).with_auto_id();

//         let mut harness =
//             TestHarness::create_with_size(default_property_set(), widget, Size::new(100., 200.));
//         let virtual_scroll_id = harness.root_id();
//         fn driver(action: VirtualScrollAction, mut scroll: WidgetMut<'_, VirtualHScroll>) {
//             VirtualHScroll::will_handle_action(&mut scroll, &action);
//             for idx in action.old_active.clone() {
//                 if !action.target.contains(&idx) {
//                     VirtualHScroll::remove_child(&mut scroll, idx);
//                 }
//             }
//             for idx in action.target {
//                 if !action.old_active.contains(&idx) {
//                     VirtualHScroll::add_child(
//                         &mut scroll,
//                         idx,
//                         NewWidget::new(
//                             Label::new(format!("{idx}")).with_style(StyleProperty::FontSize(30.)),
//                         )
//                         .erased(),
//                     );
//                 }
//             }
//         }

//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         assert_render_snapshot!(harness, "virtual_scroll_basic");
//         harness.edit_root_widget(|mut scroll| {
//             VirtualHScroll::overwrite_anchor(&mut scroll, 100);
//         });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         assert_render_snapshot!(harness, "virtual_scroll_moved");
//         harness.mouse_move_to(virtual_scroll_id);
//         harness.mouse_wheel(Vec2 { x: 25., y: 0. });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         assert_render_snapshot!(harness, "virtual_scroll_scrolled");
//     }

//     #[test]
//     /// We shouldn't panic or loop if there are small gaps in the items provided by the driver.
//     /// Again, this isn't valid code for a user to write, but we should just warn and deal with it
//     fn small_gaps() {
//         let widget = VirtualHScroll::new(0).with_auto_id();

//         let mut harness =
//             TestHarness::create_with_size(default_property_set(), widget, Size::new(100., 200.));
//         let virtual_scroll_id = harness.root_id();
//         fn driver(action: VirtualScrollAction, mut scroll: WidgetMut<'_, VirtualHScroll>) {
//             VirtualHScroll::will_handle_action(&mut scroll, &action);
//             for idx in action.old_active.clone() {
//                 if !action.target.contains(&idx) {
//                     VirtualHScroll::remove_child(&mut scroll, idx);
//                 }
//             }
//             for idx in action.target {
//                 if !action.old_active.contains(&idx) && idx % 2 == 0 {
//                     VirtualHScroll::add_child(
//                         &mut scroll,
//                         idx,
//                         NewWidget::new(
//                             Label::new(format!("{idx}")).with_style(StyleProperty::FontSize(30.)),
//                         )
//                         .erased(),
//                     );
//                 }
//             }
//         }

//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         harness.edit_root_widget(|mut scroll| {
//             VirtualHScroll::overwrite_anchor(&mut scroll, 100);
//         });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         harness.mouse_move_to(virtual_scroll_id);
//         harness.mouse_wheel(Vec2 { x: 200., y: 0. });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//     }

//     #[test]
//     /// We shouldn't panic or loop if there are big gaps in the items provided by the driver.
//     /// Note that we don't test rendering in this case, because this is a driver which breaks our contract.
//     fn big_gaps() {
//         let widget = VirtualHScroll::new(0).with_auto_id();

//         let mut harness =
//             TestHarness::create_with_size(default_property_set(), widget, Size::new(100., 200.));
//         let virtual_scroll_id = harness.root_id();
//         fn driver(action: VirtualScrollAction, mut scroll: WidgetMut<'_, VirtualHScroll>) {
//             VirtualHScroll::will_handle_action(&mut scroll, &action);
//             for idx in action.old_active.clone() {
//                 if !action.target.contains(&idx) {
//                     VirtualHScroll::remove_child(&mut scroll, idx);
//                 }
//             }
//             for idx in action.target {
//                 if !action.old_active.contains(&idx) && idx % 100 == 1 {
//                     VirtualHScroll::add_child(
//                         &mut scroll,
//                         idx,
//                         NewWidget::new(
//                             Label::new(format!("{idx}")).with_style(StyleProperty::FontSize(30.)),
//                         )
//                         .erased(),
//                     );
//                 }
//             }
//         }

//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         harness.edit_root_widget(|mut scroll| {
//             VirtualHScroll::overwrite_anchor(&mut scroll, 200);
//         });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         harness.mouse_move_to(virtual_scroll_id);
//         harness.mouse_wheel(Vec2 { x: 200., y: 0. });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//     }

//     #[test]
//     /// We shouldn't panic or loop if the driver is very poorly written (doesn't set `valid_range` correctly)
//     /// Note that we don't test rendering in this case, because this is a driver which breaks our contract.
//     fn degenerate_driver() {
//         let widget = VirtualHScroll::new(0).with_auto_id();

//         let mut harness =
//             TestHarness::create_with_size(default_property_set(), widget, Size::new(100., 200.));
//         let virtual_scroll_id = harness.root_id();
//         fn driver(action: VirtualScrollAction, mut scroll: WidgetMut<'_, VirtualHScroll>) {
//             VirtualHScroll::will_handle_action(&mut scroll, &action);
//             for idx in action.old_active.clone() {
//                 if !action.target.contains(&idx) {
//                     VirtualHScroll::remove_child(&mut scroll, idx);
//                 }
//             }
//             for idx in action.target {
//                 if !action.old_active.contains(&idx) && idx < 5 {
//                     VirtualHScroll::add_child(
//                         &mut scroll,
//                         idx,
//                         NewWidget::new(
//                             Label::new(format!("{idx}")).with_style(StyleProperty::FontSize(30.)),
//                         )
//                         .erased(),
//                     );
//                 }
//             }
//         }

//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         harness.edit_root_widget(|mut scroll| {
//             VirtualHScroll::overwrite_anchor(&mut scroll, 200);
//         });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         harness.mouse_move_to(virtual_scroll_id);
//         harness.mouse_wheel(Vec2 { x: 200., y: 0. });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//     }

//     #[test]
//     /// If there's a minimum to the valid range, we should behave in a sensible way.
//     fn limited_up() {
//         const MIN: i64 = 10;
//         let widget = VirtualHScroll::new(0)
//             .with_valid_range(MIN..i64::MAX)
//             .with_auto_id();

//         let mut harness =
//             TestHarness::create_with_size(default_property_set(), widget, Size::new(100., 200.));
//         let virtual_scroll_id = harness.root_id();
//         fn driver(action: VirtualScrollAction, mut scroll: WidgetMut<'_, VirtualHScroll>) {
//             VirtualHScroll::will_handle_action(&mut scroll, &action);
//             for idx in action.old_active.clone() {
//                 if !action.target.contains(&idx) {
//                     VirtualHScroll::remove_child(&mut scroll, idx);
//                 }
//             }
//             for idx in action.target {
//                 if !action.old_active.contains(&idx) {
//                     assert!(
//                         idx >= MIN,
//                         "Virtual Scroll controller should never request an invalid id. Requested {idx}"
//                     );
//                     VirtualHScroll::add_child(
//                         &mut scroll,
//                         idx,
//                         NewWidget::new(
//                             Label::new(format!("{idx}")).with_style(StyleProperty::FontSize(30.)),
//                         )
//                         .erased(),
//                     );
//                 }
//             }
//         }

//         let original_range;
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         {
//             let widget = harness.root_widget();
//             assert_eq!(
//                 widget.anchor_index, MIN,
//                 "Virtual Scroll controller should lock anchor to be within active range"
//             );
//             assert_eq!(
//                 widget.scroll_offset_from_anchor, 0.0,
//                 "Virtual Scroll controller should lock top of the first item to the top of the screen if jumping"
//             );
//             original_range = widget.active_range.clone();
//         }
//         harness.mouse_move_to(virtual_scroll_id);
//         harness.mouse_wheel(Vec2 { x: -50., y: 0. });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         {
//             let widget = harness.root_widget();
//             assert_ne!(widget.anchor_index, MIN);
//             assert_ne!(widget.active_range, original_range);
//         }
//         harness.mouse_wheel(Vec2 { x: 60., y: 0. });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         {
//             let widget = harness.root_widget();
//             assert_eq!(widget.anchor_index, MIN);
//             assert_eq!(widget.scroll_offset_from_anchor, 0.0);
//         }
//     }

//     #[test]
//     /// If there's a maximum to the valid range, we should behave in a sensible way.
//     fn limited_down() {
//         const MAX: i64 = 10;
//         let widget = VirtualHScroll::new(100)
//             .with_valid_range(i64::MIN..MAX)
//             .with_auto_id();

//         let mut harness =
//             TestHarness::create_with_size(default_property_set(), widget, Size::new(100., 200.));
//         let virtual_scroll_id = harness.root_id();
//         fn driver(action: VirtualScrollAction, mut scroll: WidgetMut<'_, VirtualHScroll>) {
//             VirtualHScroll::will_handle_action(&mut scroll, &action);
//             for idx in action.old_active.clone() {
//                 if !action.target.contains(&idx) {
//                     VirtualHScroll::remove_child(&mut scroll, idx);
//                 }
//             }
//             for idx in action.target {
//                 if !action.old_active.contains(&idx) {
//                     assert!(
//                         idx < MAX,
//                         "Virtual Scroll controller should never request an invalid id. Requested {idx}"
//                     );
//                     VirtualHScroll::add_child(
//                         &mut scroll,
//                         idx,
//                         NewWidget::new(
//                             Label::new(format!("{idx}")).with_style(StyleProperty::FontSize(30.)),
//                         )
//                         .erased(),
//                     );
//                 }
//             }
//         }

//         let original_range;
//         let original_scroll;
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         {
//             let widget = harness.root_widget();
//             assert_eq!(
//                 widget.anchor_index,
//                 MAX - 1,
//                 "Virtual Scroll controller should lock anchor to be within active range"
//             );
//             // We are scrolled down as far as possible. This is hard to write a convincing code test for,
//             // so validate it with code.
//             original_scroll = widget.scroll_offset_from_anchor;
//             original_range = widget.active_range.clone();
//             assert_render_snapshot!(harness, "virtual_scroll_limited_up_bottom");
//         }
//         harness.mouse_move_to(virtual_scroll_id);
//         harness.mouse_wheel(Vec2 { x: 5., y: 0. });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         {
//             let widget = harness.root_widget();
//             assert_ne!(widget.anchor_index, MAX);
//             assert_ne!(widget.active_range, original_range);
//         }
//         harness.mouse_wheel(Vec2 { x: -6., y: 0. });
//         drive_to_fixpoint(&mut harness, virtual_scroll_id, driver);
//         {
//             let widget = harness.root_widget();
//             assert_eq!(widget.anchor_index, MAX - 1);
//             assert_eq!(
//                 widget.scroll_offset_from_anchor, original_scroll,
//                 "Should be scrolled as far as possible (which is the same as we originally were)"
//             );
//         }
//     }

//     fn drive_to_fixpoint(
//         harness: &mut TestHarness<VirtualHScroll>,
//         virtual_scroll_id: WidgetId,
//         mut f: impl FnMut(VirtualScrollAction, WidgetMut<'_, VirtualHScroll>),
//     ) {
//         let mut iteration = 0;
//         let mut old_active = None;
//         loop {
//             iteration += 1;
//             if iteration > 1000 {
//                 panic!("Took too long to reach fixpoint");
//             }
//             let Some((action, id)) = harness.pop_action::<VirtualScrollAction>() else {
//                 break;
//             };
//             assert_eq!(
//                 id, virtual_scroll_id,
//                 "Only widget in tree should give action"
//             );
//             if let Some(old_active) = old_active.take() {
//                 assert_eq!(action.old_active, old_active);
//             }
//             old_active = Some(action.target.clone());
//             assert!(
//                 action.target != action.old_active,
//                 "Shouldn't have sent an update if tUsehe target hasn't changed"
//             );

//             harness.edit_root_widget(|scroll| {
//                 f(action, scroll);
//             });
//         }
//     }
// }
