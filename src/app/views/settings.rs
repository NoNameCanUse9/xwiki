//! Settings and access-control view.
//! State and network operations stay in `crate::app` (mod.rs).
//!
//! Layout follows the Stitch "设置 - XWiki (Optimized)" demo: a plain
//! content column with mono UPPERCASE section labels, hairline rules and
//! compact outline buttons — no card containers.

use gpui::*;
use guise::prelude::*;
// Explicit (non-glob) import wins over both globs above: `Size` here is
// guise's component size scale (Xs..Xl), not gpui's geometry type.
use guise::theme::Size;

use crate::app::{Screen, XWikiApp};
use crate::ui::{mono_label, tokens};

const SETTINGS_ACTION_WIDTH: f32 = 116.0;
const SETTINGS_COMPACT_ACTION_WIDTH: f32 = SETTINGS_ACTION_WIDTH;
const SETTINGS_LABEL_WIDTH: f32 = 48.0;

#[derive(Clone, Copy)]
struct SettingsActionStyle {
    width: f32,
    color: Hsla,
    variant: Variant,
    disabled: bool,
}

impl SettingsActionStyle {
    fn outline(width: f32, color: Hsla, disabled: bool) -> Self {
        Self {
            width,
            color,
            variant: Variant::Outline,
            disabled,
        }
    }

    fn subtle(width: f32, color: Hsla, disabled: bool) -> Self {
        Self {
            width,
            color,
            variant: Variant::Subtle,
            disabled,
        }
    }

    fn filled(width: f32, color: Hsla, disabled: bool) -> Self {
        Self {
            width,
            color,
            variant: Variant::Filled,
            disabled,
        }
    }
}

/// Keep settings actions on the native guise button for keyboard/focus
/// behavior, while giving every action the same icon and label columns.
fn settings_action_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: IconName,
    style: SettingsActionStyle,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let label: SharedString = label.into();
    let content = div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_none()
                .w(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon)),
        )
        .child(
            div()
                .flex_none()
                .w(px(SETTINGS_LABEL_WIDTH))
                .whitespace_nowrap()
                .child(label),
        );
    div().flex_none().w(px(style.width)).child(
        Button::new(id, "")
            .variant(style.variant)
            .color(style.color)
            .size(Size::Xs)
            .full_width(true)
            .left_section(content)
            .disabled(style.disabled)
            .on_click(on_click),
    )
}

