//! Settings and access-control view.
//! State and network operations stay in `crate::app` (mod.rs).
//!
//! Layout follows the Stitch "设置 - AgentDocs (Optimized)" demo: a plain
//! content column with mono UPPERCASE section labels, hairline rules and
//! compact outline buttons — no card containers.

use gpui::*;
use gpui_component::{button::*, input::Input, scroll::ScrollableElement as _, *};

use crate::app::{Screen, XWikiApp};
use crate::ui::{mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scrollbar()
                    .flex()
                    .items_start()
                    .justify_center()
                    .child(
                        div()
                            .w_full()
                            .flex_none()
                            .max_w(px(920.0))
                            .p_6()
                            .v_flex()
                            .gap_4()
                            .child(self.render_settings_header(cx))
                            .child(self.render_settings_service(cx))
                            .child(self.render_settings_token(cx))
                            .child(self.render_settings_user(cx))
                            .child(self.render_settings_audit(cx))
                            .child(self.render_settings_session(cx)),
                    ),
            )
    }

    fn render_settings_header(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .flex()
            .items_center()
            .gap_3()
            .pb_4()
            .border_b_1()
            .border_color(theme.border)
            .child(
                Button::new("settings-back")
                    .ghost()
                    .compact()
                    .icon(IconName::ArrowLeft)
                    .label("返回工作台")
                    .tooltip("返回工作台 (Esc)")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.screen = Screen::Workspace;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .font_family(tokens::FONT_DISPLAY)
                    .text_3xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.foreground)
                    .child("设置"),
            )
    }

    fn render_settings_service(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let test_busy = self.settings_loading;
        let test_failed = matches!(self.settings_test.as_ref(), Some((false, _)));
        let test_detail: AnyElement = if let Some(detail) = &self.settings_test_detail {
            div()
                .font_family(tokens::FONT_MONO)
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(detail.clone())
                .into_any_element()
        } else {
            div().into_any_element()
        };
        let status: AnyElement = if let Some((ok, message)) = &self.settings_test {
            div()
                .flex()
                .items_start()
                .gap_2()
                .p_2()
                .rounded(px(tokens::RADIUS_SMALL))
                .border_1()
                .border_color(if *ok { theme.success } else { theme.danger })
                .bg(if *ok {
                    theme.success.opacity(0.1)
                } else {
                    theme.danger.opacity(0.1)
                })
                .child(Icon::new(if *ok {
                    IconName::CircleCheck
                } else {
                    IconName::CircleX
                }))
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(if *ok { theme.success } else { theme.danger })
                                .child(message.clone()),
                        )
                        .child(test_detail),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        div()
            .v_flex()
            .gap_3()
            .p_4()
            .child(mono_label("服务地址").text_color(theme.muted_foreground))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(260.0))
                            .child(Input::new(&self.settings_server_input).w_full()),
                    )
                    .child(
                        Button::new("settings-test")
                            .outline()
                            .compact()
                            .icon(IconName::Network)
                            .label(if test_busy {
                                "测试中…"
                            } else {
                                "测试连接"
                            })
                            .loading(test_busy)
                            .disabled(test_busy)
                            .tooltip("检查服务器是否可达")
                            .on_click(cx.listener(|this, _, _, cx| this.test_connection(cx))),
                    )
                    .child(
                        Button::new("settings-save")
                            .primary()
                            .compact()
                            .icon(IconName::Check)
                            .label("保存")
                            .disabled(test_busy || test_failed)
                            .on_click(cx.listener(|this, _, _, cx| this.save_server_settings(cx))),
                    ),
            )
            .child(status)
    }

    fn render_settings_token(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let access_busy = self.settings_access_loading;
        let test_busy = self.settings_loading;
        let tokens_empty = self.settings_tokens.is_empty();

        let token_rows: Vec<AnyElement> = self
            .settings_tokens
            .iter()
            .map(|token| {
                let id = token.id.clone();
                let token_name = token.name.clone();
                let revoked = !token.revoked_at.is_empty();
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(token.name.clone()),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(if revoked {
                                        "已撤销".to_string()
                                    } else {
                                        format!("{} · {}", token.scope, token.created_at)
                                    }),
                            ),
                    )
                    .child(
                        Button::new(format!("revoke-token-{id}"))
                            .danger()
                            .outline()
                            .compact()
                            .icon(IconName::Delete)
                            .label("撤销")
                            .disabled(revoked || access_busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_revoke_token(
                                    window,
                                    cx,
                                    id.clone(),
                                    token_name.clone(),
                                );
                            })),
                    )
                    .into_any_element()
            })
            .collect();

        let token_content: AnyElement = if access_busy {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在加载访问 Token…")
                .into_any_element()
        } else if tokens_empty {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("暂无访问 Token。")
                .into_any_element()
        } else {
            div().v_flex().children(token_rows).into_any_element()
        };

        let status: AnyElement = if let Some(secret) = &self.settings_token_secret {
            div()
                .flex()
                .items_start()
                .gap_2()
                .p_3()
                .rounded(px(tokens::RADIUS_SMALL))
                .border_1()
                .border_color(theme.accent)
                .bg(theme.accent.opacity(0.1))
                .child(Icon::new(IconName::CircleCheck).text_color(theme.accent))
                .child(
                    div()
                        .flex_1()
                        .v_flex()
                        .gap_1()
                        .child(mono_label("新 Token 密钥（只显示一次）").text_color(theme.accent))
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.foreground)
                                .child(secret.clone()),
                        ),
                )
                .into_any_element()
        } else if let Some(error) = &self.settings_error {
            div()
                .flex()
                .items_start()
                .gap_2()
                .p_2()
                .rounded(px(tokens::RADIUS_SMALL))
                .border_1()
                .border_color(theme.danger)
                .bg(theme.danger.opacity(0.1))
                .child(Icon::new(IconName::CircleX).text_color(theme.danger))
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(theme.danger)
                        .child(error.clone()),
                )
                .child(
                    Button::new("retry-access")
                        .outline()
                        .compact()
                        .icon(IconName::Redo2)
                        .label("重试")
                        .disabled(access_busy)
                        .on_click(cx.listener(|this, _, _, cx| this.load_settings_access(cx))),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        div()
            .v_flex()
            .gap_2()
            .p_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(mono_label("Token 管理").text_color(theme.muted_foreground))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("create-token")
                                    .outline()
                                    .compact()
                                    .icon(IconName::Plus)
                                    .label("新建")
                                    .disabled(access_busy || test_busy)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_create_token_dialog(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("refresh-access")
                                    .outline()
                                    .compact()
                                    .icon(IconName::Redo2)
                                    .label(if access_busy {
                                        "加载中…"
                                    } else {
                                        "刷新"
                                    })
                                    .loading(access_busy)
                                    .disabled(access_busy || test_busy)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.load_settings_access(cx)),
                                    ),
                            ),
                    ),
            )
            .child(status)
            .child(
                div()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(token_content),
            )
    }

    fn render_settings_user(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let access_busy = self.settings_access_loading;
        let test_busy = self.settings_loading;
        let users_empty = self.settings_users.is_empty();

        let user_rows: Vec<AnyElement> = self
            .settings_users
            .iter()
            .map(|user| {
                let id = user.id.clone();
                let user_name = user.username.clone();
                let enabled = !user.disabled;
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(user.username.clone()),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(if user.is_admin {
                                        "管理员"
                                    } else {
                                        "普通用户"
                                    }),
                            ),
                    )
                    .child({
                        let button = Button::new(format!("toggle-user-{id}"))
                            .danger()
                            .outline()
                            .compact()
                            .icon(if enabled {
                                IconName::CircleX
                            } else {
                                IconName::CircleCheck
                            })
                            .label(if enabled { "停用" } else { "启用" })
                            .disabled(access_busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if enabled {
                                    this.confirm_disable_user(
                                        window,
                                        cx,
                                        id.clone(),
                                        user_name.clone(),
                                    );
                                } else {
                                    this.set_user_enabled(&id, true, cx);
                                }
                            }));
                        if !enabled {
                            button.ghost()
                        } else {
                            button
                        }
                    })
                    .into_any_element()
            })
            .collect();

        let user_content: AnyElement = if access_busy {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在加载用户…")
                .into_any_element()
        } else if users_empty {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("暂无其他用户。")
                .into_any_element()
        } else {
            div().v_flex().children(user_rows).into_any_element()
        };

        div()
            .v_flex()
            .gap_2()
            .p_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(mono_label("用户管理").text_color(theme.muted_foreground))
                    .child(
                        Button::new("create-user")
                            .outline()
                            .compact()
                            .icon(IconName::Plus)
                            .label("新建用户")
                            .disabled(access_busy || test_busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_create_user_dialog(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(user_content),
            )
    }

    fn render_settings_audit(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let audit_content: AnyElement = if self.audit_loading {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在加载审计日志…")
                .into_any_element()
        } else if let Some(err) = &self.audit_error {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.danger)
                .child(err.clone())
                .into_any_element()
        } else if self.audit_entries.is_empty() {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("暂无审计记录。")
                .into_any_element()
        } else {
            let mut rows = div().v_flex();
            for e in &self.audit_entries {
                rows = rows.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .py_2()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .v_flex()
                                .gap_0p5()
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.foreground)
                                        .child(format!("{} · {}", e.actor_id, e.action)),
                                )
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(if e.path.is_empty() {
                                            e.created_at.clone()
                                        } else {
                                            format!("{} · {}", e.path, e.created_at)
                                        }),
                                ),
                        ),
                );
            }
            rows.into_any_element()
        };
        div()
            .v_flex()
            .gap_2()
            .p_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(mono_label("审计日志").text_color(theme.muted_foreground))
                    .child(
                        Button::new("refresh-audit")
                            .outline()
                            .compact()
                            .icon(IconName::Redo2)
                            .label(if self.audit_loading {
                                "加载中…"
                            } else {
                                "刷新"
                            })
                            .loading(self.audit_loading)
                            .on_click(cx.listener(|this, _, _, cx| this.load_audit(cx))),
                    ),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(audit_content),
            )
    }

    fn render_settings_session(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let dark = cx.theme().is_dark();
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(mono_label("当前用户").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(self.username.clone()),
                            ),
                    )
                    .child(
                        Button::new("settings-logout")
                            .danger()
                            .outline()
                            .compact()
                            .icon(IconName::CircleX)
                            .label("退出登录")
                            .tooltip("退出登录")
                            .disabled(self.editing || self.saving)
                            .on_click(cx.listener(|this, _, _, cx| this.logout(cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .pt_3()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(mono_label("界面主题").text_color(theme.muted_foreground))
                    .child(
                        Button::new("settings-theme")
                            .outline()
                            .compact()
                            .icon(if dark { IconName::Sun } else { IconName::Moon })
                            .label(if dark { "浅色模式" } else { "深色模式" })
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
                    ),
            )
    }
}
