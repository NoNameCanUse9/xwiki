//! Draggable horizontal splitter with a Cobalt hairline divider.
//!
//! One side gets the draggable width; the other fills the remainder. Drag
//! clamps to `min..=max`, double-click resets to `default`, and every change
//! is reported through `on_change` so the caller can persist it.

use std::sync::Arc;

use gpui::{
    div, px, App, AppContext, ClickEvent, Context, CursorStyle, Div, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
};

use crate::ui::tokens;

/// Drag state captured when the divider renders. The mouse has to be within
/// `SPLITTER_HIT` of the divider to grab it, so the render-time position is
/// exact enough — the same approach zed's picker resize handles use.
#[derive(Clone, Copy)]
struct PanelDrag {
    width_before: f32,
    mouse_before: f32,
}

/// Empty drag overlay; the divider itself is the live feedback.
struct DragPreview;

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    /// The left panel has the draggable width.
    Left,
    /// The right panel has the draggable width.
    Right,
}

fn split(
    side: Side,
    id: &'static str,
    width: f32,
    min: f32,
    max: f32,
    default: f32,
    window: &mut Window,
    hairline: Hsla,
    hover: Hsla,
    left: impl IntoElement,
    right: impl IntoElement,
    on_change: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> Div {
    let on_change = Arc::new(on_change);
    let on_drag = on_change.clone();
    let on_reset = on_change.clone();
    let start = PanelDrag {
        width_before: width,
        mouse_before: f32::from(window.mouse_position().x),
    };

    let divider = div()
        .id(id)
        .w(px(tokens::SPLITTER_HIT))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorStyle::ResizeLeftRight)
        .hover(|s| s.bg(hover))
        .on_mouse_down(MouseButton::Left, |_, _, _| {})
        .on_drag(start, |_, _, _, cx| cx.new(|_| DragPreview))
        .on_drag_move::<PanelDrag>(move |event, window, cx| {
            let drag = event.drag(cx);
            let delta = f32::from(event.event.position.x) - drag.mouse_before;
            let next = match side {
                Side::Left => drag.width_before + delta,
                Side::Right => drag.width_before - delta,
            };
            (on_drag)(next.clamp(min, max), window, cx);
        })
        .on_click(move |event: &ClickEvent, window, cx| {
            if event.click_count() >= 2 {
                (on_reset)(default, window, cx);
            }
        })
        .child(div().w(px(1.0)).h_full().bg(hairline));

    match side {
        Side::Left => div()
            .flex()
            .size_full()
            .child(div().w(px(width)).h_full().child(left))
            .child(divider)
            .child(div().flex_1().min_w(px(0.0)).h_full().child(right)),
        Side::Right => div()
            .flex()
            .size_full()
            .child(div().flex_1().min_w(px(0.0)).h_full().child(left))
            .child(divider)
            .child(div().w(px(width)).h_full().child(right)),
    }
}

/// `[left (fixed) | divider | right (flex)]` — e.g. the project / doc rails.
pub fn horizontal(
    id: &'static str,
    width: f32,
    min: f32,
    max: f32,
    default: f32,
    window: &mut Window,
    hairline: Hsla,
    hover: Hsla,
    left: impl IntoElement,
    right: impl IntoElement,
    on_change: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> Div {
    split(
        Side::Left,
        id,
        width,
        min,
        max,
        default,
        window,
        hairline,
        hover,
        left,
        right,
        on_change,
    )
}

/// `[left (flex) | divider | right (fixed)]` — e.g. the history panel.
pub fn horizontal_right(
    id: &'static str,
    width: f32,
    min: f32,
    max: f32,
    default: f32,
    window: &mut Window,
    hairline: Hsla,
    hover: Hsla,
    left: impl IntoElement,
    right: impl IntoElement,
    on_change: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> Div {
    split(
        Side::Right,
        id,
        width,
        min,
        max,
        default,
        window,
        hairline,
        hover,
        left,
        right,
        on_change,
    )
}
