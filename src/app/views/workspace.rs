//! Workspace (project grid) (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::StatefulInteractiveElement;
use gpui::*;
use gpui_component::{
    button::*,
    input::Input,
    menu::{ContextMenuExt, DropdownMenu, PopupMenu},
    scroll::ScrollableElement as _,
    *,
};

use crate::app::{
    ProjectArchiveAction, ProjectContextAction, ProjectDeleteAction, ProjectFilter,
    ProjectRenameAction, ProjectRow, XWikiApp,
};
use crate::config;
use crate::ui::{mono_label, split_pane, tokens};

impl XWikiApp {
    pub(crate) fn render_project_cards(&self, card_width: f32, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let q = self.filter_input.read(cx).value().to_lowercase();
        let projects: Vec<ProjectRow> = self
            .projects
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
            let (empty_title, empty_description) = if !q.is_empty() {
                ("没有匹配的项目", "没有项目符合当前搜索条件。")
            } else {
                match self.project_filter {
                    ProjectFilter::All => ("还没有项目", "创建第一个项目，开始组织你的文档。"),
                    ProjectFilter::Active => {
                        ("还没有活跃项目", "新建项目或取消归档后，项目会显示在这里。")
                    }
                    ProjectFilter::Archived => ("还没有归档项目", "归档项目会显示在这里。"),
                }
            };
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
                        .child(empty_title),
                )
                .child(
                    div()
                        .font_family(tokens::FONT_BODY)
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(empty_description),
                );
            return match (q.is_empty(), self.project_filter) {
                (true, ProjectFilter::Archived) => empty,
                (true, _) => empty.child(
                    Button::new("empty-new-project")
                        .primary()
                        .rounded(px(tokens::RADIUS))
                        .icon(IconName::Plus)
                        .label(if self.project_action.is_some() {
                            "创建中…"
                        } else {
                            "新建项目"
                        })
                        .loading(self.project_action.is_some())
                        .disabled(self.project_action.is_some())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_new_project_dialog(window, cx)
                        })),
                ),
                (false, _) => empty.child(crate::ui::clear_search_button(
                    "empty-clear-project-search",
                    self.filter_input.clone(),
                    cx.entity().entity_id(),
                )),
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
            let name = p.name.clone();
            let archived = p.archived;
            let status_color = if archived {
                theme.muted_foreground
            } else {
                theme.accent
            };
            let card = div()
                .id(format!("project-card-{id}"))
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
                        theme.foreground.opacity(0.1),
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
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(tokens::truncate(&p.updated, 16)),
                                )
                                .child({
                                    let menu_id = id.clone();
                                    let menu_name = p.name.clone();
                                    let menu_archived = archived;
                                    Button::new(format!("project-menu-{}", id))
                                        .ghost()
                                        .compact()
                                        .icon(IconName::EllipsisVertical)
                                        .tooltip("项目操作")
                                        .dropdown_menu(move |menu, _window, _cx| {
                                            project_menu(
                                                menu,
                                                menu_id.clone(),
                                                menu_name.clone(),
                                                menu_archived,
                                            )
                                        })
                                }),
                        ),
                )
                .child(
                    crate::ui::display(tokens::truncate(&p.name, 42))
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
                            tokens::truncate(&p.description, 120)
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
                project_menu(menu, id.clone(), name.clone(), archived)
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
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .px_3()
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
        let window_width = f32::from(window.bounds().size.width).max(1.0);
        let rail_width = self
            .layout
            .projects_rail
            .clamp(tokens::PROJECTS_RAIL_MIN, tokens::PROJECTS_RAIL_MAX);
        let content_width = (window_width - rail_width - tokens::SPLITTER_HIT).max(1.0);
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
            let projects = &self.projects;
            (
                projects.len(),
                projects.iter().filter(|project| !project.archived).count(),
            )
        };
        div().flex().size_full().child(
            // Project panel (resizable) + project content inside the shared shell.
            split_pane::horizontal(
                "projects-rail-split",
                rail_width,
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
                            ),
                    )
                    .child({
                        let projects = &self.projects;
                        let items: Vec<AnyElement> = projects
                            .iter()
                            .map(|p| {
                                let id = p.id.clone();
                                let theme2 = theme.clone();
                                let is_selected =
                                    self.selected_project.as_deref() == Some(p.id.as_str());
                                let row = div()
                                    .id(format!("nav-{}", p.id))
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .px_4()
                                    .py_2()
                                    .cursor_pointer()
                                    .gap_2()
                                    .child(
                                        Icon::new(IconName::Folder).with_size(px(16.0)).text_color(
                                            if is_selected {
                                                theme2.accent
                                            } else {
                                                theme2.muted_foreground
                                            },
                                        ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .overflow_x_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(if is_selected {
                                                theme2.foreground
                                            } else if p.archived {
                                                theme2.muted_foreground
                                            } else {
                                                theme2.foreground
                                            })
                                            .child(tokens::truncate(p.name.trim(), 40)),
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
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .flex_col()
                            .overflow_y_scrollbar()
                            .children(items)
                    })
                    .child(
                        div()
                            .flex_none()
                            .border_t_1()
                            .border_color(theme.border)
                            .px_3()
                            .py_3()
                            .v_flex()
                            .gap_2()
                            .child(mono_label("DEVELOPER TOOLS").text_color(theme.muted_foreground))
                            .child(
                                Button::new("sidebar-api-reference")
                                    .secondary()
                                    .outline()
                                    .compact()
                                    .w_full()
                                    .icon(IconName::File)
                                    .label("API Reference")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.open_api_reference(cx)),
                                    ),
                            )
                            .child(
                                Button::new("sidebar-audit")
                                    .secondary()
                                    .outline()
                                    .compact()
                                    .w_full()
                                    .icon(IconName::Inbox)
                                    .label("Audit Log")
                                    .on_click(cx.listener(|this, _, _, cx| this.open_audit(cx))),
                            ),
                    ),
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
                                        mono_label("DESKTOP WORKSPACE").text_color(theme.accent),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_end()
                            .gap_3()
                            .child(
                                div()
                                    .h(px(34.0))
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .gap_1()
                                    .px_1()
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
                                    .min_w(px(180.0))
                                    .max_w(px(420.0))
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        mono_label("搜索项目").text_color(theme.muted_foreground),
                                    )
                                    .child(Input::new(&self.filter_input).w_full()),
                            )
                            .child(
                                Button::new("workspace-quick-open")
                                    .secondary()
                                    .outline()
                                    .compact()
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Search)
                                    .label("快速打开")
                                    .tooltip(format!("快速打开 ({} P)", tokens::MOD_KEY))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_quick_open(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("import-project")
                                    .secondary()
                                    .outline()
                                    .compact()
                                    .rounded(px(tokens::RADIUS))
                                    .label("导入项目")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_import_dialog(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("new-project")
                                    .primary()
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Plus)
                                    .label(if self.project_action.is_some() {
                                        "创建中…"
                                    } else {
                                        "新建项目"
                                    })
                                    .loading(self.project_action.is_some())
                                    .disabled(self.project_action.is_some())
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
                                    .on_click(cx.listener(|this, _, _, cx| this.load_projects(cx))),
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
                    app_handle.update(cx, |app, cx| {
                        app.layout.projects_rail = w;
                        config::save_layout(&app.layout);
                        cx.notify();
                    });
                },
            ),
        )
    }
}

/// The four project actions shared by the card dropdown and its context
/// menu — kept in one place so the two menus can't drift apart.
fn project_menu(menu: PopupMenu, id: String, name: String, archived: bool) -> PopupMenu {
    let mut m = menu.menu(
        "打开项目",
        Box::new(ProjectContextAction {
            project_id: id.clone(),
        }),
    );
    m = m.menu(
        if archived {
            "取消归档"
        } else {
            "归档项目"
        },
        Box::new(ProjectArchiveAction {
            project_id: id.clone(),
            archived: !archived,
        }),
    );
    m = m.menu(
        "重命名",
        Box::new(ProjectRenameAction {
            project_id: id.clone(),
            current_name: name.clone(),
        }),
    );
    m.menu(
        "删除项目",
        Box::new(ProjectDeleteAction {
            project_id: id.clone(),
            project_name: name.clone(),
        }),
    )
}
