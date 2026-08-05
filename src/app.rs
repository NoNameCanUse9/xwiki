use std::sync::{Arc, RwLock};

use gpui::*;
use gpui::StatefulInteractiveElement;
use gpui_component::{
    button::*,
    dialog::DialogContent,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement as _,
    table::{Column, DataTable, TableDelegate, TableEvent, TableState},
    text::TextView,
    *,
};

use crate::api::{Client, dto};

/// Application shell: login screen and the project workspace.
/// Theme discipline (Hallmark · Cobalt): cool engineered paper, hairlines
/// over shadows, exactly one electric-blue signal (primary button, focus,
/// hover underlines), mono UPPERCASE labels, 6px radii.
pub struct XWikiApp {
    screen: Screen,
    client: Option<Client>,
    username: String,
    login_error: Option<String>,
    loading: bool,
    meta_version: Option<String>,
    server_input: Entity<InputState>,
    user_input: Entity<InputState>,
    password_input: Entity<InputState>,
    projects: Arc<RwLock<Vec<ProjectRow>>>,
    filter_input: Entity<InputState>,
    table: Entity<TableState<ProjectsTable>>,
    // Document workspace state.
    selected_project: Option<String>,
    tree_entries: Vec<dto::TreeEntry>,
    tree_path: String,
    doc_path: Option<String>,
    doc_content: String,
    doc_loading: bool,
    /// Keep input subscriptions alive with the app entity.
    _subscriptions: Vec<Subscription>,
}

enum Screen {
    Login,
    Workspace,
}

#[derive(Clone)]
struct ProjectRow {
    id: String,
    name: String,
    description: String,
    updated: String,
    archived: bool,
}

