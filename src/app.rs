use gpui::*;
use gpui_component::{
    button::*,
    input::{Input, InputEvent, InputState},
    *,
};

pub struct XWikiApp {
    input_state: Entity<InputState>,
    message: SharedString,
    /// Keep the input subscription alive with the app entity.
    _subscriptions: Vec<Subscription>,
}

impl XWikiApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索或创建文档…"));

        let _subscriptions = vec![cx.subscribe_in(&input_state, window, {
            let input_state = input_state.clone();
            move |this, _, ev: &InputEvent, _window, cx| {
                if let InputEvent::Change = ev {
                    this.message = format!("输入: {}", input_state.read(cx).value()).into();
                    cx.notify();
                }
            }
        })];

        Self {
            input_state,
            message: SharedString::default(),
            _subscriptions,
        }
    }
}

impl Render for XWikiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_4()
            .size_full()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .child(
                div()
                    .text_2xl()
                    .child("XWiki GPUI App")
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted)
                            .child("长桥 gpui-component 组件演示"),
                    ),
            )
            .child(Input::new(&self.input_state))
            .child(div().child(self.message.clone()))
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        Button::new("create")
                            .primary()
                            .label("创建文档")
                            .on_click(|_, _, _| println!("create clicked")),
                    )
                    .child(
                        Button::new("cancel")
                            .label("取消")
                            .on_click(|_, _, _| println!("cancel clicked")),
                    ),
            )
    }
}
