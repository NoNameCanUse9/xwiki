//! Cobalt UI kit: design tokens + small shared styling helpers.
//!
//! Views should consume `tokens::*` and the helpers here instead of raw
//! literals, so the desktop app stays aligned with the web Cobalt language.

pub mod split_pane;
pub mod tokens;

use std::sync::{Arc, OnceLock};

use gpui::{
    div, img, px, Div, ElementId, Entity, EntityId, Image, ImageFormat, Img, IntoElement,
    ParentElement as _, Styled as _,
};
use gpui_component::button::Button;
use gpui_component::input::InputState;
use gpui_component::text::TextView;
use gpui_component::IconName;

const APP_ICON_SVG: &[u8] = include_bytes!("../../assets/xwiki-icon.svg");
static APP_ICON_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();

/// The XWiki brand mark, embedded at compile time so packaged builds do
/// not depend on the source tree at runtime.
pub fn app_icon() -> Img {
    let image = APP_ICON_IMAGE
        .get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Svg, APP_ICON_SVG.to_vec())))
        .clone();
    img(image).flex_none()
}

/// The XWiki icon rasterized for GPUI's X11 `WindowOptions.icon`.
/// Windows uses the executable resource embedded by `build.rs` instead.
/// Rendered once via resvg at 256×256; the PNG round-trip unpacks tiny_skia's
/// premultiplied pixels into plain RGBA, which the windowing system expects.
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

/// Markdown rendered at the reading measure (web `TextView::markdown`).
pub fn markdown(id: impl Into<ElementId>, content: String) -> TextView {
    TextView::markdown(id, content).w_full()
}

/// The "清空搜索" empty-state button shared by the project grid and the
/// history timeline: clears the given search input and repaints `app_id`
/// (the owning view entity, since plain `App` has no no-arg `notify`).
pub fn clear_search_button(
    id: impl Into<ElementId>,
    input: Entity<InputState>,
    app_id: EntityId,
) -> Button {
    Button::new(id)
        .rounded(px(tokens::RADIUS))
        .icon(IconName::Close)
        .label("清空搜索")
        .on_click(move |_, window, cx| {
            input.update(cx, |state, cx| {
                state.set_value(String::new(), window, cx);
            });
            cx.notify(app_id);
        })
}

#[cfg(test)]
mod tests {
    use gpui_component::{
        highlighter::{HighlightTheme, LanguageRegistry, SyntaxHighlighter},
        Rope,
    };
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

    fn assert_fenced_code_highlights(source: &str) {
        let (language, code) = fenced_code(source);
        let config = LanguageRegistry::singleton()
            .language(&language)
            .unwrap_or_else(|| panic!("language {language:?} should be registered"));
        assert!(
            config.has_grammar(),
            "language {language:?} should have a tree-sitter grammar"
        );

        let mut highlighter = SyntaxHighlighter::new(&language);
        let rope = Rope::from_str(&code);
        assert!(highlighter.update(None, &rope, None));

        for theme in [
            HighlightTheme::default_light(),
            HighlightTheme::default_dark(),
        ] {
            let styles = highlighter.styles(&(0..code.len()), &theme);
            assert!(
                styles.iter().any(|(_, style)| style.color.is_some()),
                "{language} fenced code should contain colored syntax spans in {}",
                theme.name
            );
        }
    }

    fn contrast_ratio(foreground: &str, background: &str) -> f32 {
        fn luminance(hex: &str) -> f32 {
            let channels = [1, 3, 5].map(|offset| {
                let value = u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap() as f32 / 255.;
                if value <= 0.03928 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            });
            0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
        }

        let foreground = luminance(foreground);
        let background = luminance(background);
        (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
    }

    #[test]
    fn fenced_rust_and_javascript_code_have_light_and_dark_highlight_spans() {
        assert_fenced_code_highlights("```rust\nfn main() {\n    let answer = 42;\n}\n```");
        assert_fenced_code_highlights("```javascript\nconst answer = \"ok\";\n```");
    }

    #[test]
    fn light_code_highlight_colors_are_readable_on_the_code_block_surface() {
        let theme: serde_json::Value =
            serde_json::from_str(include_str!("../themes/cobalt.json")).unwrap();
        let light = &theme["themes"][0];
        let background = light["colors"]["muted.background"].as_str().unwrap();

        for token in ["variable", "string", "keyword", "function", "type"] {
            let color = light["highlight"]["syntax"][token]["color"]
                .as_str()
                .unwrap_or_else(|| panic!("missing Light syntax color for {token}"));
            assert!(
                contrast_ratio(color, background) >= 4.5,
                "Light {token} color {color} is not readable on {background}"
            );
        }
    }
}
