//! Cobalt UI kit: design tokens + small shared styling helpers.
//!
//! Views should consume `tokens::*` and the helpers here instead of raw
//! literals, so the desktop app stays aligned with the web Cobalt language.

pub mod split_pane;
pub mod tokens;

use std::sync::{Arc, OnceLock};

use gpui::{
    div, img, px, Div, ElementId, Image, ImageFormat, Img, IntoElement, ParentElement as _,
    Styled as _,
};
use gpui_component::text::TextView;

const APP_ICON_SVG: &[u8] = include_bytes!("../../assets/agentdocs-app-icon.svg");
static APP_ICON_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();

/// The AgentDocs brand mark, embedded at compile time so packaged builds do
/// not depend on the source tree at runtime.
pub fn app_icon() -> Img {
    let image = APP_ICON_IMAGE
        .get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Svg, APP_ICON_SVG.to_vec())))
        .clone();
    img(image).flex_none()
}

/// Mono machine-readout label: JetBrains Mono, 11px, UPPERCASE.
///
/// Tracking (web 0.06em) is not exposed by GPUI's `Styled` API — skipped.
/// Only ASCII input is uppercased; CJK labels pass through untouched.
pub fn mono_label(text: impl Into<String>) -> Div {
    let text = text.into();
    let upper = if text.is_ascii() {
        text.to_uppercase()
    } else {
        text
    };
    div()
        .font_family(tokens::FONT_MONO)
        .text_size(px(tokens::FONT_SIZE_LABEL))
        .child(upper)
}

/// Body text in the Cobalt body face at the design-scale size.
pub fn body(text: impl IntoElement) -> Div {
    div()
        .font_family(tokens::FONT_BODY)
        .text_size(px(tokens::FONT_SIZE_BODY))
        .child(text)
}

/// Display voice (Space Grotesk family, fallback handled by GPUI).
pub fn display(text: impl IntoElement) -> Div {
    div()
        .font_family(tokens::FONT_DISPLAY)
        .text_size(px(tokens::FONT_SIZE_DISPLAY))
        .child(text)
}

/// Markdown rendered at the reading measure (web `TextView::markdown`).
pub fn markdown(id: impl Into<ElementId>, content: String) -> TextView {
    TextView::markdown(id, content).w_full()
}