impl ProjectRow {
    fn from_dto(p: &dto::Project) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            updated: p.updated_at.split('T').next().unwrap_or("").to_string(),
            archived: p.archived,
        }
    }
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

        // ponytail: rows are loaded from GET /api/v1/projects on login; this
        // starts empty.
        let projects = Arc::new(RwLock::new(Vec::new()));

        let filter_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索项目…"));

        let table =
            cx.new(|cx| TableState::new(ProjectsTable::new(projects.clone()), window, cx));

        let mut subs = Vec::new();
        // Double-click a project row to open its document workspace.
        {
            let table = table.clone();
            subs.push(cx.subscribe(&table, |this, _table, ev, cx| {
                if let TableEvent::DoubleClickedRow(ix) = ev {
                    let rows = this.table.read(cx).delegate().visible();
                    if let Some(row) = rows.get(*ix) {
                        this.open_project(&row.id, cx);
                    }
                }
            }));
        }
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
            client: None,
            username: String::new(),
            login_error: None,
            loading: false,
            meta_version: None,
            server_input,
            user_input,
            password_input,
            projects,
            filter_input,
            table,
            selected_project: None,
            tree_entries: Vec::new(),
            tree_path: String::new(),
            doc_path: None,
            doc_content: String::new(),
            doc_loading: false,
            _subscriptions: subs,
        }
    }

    fn open_project(&mut self, project_id: &str, cx: &mut Context<Self>) {
        self.selected_project = Some(project_id.to_string());
        self.tree_path.clear();
        self.doc_path = None;
        self.doc_content.clear();
        self.load_tree("", cx);
    }

    fn load_tree(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some(project) = self.selected_project.clone() else {
            return;
        };
        self.tree_path = path.to_string();
        self.doc_path = None;
        self.doc_content.clear();
        self.doc_loading = false;
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            match client.tree(&project, &path).await {
                Ok(entries) => {
                    let _ = this.update(cx, |app, cx| {
                        app.tree_entries = entries;
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.login_error = Some(e.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn open_doc(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some(project) = self.selected_project.clone() else {
            return;
        };
        self.doc_path = Some(path.to_string());
        self.doc_loading = true;
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            match client.page(&project, &path).await {
                Ok(page) => {
                    let _ = this.update(cx, |app, cx| {
                        app.doc_content = page.content;
                        app.doc_loading = false;
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.doc_loading = false;
                        app.login_error = Some(e.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn back_to_projects(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.selected_project = None;
        self.tree_entries.clear();
        self.tree_path.clear();
        self.doc_path = None;
        self.doc_content.clear();
        cx.notify();
    }

    /// Breadcrumb trail for the current tree directory; clicking a crumb
    /// navigates back up.
    fn render_tree(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let mut list = div().v_flex().gap_1().w_full();
        for e in &self.tree_entries {
            let is_dir = e.r#type == "tree";
            let path = e.path.clone();
            let row = div()
                .id(path.clone())
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .hover(|s| s.bg(theme.list_hover))
                .cursor_pointer()
                .child(
                    div()
                        .font_family("JetBrains Mono")
                        .text_xs()
                        .text_color(if is_dir {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(if is_dir {
                            format!("{}/", e.name)
                        } else {
                            e.name.clone()
                        }),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if is_dir {
                        this.load_tree(&path, cx);
                    } else {
                        this.open_doc(&path, cx);
                    }
                }));
            list = list.child(row);
        }
        list
    }

    fn render_doc_view(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .flex()
            .size_full()
            .child(
                // Doc rail: tree navigation + breadcrumb.
                div()
                    .w(px(280.0))
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
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("DOCS"),
                            )
                            .child(
                                Button::new("back-projects")
                                    .rounded(px(6.0))
                                    .label("← 项目")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.back_to_projects(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        // Breadcrumb: root → dir1 → dir2
                        div()
                            .px_3()
                            .pb_2()
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .child(if self.tree_path.is_empty() {
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.foreground)
                                    .child("/")
                            } else {
                                let mut crumbs = div().flex().flex_wrap().gap_1();
                                let mut acc = String::new();
                                for part in self.tree_path.split('/') {
                                    if part.is_empty() {
                                        continue;
                                    }
                                    acc = if acc.is_empty() {
                                        part.to_string()
                                    } else {
                                        format!("{acc}/{part}")
                                    };
                                    let target = acc.clone();
                                    crumbs = crumbs.child(
                                        div()
                                            .id(format!("crumb-{target}"))
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .hover(|s| s.text_color(theme.foreground))
                                            .cursor_pointer()
                                            .child(format!("{part}/"))
                                            .on_click(cx.listener(
                                                move |this, _, _, cx| {
                                                    this.load_tree(&target, cx);
                                                },
                                            )),
                                    );
                                }
                                crumbs
                            }),
                    )
                    .child(div().flex_1().overflow_y_scrollbar().child(self.render_tree(cx))),
            )
            .child(
                // Content pane: markdown rendering.
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_y_scrollbar()
                    .child(if self.doc_loading {
                        div()
                            .p_6()
                            .font_family("JetBrains Mono")
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("加载中…")
                    } else if let Some(path) = &self.doc_path {
                        div()
                            .p_6()
                            .flex_1()
                            .child(
                                div()
                                    .pb_4()
                                    .mb_4()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(path.clone()),
                            )
                            .child(
                                TextView::markdown("doc-content", self.doc_content.clone())
                                    .w_full(),
                            )
                    } else {
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family("JetBrains Mono")
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("选择左侧文档")
                    }),
            )
    }

    fn do_login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let server = {
            let v = self.server_input.read(cx).value().to_string();
            if v.trim().is_empty() {
                "http://127.0.0.1:9090".to_string()
            } else {
                v.trim().to_string()
            }
        };
        let username = self.user_input.read(cx).value().to_string();
        let password = self.password_input.read(cx).value().to_string();
        if username.trim().is_empty() || password.is_empty() {
            self.login_error = Some("请输入用户名和密码".into());
            cx.notify();
            return;
        }
        let client = Client::new(&server);
        self.client = Some(client.clone());
        self.login_error = None;
        cx.spawn(async move |this, cx| {
            match client.login(username.trim(), &password).await {
                Ok(user) => {
                    let _ = this.update(cx, |app, cx| app.on_login_ok(user, cx));
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.login_error = Some(e.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn on_login_ok(&mut self, user: dto::User, cx: &mut Context<Self>) {
        self.username = user.username;
        self.screen = Screen::Workspace;
        self.login_error = None;
        self.load_projects(cx);
    }

    fn load_projects(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.loading = true;
        cx.spawn(async move |this, cx| {
            let projects = client.projects().await;
            let meta = client.meta().await;
            match projects {
                Ok(list) => {
                    let _ = this.update(cx, |app, cx| {
                        *app.projects.write().unwrap() =
                            list.iter().map(ProjectRow::from_dto).collect();
                        app.loading = false;
                        if let Ok(m) = meta {
                            app.meta_version = Some(m.version);
                        }
                        app.table.update(cx, |s, cx| s.refresh(cx));
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.login_error = Some(e.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn logout(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Login;
        self.client = None;
        self.username.clear();
        self.login_error = None;
        *self.projects.write().unwrap() = Vec::new();
        self.table.update(cx, |s, cx| s.refresh(cx));
        cx.notify();
    }

    fn open_new_project_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let handle = cx.entity();
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

            let create_name = name_state.clone();
            let create_desc = desc_state.clone();
            let create_client = client.clone();
            let app_handle = handle.clone();
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
                    let c = create_client.clone();
                    let h = app_handle.clone();
                    cx.spawn(async move |cx| {
                        match c.create_project(name.trim(), desc.trim()).await {
                            Ok(p) => {
                                let _ = h.update(cx, |app, cx| {
                                    app.projects
                                        .write()
                                        .unwrap()
                                        .push(ProjectRow::from_dto(&p));
                                    app.table.update(cx, |s, cx| s.refresh(cx));
                                    cx.notify();
                                });
                            }
                            Err(e) => {
                                let _ = h.update(cx, |app, cx| {
                                    app.login_error = Some(e.to_string());
                                    cx.notify();
                                });
                            }
                        }
                    })
                    .detach();
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
                                this.do_login(window, cx)
                            })),
                    )
                    .child(if let Some(err) = &self.login_error {
                        div()
                            .font_family("JetBrains Mono")
                            .text_xs()
                            .text_color(theme.danger)
                            .child(err.clone())
                    } else {
                        div()
                    })
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .font_family("JetBrains Mono")
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("session · cookie")
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
                                    .child(self.username.clone()),
                            )
                            .child(if let Some(v) = &self.meta_version {
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("v{v}"))
                            } else {
                                div()
                            })
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
                            )
                            .child(if self.loading {
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("加载中…")
                            } else if self.projects.read().unwrap().is_empty() {
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("还没有项目 · 点击右上角新建")
                            } else {
                                div()
                            }),
                    ),
            )
    }
}

impl Render for XWikiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.screen {
            Screen::Login => self.render_login(cx),
            Screen::Workspace => {
                if self.selected_project.is_some() {
                    self.render_doc_view(cx)
                } else {
                    self.render_workspace(cx)
                }
            }
        }
    }
}
