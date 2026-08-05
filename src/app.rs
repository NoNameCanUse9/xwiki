use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

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
    // Edit state: page lock + markdown editor + commit message.
    editing: bool,
    edit_path: Option<String>,
    lock_held: bool,
    heartbeat_stop: Arc<AtomicBool>,
    status_msg: Option<String>,
    commit_msg: Entity<InputState>,
    editor_input: Entity<InputState>,
    // History view state.
    history_open: bool,
    commits: Vec<dto::Commit>,
    commit_detail: Option<dto::CommitDetail>,
    diff_stats: Vec<dto::DiffStat>,
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

        let commit_msg = cx.new(|cx| {
            InputState::new(window, cx).placeholder("提交消息…")
        });
        let editor_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("# 用 Markdown 写作…")
        });

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
            editing: false,
            edit_path: None,
            lock_held: false,
            heartbeat_stop: Arc::new(AtomicBool::new(true)),
            status_msg: None,
            commit_msg,
            editor_input,
            history_open: false,
            commits: Vec::new(),
            commit_detail: None,
            diff_stats: Vec::new(),
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

    // ----- Edit flow: acquire lock -> edit -> changeset commit -----

    fn start_edit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) =
            (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let Some(path) = self.doc_path.clone() else {
            return;
        };
        self.status_msg = None;
        cx.spawn(async move |this, cx| {
            match client.acquire_lock(&project, &path).await {
                Ok(_) => {
                    let _ = this.update(cx, |app, cx| app.begin_editing(&path, cx));
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.status_msg = Some(format!("无法编辑: {e}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn begin_editing(&mut self, path: &str, cx: &mut Context<Self>) {
        let content = self.doc_content.clone();
        let editor = self.editor_input.clone();
        let commit = self.commit_msg.clone();
        if let Some(handle) = cx.active_window() {
            let _ = cx.update_window(handle, |_view, window, cx| {
                editor.update(cx, |s, cx| s.set_value(content, window, cx));
                commit.update(cx, |s, cx| s.set_value(String::new(), window, cx));
            });
        }
        self.edit_path = Some(path.to_string());
        self.editing = true;
        self.lock_held = true;
        self.start_heartbeat(cx);
        cx.notify();
    }

    fn start_heartbeat(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project), Some(path)) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.edit_path.clone(),
        ) else {
            return;
        };
        let stop = self.heartbeat_stop.clone();
        stop.store(false, Ordering::Relaxed);
        cx.spawn(async move |this, cx| {
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(25)).await;
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if client.heartbeat_lock(&project, &path).await.is_err() {
                    let _ = this.update(cx, |app, cx| {
                        app.status_msg = Some("锁续租失败".into());
                        cx.notify();
                    });
                    break;
                }
            }
        })
        .detach();
    }

    fn stop_heartbeat(&self) {
        self.heartbeat_stop.store(true, Ordering::Relaxed);
    }

    fn cancel_edit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.stop_heartbeat();
        self.editing = false;
        self.lock_held = false;
        let (client, project, path) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.edit_path.take(),
        );
        if let (Some(client), Some(project), Some(path)) = (client, project, path) {
            cx.spawn(async move |_this, _cx| {
                let _ = client.release_lock(&project, &path).await;
            })
            .detach();
        }
        self.status_msg = None;
        cx.notify();
    }

    fn save_edit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) =
            (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let Some(path) = self.edit_path.clone() else {
            return;
        };
        let content = self.editor_input.read(cx).value().to_string();
        let msg = self.commit_msg.read(cx).value().to_string();
        if msg.trim().is_empty() {
            self.status_msg = Some("需要提交消息".into());
            cx.notify();
            return;
        }
        self.status_msg = None;
        cx.spawn(async move |this, cx| {
            let base = match client.revision(&project).await {
                Ok(rev) => rev,
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.status_msg = Some(format!("读取 revision 失败: {e}"));
                        cx.notify();
                    });
                    return;
                }
            };
            let change = dto::Change {
                op: "update".into(),
                path: path.clone(),
                new_path: None,
                content: Some(content),
            };
            match client
                .apply_changeset(&project, &base, msg.trim(), vec![change])
                .await
            {
                Ok(_) => {
                    let _ = this.update(cx, |app, cx| app.after_save(cx));
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.status_msg = Some(format!("提交失败: {e}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn after_save(&mut self, cx: &mut Context<Self>) {
        self.stop_heartbeat();
        self.editing = false;
        self.lock_held = false;
        let (client, project, path) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.edit_path.take(),
        );
        if let (Some(client), Some(project), Some(path)) = (client, project, path) {
            cx.spawn(async move |_this, _cx| {
                let _ = client.release_lock(&project, &path).await;
            })
            .detach();
        }
        let path = self.doc_path.clone().unwrap_or_default();
        self.open_doc(&path, cx);
    }

    // ----- History: commit list + per-commit diff stats -----

    fn open_history(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.history_open = true;
        self.commit_detail = None;
        self.diff_stats.clear();
        self.load_commits(cx);
    }

    fn close_history(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.history_open = false;
        cx.notify();
    }

    fn load_commits(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) =
            (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        cx.spawn(async move |this, cx| {
            match client.commits(&project, 50).await {
                Ok(list) => {
                    let _ = this.update(cx, |app, cx| {
                        app.commits = list;
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.status_msg = Some(format!("加载历史失败: {e}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn select_commit(&mut self, sha: &str, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) =
            (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let sha = sha.to_string();
        self.commit_detail = None;
        self.diff_stats.clear();
        cx.spawn(async move |this, cx| {
            let detail = client.commit_detail(&project, &sha).await;
            let stats = client.diff_stats(&project, &sha).await;
            let _ = this.update(cx, |app, cx| {
                if let Ok(d) = detail {
                    app.commit_detail = Some(d);
                }
                if let Ok(s) = stats {
                    app.diff_stats = s;
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_history_view(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(
                // History toolbar.
                div()
                    .h(px(44.0))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(
                        div()
                            .font_family("JetBrains Mono")
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("HISTORY"),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("close-history")
                            .rounded(px(6.0))
                            .label("← 返回文档")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_history(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .h_full()
                    .child(
                        // Commit list.
                        div()
                            .w(px(340.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(theme.border)
                            .overflow_y_scrollbar()
                            .children(self.commits.iter().map(|c| {
                                let sha = c.sha.clone();
                                let short: String = c.sha.chars().take(7).collect();
                                div()
                                    .id(format!("commit-{short}"))
                                    .px_3()
                                    .py_2()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .hover(|s| s.bg(theme.list_hover))
                                    .cursor_pointer()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.foreground)
                                            .child(short),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child(c.message.clone()),
                                    )
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(format!("{} · {}", c.author, c.date)),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_commit(&sha, cx);
                                    }))
                            })),
                    )
                    .child(
                        // Commit detail: message, files, numstat.
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .overflow_y_scrollbar()
                            .p_6()
                            .child(if let Some(d) = &self.commit_detail {
                                div()
                                    .v_flex()
                                    .gap_3()
                                    .w_full()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(d.message.clone()),
                                    )
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(format!(
                                                "{} · {} · {}",
                                                d.sha, d.author, d.date
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .h(px(1.0))
                                            .bg(theme.border),
                                    )
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("FILES"),
                                    )
                                    .children(d.files.iter().map(|f| {
                                        let color = if f.status.as_str() == "D" {
                                            theme.danger
                                        } else {
                                            theme.foreground
                                        };
                                        div()
                                            .flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .font_family("JetBrains Mono")
                                                    .text_xs()
                                                    .text_color(theme.foreground)
                                                    .child(f.status.clone()),
                                            )
                                            .child(
                                                div()
                                                    .font_family("JetBrains Mono")
                                                    .text_xs()
                                                    .text_color(color)
                                                    .child(f.path.clone()),
                                            )
                                    }))
                                    .child(
                                        div()
                                            .w_full()
                                            .h(px(1.0))
                                            .bg(theme.border),
                                    )
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("NUMSTAT"),
                                    )
                                    .children(self.diff_stats.iter().map(|s| {
                                        div()
                                            .flex()
                                            .gap_3()
                                            .items_center()
                                            .child(
                                                div()
                                                    .w(px(56.0))
                                                    .text_right()
                                                    .font_family("JetBrains Mono")
                                                    .text_xs()
                                                    .text_color(theme.foreground)
                                                    .child(format!("+{}", s.added)),
                                            )
                                            .child(
                                                div()
                                                    .w(px(56.0))
                                                    .text_right()
                                                    .font_family("JetBrains Mono")
                                                    .text_xs()
                                                    .text_color(theme.danger)
                                                    .child(format!("-{}", s.deleted)),
                                            )
                                            .child(
                                                div()
                                                    .font_family("JetBrains Mono")
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child(s.path.clone()),
                                            )
                                    }))
                            } else {
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("选择左侧提交查看详情")
                            }),
                    ),
            )
    }

    fn render_editor_view(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(
                // Edit toolbar: path, commit message, save/cancel.
                div()
                    .h(px(44.0))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(
                        div()
                            .font_family("JetBrains Mono")
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(self.edit_path.clone().unwrap_or_default()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .w(px(320.0))
                            .child(Input::new(&self.commit_msg)),
                    )
                    .child(
                        Button::new("save-edit")
                            .primary()
                            .rounded(px(6.0))
                            .label("保存")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_edit(window, cx)
                            })),
                    )
                    .child(
                        Button::new("cancel-edit")
                            .rounded(px(6.0))
                            .label("取消")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel_edit(window, cx)
                            })),
                    ),
            )
            .child(if let Some(msg) = &self.status_msg {
                div()
                    .px_4()
                    .py_1()
                    .font_family("JetBrains Mono")
                    .text_xs()
                    .text_color(theme.danger)
                    .child(msg.clone())
            } else {
                div()
            })
            .child(
                div()
                    .flex_1()
                    .p_4()
                    .child(Input::new(&self.editor_input).h_full().w_full()),
            )
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
                // Content pane: markdown rendering (or the editor).
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(if self.history_open {
                        self.render_history_view(cx).into_any_element()
                    } else if self.editing {
                        self.render_editor_view(cx).into_any_element()
                    } else {
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
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .font_family("JetBrains Mono")
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child(path.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_2()
                                                    .child(
                                                        Button::new("open-history")
                                                            .rounded(px(6.0))
                                                            .label("历史")
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.open_history(
                                                                        window, cx,
                                                                    )
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        Button::new("start-edit")
                                                            .rounded(px(6.0))
                                                            .label("编辑")
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.start_edit(
                                                                        window, cx,
                                                                    )
                                                                },
                                                            )),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        TextView::markdown(
                                            "doc-content",
                                            self.doc_content.clone(),
                                        )
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
                            })
                            .into_any_element()
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
