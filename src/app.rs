use std::sync::{Arc, RwLock};

use gpui::*;
use gpui_component::{
    button::*,
    dialog::DialogContent,
    input::{Input, InputEvent, InputState},
    table::{Column, DataTable, TableDelegate, TableState},
    *,
};

/// Application shell: login screen and the project workspace.
/// Theme discipline (Hallmark · Cobalt): cool engineered paper, hairlines
/// over shadows, exactly one electric-blue signal (primary button, focus,
/// hover underlines), mono UPPERCASE labels, 6px radii.
pub struct XWikiApp {
    screen: Screen,
    server_input: Entity<InputState>,
    user_input: Entity<InputState>,
    password_input: Entity<InputState>,
    projects: Arc<RwLock<Vec<ProjectRow>>>,
    filter_input: Entity<InputState>,
    table: Entity<TableState<ProjectsTable>>,
    /// Keep input subscriptions alive with the app entity.
    _subscriptions: Vec<Subscription>,
}

enum Screen {
    Login,
    Workspace,
}

#[derive(Clone)]
struct ProjectRow {
    name: String,
    description: String,
    updated: String,
    archived: bool,
}

/// Table delegate over the shared project list, with client-side filtering.
struct ProjectsTable {
    projects: Arc<RwLock<Vec<ProjectRow>>>,
    filter: String,
}

impl ProjectsTable {
    fn new(projects: Arc<RwLock<Vec<ProjectRow>>>) -> Self {
        Self { projects, filter: String::new() }
    }

    fn visible(&self) -> Vec<ProjectRow> {
        let q = self.filter.to_lowercase();
        self.projects
            .read()
            .unwrap()
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.name.to_lowercase().contains(&q)
                    || p.description.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }
}

impl TableDelegate for ProjectsTable {
    fn columns_count(&self, _: &App) -> usize {
        4
    }

    fn rows_count(&self, _: &App) -> usize {
        self.visible().len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("name", "项目").width(px(220.0)),
            1 => Column::new("description", "描述").width(px(300.0)),
            2 => Column::new("updated", "更新").width(px(120.0)),
            _ => Column::new("status", "状态").width(px(110.0)),
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let rows = self.visible();
        let Some(p) = rows.get(row_ix) else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let cell = match col_ix {
            0 => div()
                .font_family("JetBrains Mono")
                .text_sm()
                .text_color(theme.foreground)
                .child(p.name.clone()),
            1 => div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(p.description.clone()),
            2 => div()
                .font_family("JetBrains Mono")
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(p.updated.clone()),
            _ => div()
                .font_family("JetBrains Mono")
                .text_xs()
                .text_color(if p.archived {
                    theme.muted_foreground
                } else {
                    theme.foreground
                })
                .child(if p.archived { "ARCHIVED" } else { "ACTIVE" }),
        };
        cell.into_any_element()
    }
}

