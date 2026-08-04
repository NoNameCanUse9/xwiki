use gpui::{div, hsla, prelude::*, Window};

pub struct XWikiApp;

impl Render for XWikiApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .bg(gpui::white())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(gpui::black())
                            .child("XWiki GPUI App"),
                    )
                    .child(
                        div()
                            .text_lg()
                            .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                            .child("Welcome to XWiki GPUI Component Project"),
                    ),
            )
    }
}
