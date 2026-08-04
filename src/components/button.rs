use gpui::{div, hsla, prelude::*, SharedString, Window};

pub struct Button {
    label: SharedString,
    on_click: Option<Box<dyn Fn(&mut Window, &mut gpui::Context<Self>)>>,
}

impl Button {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
        }
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut gpui::Context<Self>) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl Render for Button {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .px_4()
            .py_2()
            .rounded_md()
            .bg(gpui::blue())
            .text_color(gpui::white())
            .cursor_pointer()
            .hover(|this| this.bg(hsla(0.6, 0.8, 0.5, 1.0)))
            .child(self.label.clone())
    }
}