impl XWikiApp {
    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> Div {
        let theme = theme(cx).clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.body().hsla())
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .flex()
                    .items_start()
                    .justify_center()
                    .child(
                        div()
                            .w_full()
                            .flex_none()
                            .max_w(px(920.0))
                            .p_6()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(self.render_settings_header(cx))
                            .child(self.render_settings_service(cx))
                            .child(self.render_settings_token(cx))
                            .child(self.render_settings_user(cx))
                            .child(self.render_settings_session(cx))
                            .child(self.render_settings_ota(cx)),
                    ),
            )
    }

    fn render_settings_header(&self, cx: &mut Context<Self>) -> Div {
        let theme = theme(cx).clone();
        div()
            .flex()
            .items_center()
            .gap_3()
            .pb_4()
            .border_b_1()
            .border_color(theme.border().hsla())
            .child(
                div()
                    .id("settings-back-tip")
                    .tooltip(guise::tooltip("返回工作台 (Esc)"))
                    .child(
                        Button::new("settings-back", "返回工作台")
                            .variant(Variant::Subtle)
                            .size(Size::Xs)
                            .left_section(Icon::new(IconName::ArrowLeft))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.screen = Screen::Workspace;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .font_family(tokens::FONT_DISPLAY)
                    .text_3xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text().hsla())
                    .child("设置"),
            )
    }

    fn render_settings_service(&self, cx: &mut Context<Self>) -> Div {
        let theme = theme(cx).clone();
        let test_busy = self.settings_loading;
        let test_failed = matches!(self.settings_test.as_ref(), Some((false, _)));
        let test_detail: AnyElement = if let Some(detail) = &self.settings_test_detail {
            div()
                .font_family(tokens::FONT_MONO)
                .text_xs()
                .text_color(theme.dimmed().hsla())
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
                .border_color(if *ok {
                    theme.success().hsla()
                } else {
                    theme.danger().hsla()
                })
                .bg(if *ok {
                    theme.success().alpha(0.1)
                } else {
                    theme.danger().alpha(0.1)
                })
                .child(Icon::new(if *ok {
                    IconName::CircleCheck
                } else {
                    IconName::CircleX
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(if *ok {
                                    theme.success().hsla()
                                } else {
                                    theme.danger().hsla()
                                })
                                .child(message.clone()),
                        )
                        .child(test_detail),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(mono_label("服务地址").text_color(theme.dimmed().hsla()))
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
                            .child(div().w_full().child(self.settings_server_input.clone())),
                    )
                    .child(
                        div()
                            .id("settings-test-tip")
                            .tooltip(guise::tooltip("检查服务器是否可达"))
                            .child(settings_action_button(
                                "settings-test",
                                if test_busy {
                                    "测试中…"
                                } else {
                                    "测试连接"
                                },
                                IconName::Network,
                                SettingsActionStyle::outline(
                                    SETTINGS_ACTION_WIDTH,
                                    theme.primary().hsla(),
                                    test_busy,
                                ),
                                cx.listener(|this, _, _, cx| this.test_connection(cx)),
                            )),
                    )
                    .child(settings_action_button(
                        "settings-save",
                        "保存",
                        IconName::Check,
                        SettingsActionStyle::filled(
                            SETTINGS_ACTION_WIDTH,
                            theme.primary().hsla(),
                            test_busy || test_failed,
                        ),
                        cx.listener(|this, _, _, cx| this.save_server_settings(cx)),
                    )),
            )
            .child(status)
    }

    fn render_settings_ota(&self, cx: &mut Context<Self>) -> Div {
        let theme = theme(cx).clone();
        let ota_busy = self.settings_ota_loading;
        let current_version = env!("CARGO_PKG_VERSION");
        let status: AnyElement = if let Some((ok, message)) = &self.settings_ota_status {
            div()
                .text_xs()
                .text_color(if *ok {
                    theme.success().hsla()
                } else {
                    theme.danger().hsla()
                })
                .child(message.clone())
                .into_any_element()
        } else {
            div().into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .pt_4()
            .px_4()
            .border_t_1()
            .border_color(theme.border().hsla())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(mono_label("OTA 更新").text_color(theme.dimmed().hsla()))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.text().hsla())
                                    .child(format!("v{current_version}")),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-ota-tip")
                            .tooltip(guise::tooltip("检查 GitHub Releases 最新版本"))
                            .child(settings_action_button(
                                "settings-ota-check",
                                if ota_busy {
                                    "检查中…"
                                } else {
                                    "检查更新"
                                },
                                IconName::Redo2,
                                SettingsActionStyle::outline(
                                    SETTINGS_ACTION_WIDTH,
                                    theme.primary().hsla(),
                                    ota_busy,
                                ),
                                cx.listener(|this, _, _, cx| this.check_ota_update(cx)),
                            )),
                    ),
            )
            .child(status)
    }

    fn render_settings_token(&self, cx: &mut Context<Self>) -> Div {
        let theme = theme(cx).clone();
        let access_busy = self.settings_access_loading;
        let test_busy = self.settings_loading;
        let tokens_empty = self.settings_tokens.is_empty();

        let token_rows: Vec<AnyElement> = self
            .settings_tokens
            .iter()
            .enumerate()
            .map(|(i, token)| {
                let id = token.id.clone();
                let token_name = token.name.clone();
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border().hsla())
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_sm()
                                    .text_color(theme.text().hsla())
                                    .child(token.name.clone()),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.dimmed().hsla())
                                    .child(format!("{} · {}", token.scope, token.created_at)),
                            ),
                    )
                    .child(settings_action_button(
                        ElementId::named_usize("revoke-token", i),
                        "撤销",
                        IconName::Delete,
                        SettingsActionStyle::outline(
                            SETTINGS_COMPACT_ACTION_WIDTH,
                            theme.danger().hsla(),
                            access_busy,
                        ),
                        cx.listener(move |this, _, window, cx| {
                            this.confirm_revoke_token(window, cx, id.clone(), token_name.clone());
                        }),
                    ))
                    .into_any_element()
            })
            .collect();

        let token_content: AnyElement = if access_busy {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.dimmed().hsla())
                .child("正在加载访问 Token…")
                .into_any_element()
        } else if tokens_empty {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.dimmed().hsla())
                .child("暂无可用访问 Token。")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .children(token_rows)
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
                .border_color(theme.primary().hsla())
                .bg(theme.primary().alpha(0.1))
                .child(
                    div()
                        .text_color(theme.primary().hsla())
                        .child(Icon::new(IconName::CircleCheck)),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            mono_label("新 Token 密钥（只显示一次）")
                                .text_color(theme.primary().hsla()),
                        )
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.text().hsla())
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
                .border_color(theme.danger().hsla())
                .bg(theme.danger().alpha(0.1))
                .child(
                    div()
                        .text_color(theme.danger().hsla())
                        .child(Icon::new(IconName::CircleX)),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(theme.danger().hsla())
                        .child(error.clone()),
                )
                .child(settings_action_button(
                    "retry-access",
                    "重试",
                    IconName::Redo2,
                    SettingsActionStyle::outline(
                        SETTINGS_COMPACT_ACTION_WIDTH,
                        theme.primary().hsla(),
                        access_busy,
                    ),
                    cx.listener(|this, _, _, cx| this.load_settings_access(cx)),
                ))
                .into_any_element()
        } else {
            div().into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(mono_label("Token 管理").text_color(theme.dimmed().hsla()))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(settings_action_button(
                                "create-token",
                                "新建",
                                IconName::Plus,
                                SettingsActionStyle::outline(
                                    SETTINGS_COMPACT_ACTION_WIDTH,
                                    theme.primary().hsla(),
                                    access_busy || test_busy,
                                ),
                                cx.listener(|this, _, window, cx| {
                                    this.open_create_token_dialog(window, cx)
                                }),
                            ))
                            .child(settings_action_button(
                                "refresh-access",
                                if access_busy {
                                    "加载中…"
                                } else {
                                    "刷新"
                                },
                                IconName::Redo2,
                                SettingsActionStyle::outline(
                                    SETTINGS_COMPACT_ACTION_WIDTH,
                                    theme.primary().hsla(),
                                    access_busy || test_busy,
                                ),
                                cx.listener(|this, _, _, cx| this.load_settings_access(cx)),
                            )),
                    ),
            )
            .child(status)
            .child(
                div()
                    .border_t_1()
                    .border_color(theme.border().hsla())
                    .child(token_content),
            )
    }

    fn render_settings_user(&self, cx: &mut Context<Self>) -> Div {
        let theme = theme(cx).clone();
        let access_busy = self.settings_access_loading;
        let test_busy = self.settings_loading;
        let users_empty = self.settings_users.is_empty();

        let user_rows: Vec<AnyElement> = self
            .settings_users
            .iter()
            .enumerate()
            .map(|(i, user)| {
                let id = user.id.clone();
                let user_name = user.username.clone();
                let enabled = !user.disabled;
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border().hsla())
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_sm()
                                    .text_color(theme.text().hsla())
                                    .child(user.username.clone()),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.dimmed().hsla())
                                    .child(if user.is_admin {
                                        "管理员"
                                    } else {
                                        "普通用户"
                                    }),
                            ),
                    )
                    .child(settings_action_button(
                        ElementId::named_usize("toggle-user", i),
                        if enabled { "停用" } else { "启用" },
                        if enabled {
                            IconName::CircleX
                        } else {
                            IconName::CircleCheck
                        },
                        if enabled {
                            SettingsActionStyle::outline(
                                SETTINGS_ACTION_WIDTH,
                                theme.danger().hsla(),
                                access_busy,
                            )
                        } else {
                            SettingsActionStyle::subtle(
                                SETTINGS_ACTION_WIDTH,
                                theme.danger().hsla(),
                                access_busy,
                            )
                        },
                        cx.listener(move |this, _, window, cx| {
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
                        }),
                    ))
                    .into_any_element()
            })
            .collect();

        let user_content: AnyElement = if access_busy {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.dimmed().hsla())
                .child("正在加载用户…")
                .into_any_element()
        } else if users_empty {
            div()
                .py_4()
                .font_family(tokens::FONT_BODY)
                .text_sm()
                .text_color(theme.dimmed().hsla())
                .child("暂无其他用户。")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .children(user_rows)
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(mono_label("用户管理").text_color(theme.dimmed().hsla()))
                    .child(settings_action_button(
                        "create-user",
                        "新建用户",
                        IconName::Plus,
                        SettingsActionStyle::outline(
                            SETTINGS_ACTION_WIDTH,
                            theme.primary().hsla(),
                            access_busy || test_busy,
                        ),
                        cx.listener(|this, _, window, cx| this.open_create_user_dialog(window, cx)),
                    )),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(theme.border().hsla())
                    .child(user_content),
            )
    }

    fn render_settings_session(&self, cx: &mut Context<Self>) -> Div {
        let theme = theme(cx).clone();
        let dark = theme.scheme.is_dark();
        div()
            .flex()
            .flex_col()
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
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(mono_label("当前用户").text_color(theme.dimmed().hsla()))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_sm()
                                    .text_color(theme.text().hsla())
                                    .child(self.username.clone()),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-logout-tip")
                            .tooltip(guise::tooltip("退出登录"))
                            .child(settings_action_button(
                                "settings-logout",
                                "退出登录",
                                IconName::CircleX,
                                SettingsActionStyle::outline(
                                    SETTINGS_ACTION_WIDTH,
                                    theme.danger().hsla(),
                                    self.editing || self.saving,
                                ),
                                cx.listener(|this, _, _, cx| this.logout(cx)),
                            )),
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
                    .border_color(theme.border().hsla())
                    .child(mono_label("界面主题").text_color(theme.dimmed().hsla()))
                    .child(settings_action_button(
                        "settings-theme",
                        if dark { "浅色模式" } else { "深色模式" },
                        if dark { IconName::Sun } else { IconName::Moon },
                        SettingsActionStyle::outline(
                            SETTINGS_ACTION_WIDTH,
                            theme.primary().hsla(),
                            false,
                        ),
                        cx.listener(|this, _, _, cx| this.toggle_theme(cx)),
                    )),
            )
    }
}
