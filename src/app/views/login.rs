//! Login and password-reset view.
//! State and network operations stay in `crate::app` (mod.rs).

use gpui::*;
use gpui_component::{button::*, input::Input, scroll::ScrollableElement as _, *};

use crate::app::XWikiApp;
use crate::ui::{app_icon, body, mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_login(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let server = self.server_input.read(cx).value().to_string();
        let reset_failed = self
            .reset_status
            .as_ref()
            .map(|(ok, _)| !*ok)
            .unwrap_or(false);
        let status_label = if self.loading {
            "连接中 · 正在验证会话"
        } else if self.login_error.is_some() || reset_failed {
            "连接失败 · 请检查输入"
        } else {
            "待连接 · 登录以继续"
        };
        let status_color = if self.loading {
            theme.accent
        } else if self.login_error.is_some() || reset_failed {
            theme.danger
        } else {
            theme.muted_foreground
        };

        let login_feedback = if let Some(err) = &self.login_error {
            div()
                .w_full()
                .flex()
                .items_start()
                .gap_2()
                .p_3()
                .rounded(px(tokens::RADIUS_SMALL))
                .border_1()
                .border_color(theme.danger)
                .bg(theme.danger.opacity(0.1))
                .child(Icon::new(IconName::CircleX).text_color(theme.danger))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .whitespace_normal()
                        .text_xs()
                        .text_color(theme.danger)
                        .child(err.clone()),
                )
        } else if let Some((true, msg)) = &self.reset_status {
            div()
                .w_full()
                .flex()
                .items_start()
                .gap_2()
                .p_3()
                .rounded(px(tokens::RADIUS_SMALL))
                .border_1()
                .border_color(theme.success)
                .bg(theme.success.opacity(0.1))
                .child(Icon::new(IconName::CircleCheck).text_color(theme.success))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .whitespace_normal()
                        .text_xs()
                        .text_color(theme.success)
                        .child(msg.clone()),
                )
        } else {
            div()
        };

        let reset_feedback = if let Some((ok, msg)) = &self.reset_status {
            let color = if *ok { theme.success } else { theme.danger };
            div()
                .w_full()
                .flex()
                .items_start()
                .gap_2()
                .p_3()
                .rounded(px(tokens::RADIUS_SMALL))
                .border_1()
                .border_color(color)
                .bg(color.opacity(0.1))
                .child(
                    Icon::new(if *ok {
                        IconName::CircleCheck
                    } else {
                        IconName::CircleX
                    })
                    .text_color(color),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .whitespace_normal()
                        .text_xs()
                        .text_color(color)
                        .child(msg.clone()),
                )
        } else {
            div()
        };

        let form = if self.reset_mode {
            div()
                .w_full()
                .v_flex()
                .gap_3()
                .child(mono_label("步骤 1 · 请求一次性 token").text_color(theme.muted_foreground))
                .child(
                    Button::new("request-reset")
                        .secondary()
                        .w_full()
                        .rounded(px(tokens::RADIUS))
                        .icon(IconName::ArrowRight)
                        .label(if self.loading {
                            "请求中…"
                        } else {
                            "请求 token"
                        })
                        .loading(self.loading)
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| this.request_reset(cx))),
                )
                .child(reset_feedback)
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .child(mono_label("一次性 token").text_color(theme.muted_foreground))
                        .child(Input::new(&self.reset_token_input).w_full()),
                )
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .child(mono_label("新密码（至少 8 位）").text_color(theme.muted_foreground))
                        .child(Input::new(&self.reset_password_input).w_full()),
                )
                .child(
                    Button::new("submit-reset")
                        .primary()
                        .w_full()
                        .rounded(px(tokens::RADIUS))
                        .icon(IconName::Check)
                        .label(if self.loading {
                            "提交中…"
                        } else {
                            "重置密码"
                        })
                        .loading(self.loading)
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| this.submit_reset(cx))),
                )
                .child(
                    Button::new("back-to-login")
                        .ghost()
                        .w_full()
                        .rounded(px(tokens::RADIUS))
                        .icon(IconName::ArrowLeft)
                        .label("返回登录")
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_reset_mode(cx))),
                )
        } else {
            div()
                .w_full()
                .v_flex()
                .gap_4()
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .child(mono_label("服务地址").text_color(theme.muted_foreground))
                        .child(Input::new(&self.server_input).w_full()),
                )
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .child(mono_label("用户名").text_color(theme.muted_foreground))
                        .child(Input::new(&self.user_input).w_full()),
                )
                .child(
                    div()
                        .w_full()
                        .v_flex()
                        .gap_1()
                        .child(mono_label("密码").text_color(theme.muted_foreground))
                        .child(Input::new(&self.password_input).w_full()),
                )
                .child(login_feedback)
                .child(
                    Button::new("login")
                        .primary()
                        .w_full()
                        .rounded(px(tokens::RADIUS))
                        .label(if self.loading {
                            "登录中…"
                        } else {
                            "登录"
                        })
                        .loading(self.loading)
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, window, cx| this.do_login(window, cx))),
                )
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .overflow_y_scrollbar()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_4()
                    .py_6()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(tokens::LOGIN_PANEL + 56.0))
                            .v_flex()
                            .gap_4()
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(app_icon().size(px(28.0)))
                                            .child(
                                                div()
                                                    .font_family(tokens::FONT_MONO)
                                                    .text_xs()
                                                    .text_color(theme.accent)
                                                    .child("AGENTDOCS"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(if self.reset_mode {
                                                "PASSWORD RESET"
                                            } else {
                                                "SECURE SIGN-IN"
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .max_w(px(tokens::LOGIN_PANEL))
                                    .self_center()
                                    .p_8()
                                    .rounded(px(tokens::RADIUS))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.sidebar)
                                    .v_flex()
                                    .gap_5()
                                    .child(
                                        div()
                                            .w_full()
                                            .v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .font_family(tokens::FONT_DISPLAY)
                                                    .text_color(theme.foreground)
                                                    .child(if self.reset_mode {
                                                        "重置密码"
                                                    } else {
                                                        "登录以继续"
                                                    }),
                                            )
                                            .child(
                                                body(if self.reset_mode {
                                                    "请求一次性 token 后设置新的登录密码"
                                                } else {
                                                    "使用管理员账号访问你的文档工作台"
                                                })
                                                .text_color(theme.muted_foreground),
                                            ),
                                    )
                                    .child(form),
                            ),
                    ),
            )
            .child(
                div()
                    .h(px(tokens::STATUS_H))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(div().size_2().rounded_full().bg(status_color))
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(status_color)
                            .child(status_label),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .max_w(px(360.0))
                            .overflow_x_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(if server.trim().is_empty() {
                                "未设置服务地址".to_string()
                            } else {
                                tokens::truncate(&server, 72)
                            }),
                    ),
            )
    }
}