impl XWikiApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let server_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("http://127.0.0.1:9090")
        });
        let user_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("用户名"));
        let password_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("密码").masked(true));

        // ponytail: seed rows stand in for GET /api/v1/projects until the
        // api layer lands.
        let projects = Arc::new(RwLock::new(vec![
            ProjectRow {
                name: "docs-site".into(),
                description: "产品与平台文档".into(),
                updated: "2h ago".into(),
                archived: false,
            },
            ProjectRow {
                name: "handbook".into(),
                description: "团队手册与流程".into(),
                updated: "1d ago".into(),
                archived: false,
            },
            ProjectRow {
                name: "api-reference".into(),
                description: "API 参考与集成指南".into(),
                updated: "3d ago".into(),
                archived: false,
            },
            ProjectRow {
                name: "legacy-wiki".into(),
                description: "旧版迁移中".into(),
                updated: "30d ago".into(),
                archived: true,
            },
        ]));

        let filter_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索项目…"));

        let table =
            cx.new(|cx| TableState::new(ProjectsTable::new(projects.clone()), window, cx));

        let mut subs = Vec::new();
        for state in [&server_input, &user_input, &password_input] {
            subs.push(cx.subscribe_in(state, window, |_, _, _: &InputEvent, _, cx| {
                cx.notify()
            }));
        }
        // Filter keystrokes re-query the table.
        {
            let table = table.clone();
            let filter = filter_input.clone();
            subs.push(cx.subscribe_in(
                &filter_input,
                window,
                move |_, _, _: &InputEvent, _, cx| {
                    let q = filter.read(cx).value().to_string();
                    table.update(cx, |state, cx| {
                        state.delegate_mut().filter = q;
                        state.refresh(cx);
                    });
                },
            ));
        }

        Self {
            screen: Screen::Login,
            server_input,
            user_input,
            password_input,
            projects,
            filter_input,
            table,
            _subscriptions: subs,
        }
    }

    fn login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // ponytail: UI skeleton only — wire the real /api/v1/auth/login call
        // when the api layer lands.
        self.screen = Screen::Workspace;
        cx.notify();
    }

    fn logout(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Login;
        cx.notify();
    }

    fn open_new_project_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let projects = self.projects.clone();
        let table = self.table.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let name_state =
                cx.new(|cx| InputState::new(window, cx).placeholder("docs-site"));
            let desc_state = cx
                .new(|cx| InputState::new(window, cx).placeholder("项目描述（可选）"));

            let content_name = name_state.clone();
            let content_desc = desc_state.clone();
            let content_builder =
                move |content: DialogContent, _: &mut Window, cx: &mut App| {
                let theme = cx.theme();
                content
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .w_full()
                            .child(
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("名称"),
                            )
                            .child(Input::new(&content_name).w_full())
                            .child(
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("描述"),
                            )
                            .child(Input::new(&content_desc).w_full()),
                    )
            };

            let cancel = Button::new("cancel-project")
                .rounded(px(6.0))
                .label("取消")
                .on_click(move |_, window, cx| window.close_dialog(cx));

            let create_projects = projects.clone();
            let create_table = table.clone();
            let create_name = name_state.clone();
            let create_desc = desc_state.clone();
            let create = Button::new("create-project")
                .primary()
                .rounded(px(6.0))
                .label("创建")
                .on_click(move |_, window, cx| {
                    let name = create_name.read(cx).value().to_string();
                    if name.trim().is_empty() {
                        return;
                    }
                    let desc = create_desc.read(cx).value().to_string();
                    create_projects.write().unwrap().push(ProjectRow {
                        name: name.trim().to_string(),
                        description: desc.trim().to_string(),
                        updated: "just now".into(),
                        archived: false,
                    });
                    create_table.update(cx, |state, cx| state.refresh(cx));
                    window.close_dialog(cx);
                });

            dialog
                .title(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("新建项目"),
                )
                .content(content_builder)
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .w_full()
                        .child(cancel)
                        .child(create),
                )
        });
    }

    fn eyebrow(&self, label: &'static str, cx: &Context<Self>) -> Div {
        div()
            .font_family("JetBrains Mono")
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(label)
    }

    fn render_login(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .w(px(360.0))
                    .p_6()
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(self.eyebrow("AgentDocs", cx))
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("文档工作台"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Git-backed 文档系统 · 团队协作"),
                    )
                    .child(Input::new(&self.server_input).w_full())
                    .child(Input::new(&self.user_input).w_full())
                    .child(Input::new(&self.password_input).w_full())
                    .child(
                        Button::new("login")
                            .primary()
                            .w_full()
                            .rounded(px(6.0))
                            .label("登录")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.login(window, cx)
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .font_family("JetBrains Mono")
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("v0.8.0")
                            .child("/api/v1"),
                    ),
            )
    }

    fn render_workspace(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Top bar: flush, hairline bottom border, mono labels.
                div()
                    .h(px(44.0))
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
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("docs-site"),
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
                                    .rounded(px(6.0))
                                    .border_1()
                                    .border_color(theme.border)
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("⌘K"),
                            )
                            .child(
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("admin"),
                            )
                            .child(
                                Button::new("logout")
                                    .rounded(px(6.0))
                                    .label("退出")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.logout(window, cx)
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .size_full()
                    .child(
                        // Project rail: mono section label, hairline divider.
                        div()
                            .w(px(200.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(theme.border)
                            .bg(theme.sidebar)
                            .child(
                                div()
                                    .px_3()
                                    .py_3()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("PROJECTS"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .px_3()
                                    .py_1_5()
                                    .hover(|s| s.bg(theme.list_hover))
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.foreground)
                                            .child("全部"),
                                    ),
                            ),
                    )
                    .child(
                        // Content: filter row + project table.
                        div()
                            .flex_1()
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
                                            .rounded(px(6.0))
                                            .label("新建项目")
                                            .on_click(cx.listener(
                                                |this, _, window, cx| {
                                                    this.open_new_project_dialog(
                                                        window, cx,
                                                    )
                                                },
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(
                                        DataTable::new(&self.table)
                                            .stripe(true)
                                            .bordered(true),
                                    ),
                            ),
                    ),
            )
    }
}

impl Render for XWikiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.screen {
            Screen::Login => self.render_login(cx),
            Screen::Workspace => self.render_workspace(cx),
        }
    }
}
