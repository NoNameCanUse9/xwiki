use gpui::{div, hsla, prelude::*, SharedString, Window};

pub struct TextInput {
    value: SharedString,
    placeholder: SharedString,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            value: SharedString::default(),
            placeholder: SharedString::default(),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(hsla(0.0, 0.0, 0.5, 1.0))
            .text_color(gpui::black())
            .child(
                div()
                    .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                    .child(self.placeholder.clone()),
            )
    }
}
