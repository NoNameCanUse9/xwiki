//! Login (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::*;
use gpui_component::{button::*, input::Input, *};

use crate::app::XWikiApp;
use crate::ui::tokens;

impl XWikiApp {
    pub(crate) fn render_login(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let cobalt = tokens::Cobalt::from_theme(theme);
        let graphite = cobalt.graphite;
        let graphite_soft = cobalt.graphite_soft;
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .w(px(tokens::LOGIN_WIDTH))
                    .gap(px(tokens::LOGIN_GAP))
                    .items_center()
                    .child(
                        // Left — brand statement + terminal card.
                        div()
                            .flex_1()
                            .v_flex()
                            .gap_5()
                            .child(
                                div()
                                    .v_flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.accent)
                                            .child("Git-backed documentation"),
                                    )
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .font_family(tokens::FONT_DISPLAY)
                                            .text_color(theme.foreground)
                                            .child("AgentDocs"),
                                    )
                                    .child(
                                        div()
                                            .w(px(tokens::LOGIN_TEXT))
                                            .child(
                                                crate::ui::body(
                                                    "面向人类与 AI Agent 的轻量文档管理系统。一项目一 Git 仓库，文档即版本，ChangeSet 原子提交。",
                                                )
                                                .text_color(theme.muted_foreground),
                                            ),
                                    ),
                            )
                            .child(
                                // The graphite code card — one dark beat.
                                div()
                                    .w_full()
                                    .rounded(px(tokens::RADIUS))
                                    .overflow_hidden()
                                    .bg(graphite)
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .px_4()
                                            .py_2()
                                            .border_b_1()
                                            .border_color(tokens::card_rule())
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_1_5()
                                                    .child(div().size_2_5().rounded_full().bg(tokens::card_dot()))
                                                    .child(div().size_2_5().rounded_full().bg(tokens::card_dot()))
                                                    .child(div().size_2_5().rounded_full().bg(tokens::card_dot())),
                                            )
                                            .child(
                                                div()
                                                    .font_family(tokens::FONT_MONO)
                                                    .text_xs()
                                                    .text_color(tokens::card_muted())
                                                    .child("agentdocs — session"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .v_flex()
                                            .gap_1_5()
                                            .px_4()
                                            .py_4()
                                            .child(
                                                div()
                                                    .font_family(tokens::FONT_MONO)
                                                    .text_sm()
                                                    .child(
                                                        div().flex().gap_2().child(
                                                            div().text_color(theme.accent).child("$"),
                                                        ).child(
                                                            div().text_color(tokens::card_title()).child("agentdocs admin create -username admin"),
                                                        ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .font_family(tokens::FONT_MONO)
                                                    .text_sm()
                                                    .text_color(graphite_soft)
                                                    .child("› argon2id · session persisted to sqlite"),
                                            )
                                            .child(
                                                div()
                                                    .font_family(tokens::FONT_MONO)
                                                    .text_sm()
                                                    .text_color(tokens::card_ok())
                                                    .child("✓ 200 OK — agentdocs_session set (HttpOnly)"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("phase 01 · skeleton · serve / admin create"),
                            ),
                    )
                    .child(
                        // Right — sign-in panel (hairline).
                        div()
                            .w(px(tokens::LOGIN_PANEL))
                            .p_8()
                            .rounded(px(tokens::RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.sidebar)
                            .v_flex()
                            .gap_4()
                            .child(
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child("登录以继续"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child("使用管理员账号访问你的文档工作台"),
                                    ),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("服务地址"),
                            )
                            .child(Input::new(&self.server_input).w_full())
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("用户名"),
                            )
                            .child(Input::new(&self.user_input).w_full())
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("密码"),
                            )
                            .child(Input::new(&self.password_input).w_full())
                            .child(if self.reset_mode {
                                // Forgot-password form: token + new password.
                                div().v_flex().gap_3().w_full()
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("重置密码"),
                                )
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("1 · 请求一次性 token（发往服务端日志）"),
                                )
                                .child(
                                    Button::new("request-reset")
                                        .rounded(px(tokens::RADIUS))
                                        .icon(IconName::ArrowRight)
                                        .label(if self.loading { "请求中…" } else { "请求 token" })
                                        .disabled(self.loading)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.request_reset(cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("2 · 输入 token 与新密码"),
                                )
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("一次性 token"),
                                ).child(Input::new(&self.reset_token_input).w_full())
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("新密码（至少 8 位）"),
                                )
                                .child(Input::new(&self.reset_password_input).w_full())
                                .child(if let Some((ok, msg)) = &self.reset_status {
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(if *ok { theme.success_foreground } else { theme.danger })
                                        .child(msg.clone())
                                } else {
                                    div()
                                })
                                .child(
                                    Button::new("submit-reset")
                                        .primary()
                                        .w_full()
                                        .rounded(px(tokens::RADIUS))
                                        .icon(IconName::Check)
                                        .label(if self.loading { "提交中…" } else { "重置密码" })
                                        .disabled(self.loading)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.submit_reset(cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .id("back-to-login")
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .hover(|s| s.text_color(theme.accent))
                                        .cursor_pointer()
                                        .child("← 返回登录")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_reset_mode(cx)
                                        })),
                                )
                            } else {
                                // Sign-in form.
                                div().v_flex().gap_3().w_full().child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .font_family(tokens::FONT_MONO)
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child("密码"),
                                        )
                                        .child(
                                            div()
                                                .id("forgot-password-link")
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .hover(|s| s.text_color(theme.accent))
                                                .cursor_pointer()
                                                .child("忘记密码？")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.toggle_reset_mode(cx)
                                                })),
                                        ),
                                )
                            })
                            .child(if let Some(err) = &self.login_error {
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.danger)
                                    .child(err.clone())
                            } else {
                                div()
                            })
                            .child(
                                Button::new("login")
                                    .primary()
                                    .w_full()
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::ArrowRight)
                                    .label(if self.loading { "登录中…" } else { "登录" })
                                    .disabled(self.loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.do_login(window, cx)
                                    })),
                            )
                            .child(if self.loading {
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.accent)
                                    .child("正在连接服务器并验证会话…")
                            } else {
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("session · argon2id · http-only cookie")
                            }),
                    ),
            )
    }
}
