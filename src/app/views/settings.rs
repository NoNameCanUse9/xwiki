//! Settings and access-control view.
//! State and network operations stay in `crate::app` (mod.rs).

use gpui::*;
use gpui_component::{button::*, input::Input, scroll::ScrollableElement as _, *};

use crate::app::{Screen, XWikiApp};
use crate::ui::{mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> Div {
        let header = self.render_settings_header(cx);
        let service = self.render_settings_service(cx);
        let access = self.render_settings_access(cx);
        let workspace = self.render_settings_workspace(cx);

        div().size_full().flex().flex_col().child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scrollbar()
                .p_6()
                .flex()
                .items_start()
                .justify_center()
                .child(
                    div()
                        .w_full()
                        .flex_none()
                        .max_w(px(920.0))
                        .v_flex()
                        .gap_5()
                        .children([header, service, access, workspace]),
                ),
        )
    }

    fn render_settings_header(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .flex()
            .items_start()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(mono_label("SETTINGS").text_color(theme.accent))
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child("设置"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("管理服务连接、访问凭证和桌面工作区。"),
                    ),
            )
            .child(
                Button::new("settings-back")
                    .rounded(px(tokens::RADIUS))
                    .icon(IconName::ArrowLeft)
                    .label("返回工作台")
                    .tooltip("返回工作台 (Esc)")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.screen = Screen::Workspace;
                        cx.notify();
                    })),
            )
    }

    fn render_settings_service(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let access_busy = self.settings_access_loading;
        let test_busy = self.settings_loading;
        let status: AnyElement = if let Some((ok, message)) = &self.settings_test {
            div()
                .flex()
                .items_center()
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
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(if *ok { theme.success } else { theme.danger })
                        .child(message.clone()),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded(px(tokens::RADIUS))
            .border_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(mono_label("SERVICE").text_color(theme.muted_foreground))
            .child(
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("服务地址"),
            )
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
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Network)
                            .label(if test_busy {
                                "测试中…"
                            } else {
                                "测试连接"
                            })
                            .loading(test_busy)
                            .disabled(test_busy || access_busy)
                            .tooltip("检查服务器是否可达")
                            .on_click(cx.listener(|this, _, _, cx| this.test_connection(cx))),
                    )
                    .child(
                        Button::new("settings-save")
                            .primary()
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Check)
                            .label("保存")
                            .disabled(test_busy)
                            .on_click(cx.listener(|this, _, _, cx| this.save_server_settings(cx))),
                    ),
            )
            .child(status)
    }

    fn render_settings_access(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let access_busy = self.settings_access_loading;
        let test_busy = self.settings_loading;
        let tokens_empty = self.settings_tokens.is_empty();
        let users_empty = self.settings_users.is_empty();

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
                    .p_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(
                                div()
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
                    .p_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(
                                div()
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
                    .child(
                        Button::new(format!("toggle-user-{id}"))
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
                            })),
                    )
                    .into_any_element()
            })
            .collect();

        let token_content: AnyElement = if access_busy {
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在加载访问 Token…")
                .into_any_element()
        } else if tokens_empty {
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("暂无访问 Token。")
                .into_any_element()
        } else {
            div()
                .v_flex()
                .gap_2()
                .children(token_rows)
                .into_any_element()
        };
        let user_content: AnyElement = if access_busy {
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在加载用户…")
                .into_any_element()
        } else if users_empty {
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("暂无其他用户。")
                .into_any_element()
        } else {
            div()
                .v_flex()
                .gap_2()
                .children(user_rows)
                .into_any_element()
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
                        .rounded(px(tokens::RADIUS_SMALL))
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
            .gap_3()
            .p_4()
            .rounded(px(tokens::RADIUS))
            .border_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(mono_label("ACCESS CONTROL").text_color(theme.muted_foreground))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("create-token")
                                    .compact()
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Plus)
                                    .label("Token")
                                    .disabled(access_busy || test_busy)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_create_token_dialog(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("create-user")
                                    .compact()
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Plus)
                                    .label("用户")
                                    .disabled(access_busy || test_busy)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_create_user_dialog(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("refresh-access")
                                    .compact()
                                    .rounded(px(tokens::RADIUS))
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
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(320.0))
                            .v_flex()
                            .gap_2()
                            .child(mono_label("TOKENS").text_color(theme.muted_foreground))
                            .child(token_content),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(320.0))
                            .v_flex()
                            .gap_2()
                            .child(mono_label("USERS").text_color(theme.muted_foreground))
                            .child(user_content),
                    ),
            )
    }

    fn render_settings_workspace(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let dark = cx.theme().is_dark();
        div()
            .v_flex()
            .gap_3()
            .p_4()
            .rounded(px(tokens::RADIUS))
            .border_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(mono_label("WORKSPACE").text_color(theme.muted_foreground))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(280.0))
                            .v_flex()
                            .gap_1()
                            .child(mono_label("CURRENT USER").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(self.username.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(280.0))
                            .v_flex()
                            .gap_1()
                            .child(mono_label("THEME").text_color(theme.muted_foreground))
                            .child(
                                Button::new("settings-theme")
                                    .rounded(px(tokens::RADIUS))
                                    .icon(if dark { IconName::Sun } else { IconName::Moon })
                                    .label(if dark {
                                        "深色 · 切换为浅色"
                                    } else {
                                        "浅色 · 切换为深色"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(280.0))
                            .v_flex()
                            .gap_1()
                            .child(mono_label("LAYOUT").text_color(theme.muted_foreground))
                            .child(div().text_sm().text_color(theme.muted_foreground).child(
                                format!(
                                    "项目侧栏 {}px · 文档树 {}px · 历史面板 {}px",
                                    self.layout.projects_rail as i32,
                                    self.layout.doc_rail as i32,
                                    self.layout.history as i32,
                                ),
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(280.0))
                            .v_flex()
                            .gap_1()
                            .child(mono_label("SESSION").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child("退出当前登录会话"),
                                    )
                                    .child(
                                        Button::new("settings-logout")
                                            .danger()
                                            .compact()
                                            .icon(IconName::ArrowLeft)
                                            .label("退出登录")
                                            .tooltip("退出登录")
                                            .disabled(self.editing || self.saving)
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.logout(cx)),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
