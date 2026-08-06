//! Settings (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::*;
use gpui_component::{
    button::*,
    input::Input,
    *,
};

use crate::app::{Screen, XWikiApp};
use crate::ui::{mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Header: mono label + back to workspace.
                div()
                    .h(px(tokens::TOOLBAR_H))
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(mono_label("SETTINGS").text_color(theme.muted_foreground))
                    .child(
                        Button::new("settings-back")
                            .rounded(px(tokens::RADIUS))
                            .label("← 返回工作台")
                            .tooltip("返回工作台 (Esc)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.screen = Screen::Workspace;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .p_6()
                    .child(
                        div()
                            .w(px(560.0))
                            .v_flex()
                            .gap_4()
                            .child(mono_label("服务地址").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&self.settings_server_input).w_full()),
                                    )
                                    .child(
                                        Button::new("settings-test")
                                            .rounded(px(tokens::RADIUS))
                                            .label("测试连接")
                                            .tooltip("检查服务器是否可达")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.test_connection(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("settings-save")
                                            .primary()
                                            .rounded(px(tokens::RADIUS))
                                            .label("保存")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.save_server_settings(cx)
                                            })),
                                    ),
                            )
                            .child(if let Some((ok, msg)) = &self.settings_test {
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(if *ok { theme.success_foreground } else { theme.danger })
                                    .child(msg.clone())
                            } else {
                                div()
                            })
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("当前用户").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(self.username.clone()),
                            )
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("主题").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(if cx.theme().is_dark() { "深色" } else { "浅色" }),
                            )
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("布局").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "项目侧栏 {}px · 文档树 {}px · 历史面板 {}px",
                                        self.layout.projects_rail as i32,
                                        self.layout.doc_rail as i32,
                                        self.layout.history as i32,
                                    )),
                            ),
                    ),
            )
    }

}
