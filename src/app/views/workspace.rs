//! Workspace (project grid) (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::*;
use gpui_component::{button::*, input::Input, menu::ContextMenuExt, *};

use crate::app::{ProjectContextAction, ProjectRow, XWikiApp};
use crate::config;
use crate::ui::{mono_label, split_pane, tokens};

impl XWikiApp {
    pub(crate) fn render_project_cards(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let q = self.filter_input.read(cx).value().to_lowercase();
        let projects: Vec<ProjectRow> = self
            .projects
            .read()
            .unwrap()
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.name.to_lowercase().contains(&q)
                    || p.description.to_lowercase().contains(&q)
            })
            .cloned()
            .collect();
        if projects.is_empty() {
            let empty = div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    mono_label(if q.is_empty() {
                        "还没有项目"
                    } else {
                        "没有匹配的项目"
                    })
                    .text_color(theme.muted_foreground),
                );
            return if q.is_empty() {
                empty.child(
                    Button::new("empty-new-project")
                        .primary()
                        .rounded(px(tokens::RADIUS))
                        .label("新建项目")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_new_project_dialog(window, cx)
                        })),
                )
            } else {
                empty
            };
        }
        let mut grid = div().flex().flex_wrap().gap_3().w_full();
        for p in projects {
            let id = p.id.clone();
            let card = div()
                .id(format!("project-card-{}", p.name))
                .w(px(tokens::CARD_WIDTH))
                .p_4()
                .rounded(px(tokens::RADIUS))
                .border_1()
                .border_color(theme.border)
                .hover(|s| {
                    // Web "hover lift": surface tint + a soft lift shadow.
                    s.bg(theme.list_hover).shadow(vec![BoxShadow::new(
                        px(0.0),
                        px(2.0),
                        gpui::rgba(0x1920291a).into(),
                    )
                    .blur_radius(px(8.0))])
                })
                .cursor_pointer()
                .v_flex()
                .gap_2_5()
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap_3()
                        .child(
                            crate::ui::display(p.name.clone())
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground),
                        )
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(p.updated.clone()),
                        ),
                )
                .child(div().text_sm().text_color(theme.muted_foreground).child(
                    if p.description.is_empty() {
                        "—".to_string()
                    } else {
                        p.description.clone()
                    },
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_t_1()
                        .border_color(theme.border)
                        .pt_3()
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(if p.archived {
                                    theme.muted_foreground
                                } else {
                                    theme.accent
                                })
                                .child(if p.archived { "ARCHIVED" } else { "ACTIVE" }),
                        )
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.accent)
                                .child("打开 →"),
                        ),
                )
                .on_click({
                    let click_id = id.clone();
                    cx.listener(move |this, _, _, cx| this.open_project(&click_id, cx))
                });
            let card = card.context_menu(move |menu, _window, _cx| {
                menu.menu(
                    "打开项目",
                    Box::new(ProjectContextAction {
                        project_id: id.clone(),
                    }),
                )
            });
            grid = grid.child(card);
        }
        grid
    }

    pub(crate) fn eyebrow(&self, label: &'static str, cx: &Context<Self>) -> Div {
        div()
            .font_family(tokens::FONT_MONO)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(label)
    }

    pub(crate) fn render_workspace(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let app_handle = cx.entity();
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                // Top bar: flush, hairline bottom border, mono labels.
                div()
                    .h(px(tokens::TOOLBAR_H))
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(self.eyebrow("AgentDocs", cx))
                            .child(div().w(px(1.0)).h(px(16.0)).bg(theme.border))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(
                                        self.projects
                                            .read()
                                            .unwrap()
                                            .iter()
                                            .find(|p| Some(&p.id) == self.selected_project.as_ref())
                                            .map(|p| p.name.clone())
                                            .unwrap_or_else(|| "workspace".into()),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                // Bordered ⌘K affordance — the command palette
                                // wires in with the desktop feature set.
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .px_2()
                                    .py_1()
                                    .rounded(px(tokens::RADIUS))
                                    .border_1()
                                    .border_color(theme.border)
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("⌘K"),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(self.username.clone()),
                            )
                            .child(if let Some(v) = &self.meta_version {
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("v{v}"))
                            } else {
                                div()
                            })
                            .child(
                                Button::new("toggle-theme")
                                    .rounded(px(tokens::RADIUS))
                                    .label(if cx.theme().is_dark() {
                                        "浅色"
                                    } else {
                                        "深色"
                                    })
                                    .tooltip("切换主题 (⌘⇧T)")
                                    .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
                            )
                            .child(
                                Button::new("logout")
                                    .rounded(px(tokens::RADIUS))
                                    .label("退出")
                                    .tooltip("退出登录")
                                    .on_click(cx.listener(|this, _, _, cx| this.logout(cx))),
                            ),
                    ),
            )
            .child(
                // Project rail (resizable) + project content.
                split_pane::horizontal(
                    "projects-rail-split",
                    self.layout.projects_rail,
                    tokens::PROJECTS_RAIL_MIN,
                    tokens::PROJECTS_RAIL_MAX,
                    tokens::PROJECTS_RAIL,
                    window,
                    theme.border,
                    theme.list_hover,
                    div()
                        .h_full()
                        .flex()
                        .flex_col()
                        .bg(theme.sidebar)
                        .child(
                            div()
                                .px_3()
                                .py_3()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("PROJECTS"),
                        )
                        .child({
                            let projects = self.projects.read().unwrap();
                            let items: Vec<AnyElement> = projects
                                .iter()
                                .map(|p| {
                                    let id = p.id.clone();
                                    let theme2 = theme.clone();
                                    let is_selected =
                                        self.selected_project.as_deref() == Some(p.id.as_str());
                                    let row = div()
                                        .id(format!("nav-{}", p.name))
                                        .flex()
                                        .items_center()
                                        .gap_2_5()
                                        .px_3()
                                        .py_1_5()
                                        .cursor_pointer()
                                        .child(
                                            // Selection bar: cobalt signal.
                                            div().w(px(2.0)).self_stretch().bg(if is_selected {
                                                theme2.accent
                                            } else {
                                                gpui::transparent_black()
                                            }),
                                        )
                                        .child(
                                            div()
                                                .font_family(tokens::FONT_MONO)
                                                .text_xs()
                                                .text_color(if is_selected {
                                                    theme2.foreground
                                                } else if p.archived {
                                                    theme2.muted_foreground
                                                } else {
                                                    theme2.foreground
                                                })
                                                .child(p.name.clone()),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.open_project(&id, cx)
                                        }));
                                    let row = if is_selected {
                                        row.bg(theme2.list_active)
                                    } else {
                                        row.hover(|s| s.bg(theme2.list_hover))
                                    };
                                    row.into_any_element()
                                })
                                .collect();
                            div().children(items)
                        }),
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .h_full()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .w(px(280.0))
                                        .child(Input::new(&self.filter_input).w_full()),
                                )
                                .child(
                                    Button::new("new-project")
                                        .primary()
                                        .rounded(px(tokens::RADIUS))
                                        .label("新建项目")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_new_project_dialog(window, cx)
                                        })),
                                ),
                        )
                        .child(if self.loading {
                            // Plan §4: skeleton cards while projects load.
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_3()
                                .children((0..3).map(|i| {
                                    div()
                                        .id(format!("skeleton-card-{i}"))
                                        .w(px(tokens::CARD_WIDTH))
                                        .h(px(120.0))
                                        .p_4()
                                        .rounded(px(tokens::RADIUS))
                                        .border_1()
                                        .border_color(theme.border)
                                        .v_flex()
                                        .gap_3()
                                        .child(
                                            div()
                                                .w(px(180.0))
                                                .h(px(16.0))
                                                .rounded(px(tokens::RADIUS_SMALL))
                                                .bg(theme.skeleton),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .h(px(12.0))
                                                .rounded(px(tokens::RADIUS_SMALL))
                                                .bg(theme.skeleton),
                                        )
                                        .child(
                                            div()
                                                .w(px(120.0))
                                                .h(px(12.0))
                                                .rounded(px(tokens::RADIUS_SMALL))
                                                .bg(theme.skeleton),
                                        )
                                }))
                                .into_any_element()
                        } else if let Some(err) = &self.projects_error {
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap_4()
                                .child(mono_label("加载失败").text_color(theme.danger))
                                .child(
                                    div()
                                        .px_4()
                                        .text_center()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child(err.clone()),
                                )
                                .child(
                                    Button::new("retry-projects")
                                        .rounded(px(tokens::RADIUS))
                                        .label("重试")
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.load_projects(cx)),
                                        ),
                                )
                                .into_any_element()
                        } else {
                            self.render_project_cards(cx).into_any_element()
                        }),
                    move |w, _window, cx| {
                        app_handle.update(cx, |app, cx| {
                            app.layout.projects_rail = w;
                            config::save_layout(&app.layout);
                            cx.notify();
                        });
                    },
                ),
            )
            .child(self.render_status_bar(cx))
    }
}
