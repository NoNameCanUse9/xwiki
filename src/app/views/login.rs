//! Login and password-reset view.
//! State and network operations stay in `crate::app` (mod.rs).

use gpui::prelude::*;
use gpui::*;
use guise::{Button, ColorName, Icon, IconName, Size, Variant, theme};

use crate::app::XWikiApp;
use crate::ui::tokens::Cobalt;
use crate::ui::{app_icon, body, mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_login(&self, cx: &mut Context<Self>) -> Div {
        let t = theme(cx);
        let cobalt = Cobalt::from_theme(t);
        let success = t.success().hsla();
        let server = self.server_input.read(cx).text();
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
            cobalt.accent
        } else if self.login_error.is_some() || reset_failed {
            cobalt.danger
        } else {
            cobalt.ink_3
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
                .border_color(cobalt.danger)
                .bg(cobalt.danger.opacity(0.1))
                .child(
                    Icon::new(IconName::CircleX)
                        .size(Size::Sm)
                        .color(ColorName::Red),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .whitespace_normal()
                        .text_xs()
                        .text_color(cobalt.danger)
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
                .border_color(success)
                .bg(success.opacity(0.1))
                .child(
                    Icon::new(IconName::CircleCheck)
                        .size(Size::Sm)
                        .color(ColorName::Green),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .whitespace_normal()
                        .text_xs()
                        .text_color(success)
                        .child(msg.clone()),
                )
        } else {
            div()
        };

        let reset_feedback = if let Some((ok, msg)) = &self.reset_status {
            let color = if *ok { success } else { cobalt.danger };
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
                    .size(Size::Sm)
                    .color(if *ok {
                        ColorName::Green
                    } else {
                        ColorName::Red
                    }),
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
                .flex()
                .flex_col()
                .gap_3()
                .child(mono_label("步骤 1 · 请求一次性 token").text_color(cobalt.ink_3))
                .child(
                    Button::new(
                        "request-reset",
                        if self.loading {
                            "请求中…"
                        } else {
                            "请求 token"
                        },
                    )
                    .variant(Variant::Outline)
                    .radius(Size::Sm)
                    .full_width(true)
                    .left_section(Icon::new(IconName::ArrowRight).size(Size::Sm))
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| this.request_reset(cx))),
                )
                .child(reset_feedback)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(mono_label("一次性 token").text_color(cobalt.ink_3))
                        .child(self.reset_token_input.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(mono_label("新密码（至少 8 位）").text_color(cobalt.ink_3))
                        .child(self.reset_password_input.clone()),
                )
                .child(
                    Button::new(
                        "submit-reset",
                        if self.loading {
                            "提交中…"
                        } else {
                            "重置密码"
                        },
                    )
                    .variant(Variant::Filled)
                    .color(cobalt.accent)
                    .radius(Size::Sm)
                    .full_width(true)
                    .left_section(Icon::new(IconName::Check).size(Size::Sm))
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| this.submit_reset(cx))),
                )
                .child(
                    div().w_full().flex().child(
                        Button::new("back-to-login", "返回登录")
                            .variant(Variant::Subtle)
                            .size(Size::Xs)
                            .radius(Size::Sm)
                            .left_section(Icon::new(IconName::ArrowLeft).size(Size::Xs))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_reset_mode(cx))),
                    ),
                )
        } else {
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(mono_label("服务地址").text_color(cobalt.ink_3))
                        .child(self.server_input.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(mono_label("用户名").text_color(cobalt.ink_3))
                        .child(self.user_input.clone()),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(mono_label("密码").text_color(cobalt.ink_3))
                        .child(self.password_input.clone()),
                )
                .child(login_feedback)
                .child(
                    Button::new(
                        "login",
                        if self.loading {
                            "登录中…"
                        } else {
                            "登录"
                        },
                    )
                    .variant(Variant::Filled)
                    .color(cobalt.accent)
                    .radius(Size::Sm)
                    .full_width(true)
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, window, cx| this.do_login(window, cx))),
                )
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cobalt.paper)
            .child(
                div()
                    .id("login-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .overflow_y_scroll()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_4()
                    .py_6()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(tokens::LOGIN_PANEL + 56.0))
                            .flex()
                            .flex_col()
                            .items_center()
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
                                                    .text_color(cobalt.accent)
                                                    .child("XWIKI"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(cobalt.ink_3)
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
                                    .p_8()
                                    .rounded(px(tokens::RADIUS))
                                    .border_1()
                                    .border_color(cobalt.rule)
                                    .bg(cobalt.paper_2)
                                    .flex()
                                    .flex_col()
                                    .gap_5()
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .font_family(tokens::FONT_DISPLAY)
                                                    .text_color(cobalt.ink)
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
                                                .text_color(cobalt.ink_3),
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
                    .border_color(cobalt.rule)
                    .bg(cobalt.paper_2)
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
                            .text_color(cobalt.ink_3)
                            .child(if server.trim().is_empty() {
                                "未设置服务地址".to_string()
                            } else {
                                tokens::truncate(&server, 72)
                            }),
                    ),
            )
    }
}
