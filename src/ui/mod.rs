//! Cobalt UI kit: design tokens + small shared styling helpers.
//!
//! Views should consume `tokens::*` and the helpers here instead of raw
//! literals, so the desktop app stays aligned with the web Cobalt language.

pub mod split_pane;
pub mod tokens;

use std::sync::{Arc, OnceLock};

use gpui::{
    div, img, px, App, AppContext, Div, ElementId, Entity, EntityId, Image, ImageFormat, Img,
    IntoElement, ParentElement as _, Styled as _,
};
use guise::input::TextInput;
use guise::markdown::MarkdownEditor;
use guise::Button;
use guise::{Icon, IconName};

const APP_ICON_SVG: &[u8] = include_bytes!("../../assets/xwiki-icon.svg");
const REFRESH_ICON_SVG: &[u8] = include_bytes!("../../assets/refresh.svg");
static APP_ICON_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
static REFRESH_ICON_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();

/// The XWiki brand mark, embedded at compile time so packaged builds do
/// not depend on the source tree at runtime.
pub fn app_icon() -> Img {
    let image = APP_ICON_IMAGE
        .get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Svg, APP_ICON_SVG.to_vec())))
        .clone();
    img(image).flex_none()
}

/// Standard two-arrow refresh mark used by the shell toolbar.
pub fn refresh_icon() -> Img {
    let image = REFRESH_ICON_IMAGE
        .get_or_init(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                REFRESH_ICON_SVG.to_vec(),
            ))
        })
        .clone();
    img(image).flex_none()
}

/// The XWiki icon rasterized for GPUI's X11 `WindowOptions.icon`.
/// Windows uses the executable resource embedded by `build.rs` instead.
/// Rendered once via resvg at 256×256; the PNG round-trip unpacks tiny_skia's
/// premultiplied pixels into plain RGBA, which the windowing system expects.
///
/// gpui 0.2.2 removed `WindowOptions.icon`, so nothing calls this today;
/// kept for when the platform API returns or for future window icon needs.
#[allow(dead_code)]
pub fn app_icon_rgba() -> Arc<image::RgbaImage> {
    static ICON: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    ICON.get_or_init(|| {
        let tree = usvg::Tree::from_data(APP_ICON_SVG, &usvg::Options::default())
            .expect("app icon svg parses");
        let size = 256;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("icon pixmap");
        let scale = size as f32 / tree.size().width();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        let png = pixmap.encode_png().expect("icon png");
        Arc::new(
            image::load_from_memory(&png)
                .expect("icon png decodes")
                .to_rgba8(),
        )
    })
    .clone()
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

/// Markdown rendered at the reading measure. Creates a read-only
/// [`MarkdownEditor`] entity (guise) wrapped in a full-width `Div`.
///
/// NOTE: the signature changed from gpui-component's stateless
/// `markdown(id, content) -> TextView` — it now needs `cx` to spawn the
/// editor entity. Callers pass their render `Context` (derefs to `&mut App`).
pub fn markdown(cx: &mut App, content: String) -> Div {
    let editor = cx.new(|cx| MarkdownEditor::new(cx).value(&content).read_only(true));
    div().w_full().child(editor)
}

/// The "清空搜索" empty-state button shared by the project grid and the
/// history timeline: clears the given search input and repaints `app_id`
/// (the owning view entity, since plain `App` has no no-arg `notify`).
pub fn clear_search_button(
    id: impl Into<ElementId>,
    input: Entity<TextInput>,
    app_id: EntityId,
) -> Button {
    Button::new(id, "清空搜索")
        .left_section(Icon::new(IconName::Close))
        .on_click(move |_, _window, cx| {
            input.update(cx, |state, cx| state.set_text("", cx));
            cx.notify(app_id);
        })
}

#[cfg(test)]
mod tests {
    use markdown::{mdast::Node, ParseOptions};

    fn fenced_code(source: &str) -> (String, String) {
        let Node::Root(root) = markdown::to_mdast(source, &ParseOptions::default())
            .expect("fenced Markdown should parse")
        else {
            panic!("expected a Markdown root");
        };

        let Some(Node::Code(code)) = root.children.first() else {
            panic!("expected one fenced code block");
        };

        (
            code.lang
                .clone()
                .expect("fenced code should have a language"),
            code.value.clone(),
        )
    }

    #[test]
    fn markdown_fenced_code_block_keeps_language_and_body() {
        let (language, code) = fenced_code("```rust\nfn main() {}\n```");
        assert_eq!(language, "rust");
        assert_eq!(code, "fn main() {}");
    }
}
