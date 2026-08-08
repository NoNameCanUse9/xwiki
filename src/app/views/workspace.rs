//! Workspace (project grid) (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::StatefulInteractiveElement;
use gpui::*;
use gpui_component::{
    button::*, input::Input, menu::ContextMenuExt, scroll::ScrollableElement as _, *,
};

use crate::app::{ProjectContextAction, ProjectFilter, ProjectRow, Screen, XWikiApp};
use crate::config;
use crate::ui::{mono_label, split_pane, tokens};

impl XWikiApp {
    pub(crate) fn render_project_cards(&self, card_width: f32, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let q = self.filter_input.read(cx).value().to_lowercase();
        let projects: Vec<ProjectRow> = self
            .projects
            .read()
            .unwrap()
            .iter()
            .filter(|p| {
                let status_matches = match self.project_filter {
                    ProjectFilter::All => true,
                    ProjectFilter::Active => !p.archived,
                    ProjectFilter::Archived => p.archived,
                };
                status_matches
                    && (q.is_empty()
                        || p.name.to_lowercase().contains(&q)
                        || p.description.to_lowercase().contains(&q))
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
                    div()
                        .font_family(tokens::FONT_DISPLAY)
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(if q.is_empty() {
                            "还没有项目"
                        } else {
                            "没有匹配的项目"
                        }),
                )
                .child(
                    div()
                        .font_family(tokens::FONT_BODY)
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(if q.is_empty() {
                            "创建第一个项目，开始组织你的文档。"
                        } else {
                            "换一个关键词，或清空搜索条件。"
                        }),
                );
            return if q.is_empty() {
                empty.child(
                    Button::new("empty-new-project")
                        .primary()
                        .rounded(px(tokens::RADIUS))
                        .icon(IconName::Plus)
                        .label("新建项目")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_new_project_dialog(window, cx)
                        })),
                )
            } else {
                empty
            };
        }

        // Keep wrapped lines at their intrinsic height. Without an explicit
        // start alignment GPUI stretches the flex-wrap tracks, which is the
        // source of the detached footer seen in the old screenshot.
        let mut grid = div()
            .flex()
            .flex_wrap()
            .content_start()
            .items_start()
            .gap_3()
            .w_full()
            .max_w(px(tokens::PROJECT_GRID_MAX));
        for p in projects {
            let id = p.id.clone();
            let status_color = if p.archived {
                theme.muted_foreground
            } else {
                theme.accent
            };
            let card = div()
                .id(format!("project-card-{}", p.name))
                .flex_none()
                .w(px(card_width))
                .h(px(tokens::CARD_HEIGHT))
                .p_4()
                .rounded(px(tokens::RADIUS))
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .hover(|s| {
                    s.bg(theme.list_hover).shadow(vec![BoxShadow::new(
                        px(0.0),
                        px(2.0),
                        gpui::rgba(0x1920291a).into(),
                    )
                    .blur_radius(px(8.0))])
                })
                .cursor_pointer()
                .v_flex()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().size_2().rounded_full().bg(status_color))
                                .child(
                                    mono_label(if p.archived { "ARCHIVED" } else { "ACTIVE" })
                                        .text_color(status_color),
                                ),
                        )
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(p.updated.clone()),
                        ),
                )
                .child(
                    crate::ui::display(p.name.clone())
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.0))
                        .font_family(tokens::FONT_BODY)
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(if p.description.is_empty() {
                            "暂无项目描述".to_string()
                        } else {
                            p.description.clone()
                        }),
                )
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
                                .text_color(theme.muted_foreground)
                                .child("PROJECT"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.accent)
                                .child("打开")
                                .child(
                                    Icon::new(IconName::ArrowRight)
                                        .with_size(px(14.0))
                                        .text_color(theme.accent),
                                ),
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

    fn project_filter_button(
        &self,
        id: &'static str,
        label: &'static str,
        filter: ProjectFilter,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let selected = self.project_filter == filter;
        let row = div()
            .id(id)
            .px_3()
            .py_1_5()
            .rounded(px(tokens::RADIUS_SMALL))
            .font_family(tokens::FONT_MONO)
            .text_xs()
            .text_color(if selected {
                theme.accent
            } else {
                theme.muted_foreground
            })
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.project_filter = filter;
                cx.notify();
            }));
        if selected {
            row.bg(theme.list_active).into_any_element()
        } else {
            row.hover(|s| s.bg(theme.list_hover)).into_any_element()
        }
    }

    pub(crate) fn render_workspace(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let app_handle = cx.entity();
        let window_width = f32::from(window.bounds().size.width);
        let compact_rail = window_width < 1160.0;
        let rail_width = if compact_rail {
            tokens::PROJECTS_RAIL_COMPACT
        } else {
            self.layout
                .projects_rail
                .clamp(tokens::PROJECTS_RAIL_MIN, tokens::PROJECTS_RAIL_MAX)
        };
        let content_width = (window_width - rail_width - tokens::SPLITTER_HIT - 64.0).max(1.0);
        let columns = if content_width >= 1450.0 {
            4
        } else if content_width >= 900.0 {
            3
        } else if content_width >= 640.0 {
            2
        } else {
            1
        };
        let card_width = ((content_width - 12.0 * (columns as f32 - 1.0)) / columns as f32)
            .clamp(tokens::CARD_MIN_WIDTH, tokens::CARD_MAX_WIDTH);
        let (project_count, active_count) = {
            let projects = self.projects.read().unwrap();
            (
                projects.len(),
                projects.iter().filter(|project| !project.archived).count(),
            )
        };
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
                                    .child(format!("{} K", tokens::MOD_KEY)),
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
                                    .icon(if cx.theme().is_dark() {
                                        IconName::Sun
                                    } else {
                                        IconName::Moon
                                    })
                                    .label(if cx.theme().is_dark() {
                                        "浅色"
                                    } else {
                                        "深色"
                                    })
                                    .tooltip(format!("切换主题 ({} Shift T)", tokens::MOD_KEY))
                                    .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
                            )
                            .child(
                                Button::new("settings")
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Settings)
                                    .label("设置")
                                    .tooltip("打开设置")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.screen = Screen::Settings;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("logout")
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::ArrowLeft)
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
                    rail_width,
                    if compact_rail {
                        tokens::PROJECTS_RAIL_COMPACT
                    } else {
                        tokens::PROJECTS_RAIL_MIN
                    },
                    if compact_rail {
                        tokens::PROJECTS_RAIL_COMPACT
                    } else {
                        tokens::PROJECTS_RAIL_MAX
                    },
                    if compact_rail {
                        tokens::PROJECTS_RAIL_COMPACT
                    } else {
                        tokens::PROJECTS_RAIL
                    },
                    window,
                    theme.border,
                    theme.list_hover,
                    div()
                        .h_full()
                        .flex()
                        .flex_col()
                        .bg(theme.sidebar)
                        .child(if compact_rail {
                            div()
                                .h(px(52.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .border_b_1()
                                .border_color(theme.border)
                                .child(mono_label("P").text_color(theme.accent))
                        } else {
                            div()
                                .h(px(52.0))
                                .px_4()
                                .flex()
                                .items_center()
                                .justify_between()
                                .border_b_1()
                                .border_color(theme.border)
                                .child(mono_label("PROJECTS").text_color(theme.muted_foreground))
                                .child(
                                    mono_label(format!("{:02}", project_count))
                                        .text_color(theme.muted_foreground),
                                )
                        })
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
                                        .flex_none()
                                        .items_center()
                                        .gap_2_5()
                                        .px_3()
                                        .py_2()
                                        .cursor_pointer()
                                        .child(if compact_rail {
                                            Icon::new(IconName::Folder)
                                                .with_size(px(18.0))
                                                .text_color(if is_selected {
                                                    theme2.accent
                                                } else {
                                                    theme2.muted_foreground
                                                })
                                                .into_any_element()
                                        } else {
                                            div()
                                                .w(px(2.0))
                                                .self_stretch()
                                                .bg(if is_selected {
                                                    theme2.accent
                                                } else {
                                                    gpui::transparent_black()
                                                })
                                                .into_any_element()
                                        })
                                        .child(if compact_rail {
                                            div()
                                        } else {
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
                                                .child(p.name.clone())
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.open_project(&id, cx)
                                        }));
                                    let row = if compact_rail {
                                        row.justify_center().gap_0()
                                    } else {
                                        row
                                    };
                                    let row = if is_selected {
                                        row.bg(theme2.list_active)
                                    } else {
                                        row.hover(|s| s.bg(theme2.list_hover))
                                    };
                                    row.into_any_element()
                                })
                                .collect();
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .flex_col()
                                .overflow_y_scrollbar()
                                .children(items)
                        }),
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .h_full()
                        .flex()
                        .flex_col()
                        .gap_5()
                        .p_6()
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .justify_between()
                                .gap_6()
                                .child(
                                    div()
                                        .v_flex()
                                        .gap_1()
                                        .child(self.eyebrow("WORKSPACE / PROJECTS", cx))
                                        .child(
                                            crate::ui::display("项目工作台")
                                                .text_2xl()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.foreground),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .font_family(tokens::FONT_BODY)
                                                .child("选择一个项目，开始阅读、编辑或查看历史。"),
                                        ),
                                )
                                .child(
                                    div()
                                        .v_flex()
                                        .items_end()
                                        .gap_1()
                                        .child(
                                            mono_label(format!(
                                                "{} 个项目 · {} 个活跃",
                                                project_count, active_count
                                            ))
                                            .text_color(theme.muted_foreground),
                                        )
                                        .child(
                                            mono_label("DESKTOP WORKSPACE")
                                                .text_color(theme.accent),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .px_1()
                                        .py_1()
                                        .rounded(px(tokens::RADIUS))
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(self.project_filter_button(
                                            "filter-all",
                                            "全部",
                                            ProjectFilter::All,
                                            cx,
                                        ))
                                        .child(self.project_filter_button(
                                            "filter-active",
                                            "活跃",
                                            ProjectFilter::Active,
                                            cx,
                                        ))
                                        .child(self.project_filter_button(
                                            "filter-archived",
                                            "已归档",
                                            ProjectFilter::Archived,
                                            cx,
                                        )),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .max_w(px(420.0))
                                        .child(Input::new(&self.filter_input).w_full()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_3()
                                        .py_2()
                                        .rounded(px(tokens::RADIUS))
                                        .border_1()
                                        .border_color(theme.border)
                                        .text_xs()
                                        .font_family(tokens::FONT_MONO)
                                        .text_color(theme.muted_foreground)
                                        .child(format!("{} P", tokens::MOD_KEY))
                                        .child("快速打开"),
                                )
                                .child(
                                    Button::new("new-project")
                                        .primary()
                                        .rounded(px(tokens::RADIUS))
                                        .icon(IconName::Plus)
                                        .label("新建项目")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_new_project_dialog(window, cx)
                                        })),
                                ),
                        )
                        .child(if self.loading {
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .overflow_y_scrollbar()
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .content_start()
                                        .items_start()
                                        .gap_3()
                                        .children((0..3).map(|i| {
                                            div()
                                                .id(format!("skeleton-card-{i}"))
                                                .flex_none()
                                                .w(px(card_width))
                                                .h(px(tokens::CARD_HEIGHT))
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
                                        })),
                                )
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
                                        .icon(IconName::Redo2)
                                        .label("重试")
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.load_projects(cx)),
                                        ),
                                )
                                .into_any_element()
                        } else {
                            div()
                                .flex_1()
                                .min_h(px(0.0))
                                .overflow_y_scrollbar()
                                .child(self.render_project_cards(card_width, cx))
                                .into_any_element()
                        }),
                    move |w, _window, cx| {
                        if !compact_rail {
                            app_handle.update(cx, |app, cx| {
                                app.layout.projects_rail = w;
                                config::save_layout(&app.layout);
                                cx.notify();
                            });
                        }
                    },
                ),
            )
            .child(self.render_status_bar(cx))
    }
}
