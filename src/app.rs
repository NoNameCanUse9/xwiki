use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use gpui::*;
use gpui::StatefulInteractiveElement;
use gpui_component::{
    button::*,
    dialog::DialogContent,
    input::{Input, InputEvent, InputState},
    menu::ContextMenuExt,
    scroll::ScrollableElement as _,
    notification::Notification,
    *,
};

use crate::api::{Client, dto};
use crate::config;
use crate::ui::{mono_label, split_pane, tokens};
use crate::{QuickOpen, TogglePalette, ToggleTheme};

/// Command palette entries: (id, label, hint). Availability is decided by
/// the current screen state in `palette_commands`.
#[derive(Clone)]
struct PaletteCmd {
    id: &'static str,
    label: &'static str,
    hint: &'static str,
}

/// A save-time revision conflict (409): the doc changed on the server since
/// we started editing. The panel offers reload / force retry / abandon.
#[derive(Clone)]
pub(crate) struct ConflictInfo {
    pub message: String,
    pub path: String,
}

/// Destination of a ⌘P quick-open entry.
#[derive(Clone)]
pub(crate) enum QuickTarget {
    OpenProject(String),
    OpenDoc(String),
    EnterDir(String),
    BackToProjects,
}

/// Right-click menu action for a doc-tree row.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct TreeContextAction {
    pub path: String,
    pub is_dir: bool,
}

/// Right-click menu action for a project card / rail row.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct ProjectContextAction {
    pub project_id: String,
}

/// Right-click "edit" action: loads the doc, then acquires the lock.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct EditDocAction {
    pub path: String,
}

/// A filterable ⌘P entry.
#[derive(Clone)]
pub(crate) struct QuickItem {
    pub label: String,
    pub hint: String,
    pub target: QuickTarget,
}

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
    /// Selected commit sha (highlight in the history list).
    selected_sha: Option<String>,
    /// Panel layout (widths + history visibility), persisted via config.
    layout: config::Layout,
    /// Server URL as resolved at login (status bar).
    server_url: String,
    /// Tree keyboard-navigation cursor (index into visible entries).
    tree_focus: Option<usize>,
    /// Save-time revision conflict (render_main_pane shows the recovery panel).
    conflict: Option<ConflictInfo>,
    /// Transient load errors with retry (per-area, not the login banner).
    tree_error: Option<String>,
    doc_error: Option<String>,
    projects_error: Option<String>,
    /// Settings view: server URL input + connection-test result.
    settings_server_input: Entity<InputState>,
    settings_test: Option<(bool, String)>,
    /// ⌘P quick-open overlay state.
    quick_open: bool,
    quick_input: Entity<InputState>,
    /// Edit requested from a context menu while the doc is still loading.
    pending_edit: Option<String>,
    /// Cached window title; `set_window_title` only on change.
    last_title: String,
    /// Keep input subscriptions alive with the app entity.
    _subscriptions: Vec<Subscription>,
}

enum Screen {
    Login,
    Workspace,
    Settings,
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

impl XWikiApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let server_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(config::load_server())
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

        let settings_server_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(config::load_server(), window, cx);
            state
        });
        let quick_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("项目 / 文档…"));
        let mut subs = Vec::new();
        for state in [&server_input, &user_input, &password_input] {
            subs.push(cx.subscribe_in(state, window, |_, _, _: &InputEvent, _, cx| {
                cx.notify()
            }));
        }
        // Filter keystrokes re-render the project cards.
        {
            let filter = filter_input.clone();
            subs.push(cx.subscribe_in(
                &filter_input,
                window,
                move |_, _, _: &InputEvent, _, cx| {
                    let _ = filter.read(cx);
                    cx.notify();
                },
            ));
        }
        // ⌘P quick-open keystrokes re-render the overlay list.
        {
            let quick = quick_input.clone();
            subs.push(cx.subscribe_in(
                &quick_input,
                window,
                move |_, _, _: &InputEvent, _, cx| {
                    let _ = quick.read(cx);
                    cx.notify();
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
            selected_sha: None,
            layout: config::load_layout(),
            server_url: config::load_server(),
            tree_focus: None,
            conflict: None,
            tree_error: None,
            doc_error: None,
            projects_error: None,
            settings_server_input,
            settings_test: None,
            quick_open: false,
            quick_input,
            pending_edit: None,
            last_title: String::new(),
            _subscriptions: subs,
        }
    }

    fn open_project(&mut self, project_id: &str, cx: &mut Context<Self>) {
        self.selected_project = Some(project_id.to_string());
        self.selected_sha = None;
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
        self.tree_error = None;
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
                        app.tree_error = Some(e.to_string());
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
        self.doc_error = None;
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            match client.page(&project, &path).await {
                Ok(page) => {
                    let _ = this.update(cx, |app, cx| {
                        app.doc_content = page.content;
                        app.doc_loading = false;
                        // Context-menu edit: acquire the lock now that the
                        // content is loaded.
                        if app.pending_edit.take().as_deref() == Some(path.as_str()) {
                            app.start_edit(cx);
                        }
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.doc_loading = false;
                        app.doc_error = Some(e.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn back_to_projects(&mut self, cx: &mut Context<Self>) {
        self.selected_project = None;
        self.tree_entries.clear();
        self.tree_path.clear();
        self.doc_path = None;
        self.doc_content.clear();
        cx.notify();
    }

    // ----- Edit flow: acquire lock -> edit -> changeset commit -----

    fn start_edit(&mut self, cx: &mut Context<Self>) {
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

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
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

    fn save_edit(&mut self, cx: &mut Context<Self>) {
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
                    let is_conflict = e.code == "revision_conflict";
                    let message = e.message.clone();
                    let _ = this.update(cx, |app, cx| {
                        if is_conflict {
                            app.conflict = Some(ConflictInfo {
                                message,
                                path: path.clone(),
                            });
                        } else {
                            app.status_msg = Some(format!("提交失败: {e}"));
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn after_save(&mut self, cx: &mut Context<Self>) {
        self.notify("已提交".into(), cx);
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

    fn open_history(&mut self, cx: &mut Context<Self>) {
        self.history_open = true;
        self.commit_detail = None;
        self.diff_stats.clear();
        config::save_layout(&self.layout);
        self.load_commits(cx);
    }

    fn close_history(&mut self, cx: &mut Context<Self>) {
        self.history_open = false;
        config::save_layout(&self.layout);
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
        self.selected_sha = Some(sha.clone());
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
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar)
            .child(
                // Panel header: mono label + close (context panel, not a screen).
                div()
                    .h(px(tokens::TOOLBAR_H))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(mono_label("HISTORY").text_color(theme.muted_foreground))
                    .child(div().flex_1())
                    .child(
                        Button::new("close-history")
                            .rounded(px(tokens::RADIUS))
                            .label("关闭")
                            .tooltip("关闭历史面板 (Esc)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_history(cx)
                            })),
                    ),
            )
            .child(
                // Commit list (top): message first, sha/author/date meta.
                div()
                    .flex_1()
                    .min_h(px(120.0))
                    .overflow_y_scrollbar()
                    .children(self.commits.iter().map(|c| {
                        let sha = c.sha.clone();
                        let short: String = c.sha.chars().take(7).collect();
                        let selected = self.selected_sha.as_deref() == Some(c.sha.as_str());
                        let row = div()
                            .id(format!("commit-{short}"))
                            .px_3()
                            .py_2_5()
                            .border_b_1()
                            .border_color(theme.border)
                            .cursor_pointer()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(c.message.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(short)
                                    .child(format!("{} · {}", c.author, c.date)),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_commit(&sha, cx);
                            }));
                        if selected {
                            row.bg(theme.list_active)
                        } else {
                            row.hover(|s| s.bg(theme.list_hover))
                        }
                    })),
            )
            .child(
                // Commit detail (bottom): message, files, numstat — stays in
                // sync with the selected commit above.
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_4()
                    .border_t_1()
                    .border_color(theme.border)
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
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "{} · {} · {}",
                                        d.sha, d.author, d.date
                                    )),
                            )
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("FILES").text_color(theme.muted_foreground))
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
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.foreground)
                                            .child(f.status.clone()),
                                    )
                                    .child(
                                        div()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(color)
                                            .child(f.path.clone()),
                                    )
                            }))
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("NUMSTAT").text_color(theme.muted_foreground))
                            .children(self.diff_stats.iter().map(|s| {
                                div()
                                    .flex()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        div()
                                            .w(px(tokens::NUMSTAT_W))
                                            .text_right()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.foreground)
                                            .child(format!("+{}", s.added)),
                                    )
                                    .child(
                                        div()
                                            .w(px(tokens::NUMSTAT_W))
                                            .text_right()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.danger)
                                            .child(format!("-{}", s.deleted)),
                                    )
                                    .child(
                                        div()
                                            .font_family(tokens::FONT_MONO)
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
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("选择上方提交查看详情")
                    }),
            )
    }

    // ----- Command palette (⌘K), theme toggle, notifications -----

    fn palette_commands(&self) -> Vec<PaletteCmd> {
        let mut cmds = Vec::new();
        if self.selected_project.is_some() {
            cmds.push(PaletteCmd {
                id: "back",
                label: "返回项目列表",
                hint: "esc",
            });
            if !self.history_open {
                cmds.push(PaletteCmd {
                    id: "history",
                    label: "查看历史",
                    hint: "commits",
                });
            }
            if !self.editing && self.doc_path.is_some() {
                cmds.push(PaletteCmd {
                    id: "edit",
                    label: "编辑当前文档",
                    hint: "lock + changeset",
                });
            }
        } else if self.client.is_some() {
            cmds.push(PaletteCmd {
                id: "new-project",
                label: "新建项目",
                hint: "dialog",
            });
        }
        cmds.push(PaletteCmd {
            id: "theme",
            label: "切换主题",
            hint: "⌘⇧T",
        });
        if self.client.is_some() {
            cmds.push(PaletteCmd {
                id: "settings",
                label: "设置",
                hint: "server · 连接",
            });
        }
        cmds.push(PaletteCmd {
            id: "logout",
            label: "退出登录",
            hint: "session",
        });
        cmds
    }

    fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = cx.entity();
        let cmds = self.palette_commands();
        let content_cmds = cmds.clone();
        let content_handle = handle.clone();
        window.open_dialog(cx, move |dialog, _window, cx| {
            let theme = cx.theme();
            dialog
                .title(
                    div()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("命令面板 · ⌘K"),
                )
                .content({
                    let cc = content_cmds.clone();
                    let ch = content_handle.clone();
                    move |content, _window, cx| {
                    let theme = cx.theme();
                    let mut list = div().v_flex().gap_1().w_full();
                    for c in &cc {
                        let handle = ch.clone();
                        let id = c.id;
                        let label = c.label;
                        let hint = c.hint;
                        list = list.child(
                            div()
                                .id(format!("cmd-{id}"))
                                .flex()
                                .items_center()
                                .justify_between()
                                .px_2()
                                .py_1_5()
                                .rounded(px(tokens::RADIUS_SMALL))
                                .hover(|s| s.bg(theme.list_hover))
                                .cursor_pointer()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.foreground)
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(hint),
                                )
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let h = handle.clone();
                                    h.update(cx, |app, cx| {
                                        app.run_command(id, cx)
                                    });
                                }),
                        );
                    }
                    content.child(list)
                    }
                })
        });
    }

    fn run_command(&mut self, id: &str, cx: &mut Context<Self>) {
        match id {
            "back" => self.back_to_projects(cx),
            "history" => self.open_history(cx),
            "edit" => self.start_edit(cx),
            "new-project" => {
                let handle = cx.entity();
                cx.defer(move |cx| {
                    let inner = handle.clone();
                    handle.update(cx, |app, cx| {
                        let client = app.client.clone();
                        if let Some(h) = cx.active_window() {
                            let inner = inner.clone();
                            let _ = cx.update_window(h, move |_v, window, cx| {
                                open_new_project_dialog_window(window, cx, client, inner);
                            });
                        }
                    });
                });
            }
            "theme" => self.toggle_theme(cx),
            "settings" => {
                self.quick_open = false;
                self.screen = Screen::Settings;
            }
            "logout" => self.logout(cx),
            _ => {}
        }
        cx.notify();
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let next = if cx.theme().is_dark() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        config::save_theme(next);
        Theme::change(next, None, cx);
        cx.notify();
    }

    fn notify(&mut self, message: String, cx: &mut Context<Self>) {
        if let Some(h) = cx.active_window() {
            let _ = cx.update_window(h, move |_view, window, cx| {
                window.push_notification(Notification::success(message), cx);
            });
        }
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
                    .h(px(tokens::TOOLBAR_H))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
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
                            .rounded(px(tokens::RADIUS))
                            .label("保存")
                            .tooltip("提交修改 (⌘S)")
                            .disabled(!self.lock_held)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_edit(cx)
                            })),
                    )
                    .child(
                        Button::new("cancel-edit")
                            .rounded(px(tokens::RADIUS))
                            .label("取消")
                            .tooltip("放弃修改并释放锁 (Esc)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cancel_edit(cx)
                            })),
                    ),
            )
            .child(if let Some(msg) = &self.status_msg {
                div()
                    .px_4()
                    .py_1()
                    .font_family(tokens::FONT_MONO)
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
    /// navigates back up. Keyboard: ↑/↓ move the focus cursor, → enters a
    /// directory, ← goes up a level, Enter opens (plan §3.1).
    fn render_tree(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // Owned (path, is_dir) pairs: the keyboard listener below is 'static
        // and must not borrow from `self`.
        let items: Vec<(String, bool)> = self
            .tree_entries
            .iter()
            .filter(|e| e.path != "_sidebar.md")
            .map(|e| (e.path.clone(), e.r#type == "tree"))
            .collect();
        let mut list = div().v_flex().w_full();
        let count = items.len();
        list = list.child(
            div()
                .px_3()
                .py_2()
                .font_family(tokens::FONT_MONO)
                .text_size(px(tokens::FONT_SIZE_LABEL))
                .text_color(theme.muted_foreground)
                .child(format!("root · {count} {}", if count == 1 { "item" } else { "items" })),
        );
        if items.is_empty() {
            list = list.child(
                div()
                    .mx_3()
                    .my_4()
                    .px_4()
                    .py_6()
                    .rounded(px(tokens::RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .text_center()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(mono_label("空目录")),
            );
            return list.into_any_element();
        }
        let focus_bar = theme.accent;
        for (i, (path, is_dir)) in items.iter().enumerate() {
            let is_selected =
                !is_dir && self.doc_path.as_deref() == Some(path.as_str());
            let is_focused = self.tree_focus == Some(i);
            let path_owned = path.clone();
            let is_dir_owned = *is_dir;
            let row = div()
                .id(path.clone())
                .flex()
                .items_center()
                .gap_2_5()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme.border)
                .cursor_pointer()
                .child(
                    // Selection bar: cobalt signal on the focused/active row.
                    div()
                        .w(px(2.0))
                        .self_stretch()
                        .bg(if is_selected || is_focused {
                            focus_bar
                        } else {
                            gpui::transparent_black()
                        }),
                )
                .child(
                    div()
                        .w(px(16.0))
                        .text_center()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(if is_dir_owned {
                            theme.accent
                        } else {
                            theme.muted_foreground
                        })
                        .child(if is_dir_owned { "▸" } else { "·" }),
                )
                .child(
                    div()
                        .flex_1()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(if is_selected || is_focused || is_dir_owned {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(path_owned.split('/').next_back().unwrap_or("").to_string()),
                )
                .on_click({
                    let click_path = path_owned.clone();
                    cx.listener(move |this, _, _, cx| {
                        this.tree_focus = Some(i);
                        if is_dir_owned {
                            this.load_tree(&click_path, cx);
                        } else {
                            this.open_doc(&click_path, cx);
                        }
                    })
                });
            let row = if is_selected || is_focused {
                row.bg(theme.list_active)
            } else {
                row.hover(|s| s.bg(theme.list_hover))
            };
            // Plan §3.2: right-click on a tree row.
            let ctx_path = path_owned.clone();
            let ctx_dir = is_dir_owned;
            let row = row.context_menu(move |menu, _window, _cx| {
                let mut m = menu.menu(
                    if ctx_dir { "进入目录" } else { "打开" },
                    Box::new(TreeContextAction {
                        path: ctx_path.clone(),
                        is_dir: ctx_dir,
                    }),
                );
                if !ctx_dir {
                    m = m.menu(
                        "编辑",
                        Box::new(EditDocAction { path: ctx_path.clone() }),
                    );
                }
                m
            });
            list = list.child(row);
        }
        // Keyboard: wrap the list in a focusable container.
        let dirs: Vec<String> = items
            .iter()
            .filter(|(_, d)| *d)
            .map(|(p, _)| p.clone())
            .collect();
        let items_clone = items.clone();
        div()
            .id("tree-keyboard")
            .w_full()
            .focusable()
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _w, cx| {
                let n = items_clone.len();
                if n == 0 {
                    return;
                }
                let cur = this.tree_focus.unwrap_or(0).min(n - 1);
                match event.keystroke.key.as_str() {
                    "up" => this.tree_focus = Some((cur + n - 1) % n),
                    "down" => this.tree_focus = Some((cur + 1) % n),
                    "right" => {
                        if let Some(dir) = dirs.iter().find(|p| {
                            items_clone
                                .iter()
                                .position(|(path, _)| path == *p)
                                .map(|idx| idx == cur)
                                .unwrap_or(false)
                        }) {
                            this.load_tree(dir, cx);
                            return;
                        }
                    }
                    "left" => {
                        let parent = this.tree_path.rsplit_once('/').map(|(p, _)| p.to_string());
                        this.load_tree(parent.as_deref().unwrap_or(""), cx);
                        return;
                    }
                    "enter" => {
                        let (path, is_dir) = &items_clone[cur];
                        if *is_dir {
                            this.load_tree(path, cx);
                        } else {
                            this.open_doc(path, cx);
                        }
                    }
                    _ => return,
                }
                cx.notify();
            }))
            .child(list)
            .into_any_element()
    }

    /// Project card grid (web home.tsx style): hairline panels, hover lift,
    /// display title + mono meta + description + status row.
    fn render_project_cards(&self, cx: &mut Context<Self>) -> Div {
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
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(if p.description.is_empty() {
                            "—".to_string()
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
                    cx.listener(move |this, _, _, cx| {
                        this.open_project(&click_id, cx)
                    })
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

    fn render_doc_view(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let app_handle = cx.entity();
        // Build the content side first so `window` isn't borrowed twice in the
        // splitter call (it may itself open the history split).
        let content = self.render_doc_content(window, cx);
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                split_pane::horizontal(
                    "doc-rail-split",
                    self.layout.doc_rail,
                    tokens::DOC_RAIL_MIN,
                    tokens::DOC_RAIL_MAX,
                    tokens::DOC_RAIL,
                    window,
                    theme.border,
                    theme.list_hover,
                    self.render_doc_rail(cx),
                    content,
                    move |w, _window, cx| {
                        app_handle.update(cx, |app, cx| {
                            app.layout.doc_rail = w;
                            config::save_layout(&app.layout);
                            cx.notify();
                        });
                    },
                ),
            )
            .child(self.render_status_bar(cx))
    }

    /// Doc rail: tree navigation + breadcrumb (left of the rail split).
    fn render_doc_rail(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .h_full()
            .flex()
            .flex_col()
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
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("DOCS"),
                    )
                    .child(
                        Button::new("back-projects")
                            .rounded(px(tokens::RADIUS))
                            .label("← 项目")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.back_to_projects(cx)
                            })),
                    ),
            )
            .child(
                // Breadcrumb: docs › dir1 › dir2 (web style).
                div()
                    .px_3()
                    .pb_2()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id("crumb-root")
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .hover(|s| s.text_color(theme.accent))
                            .cursor_pointer()
                            .child("docs")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load_tree("", cx);
                            })),
                    )
                    .children({
                        let mut acc = String::new();
                        let mut parts: Vec<String> = Vec::new();
                        for part in self.tree_path.split('/') {
                            if part.is_empty() {
                                continue;
                            }
                            acc = if acc.is_empty() {
                                part.to_string()
                            } else {
                                format!("{acc}/{part}")
                            };
                            parts.push(acc.clone());
                        }
                        let mut out: Vec<AnyElement> = Vec::new();
                        for (i, full) in parts.iter().enumerate() {
                            let is_last = i == parts.len() - 1;
                            let target = full.clone();
                            let name: String = full
                                .split('/')
                                .next_back()
                                .unwrap_or("")
                                .to_string();
                            out.push(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("›")
                                    .into_any_element(),
                            );
                            if is_last {
                                out.push(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.foreground)
                                        .child(name)
                                        .into_any_element(),
                                );
                            } else {
                                out.push(
                                    div()
                                        .id(format!("crumb-{target}"))
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .hover(|s| s.text_color(theme.accent))
                                        .cursor_pointer()
                                        .child(name)
                                        .on_click(cx.listener(
                                            move |this, _, _, cx| {
                                                this.load_tree(&target, cx);
                                            },
                                        ))
                                        .into_any_element(),
                                );
                            }
                        }
                        out
                    }),
            )
            .child(if let Some(err) = &self.tree_error {
                div()
                    .flex_1()
                    .p_3()
                    .v_flex()
                    .gap_3()
                    .child(
                        mono_label("目录加载失败")
                            .text_color(theme.danger),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(err.clone()),
                    )
                    .child(
                        Button::new("retry-tree")
                            .rounded(px(tokens::RADIUS))
                            .label("重试")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let path = this.tree_path.clone();
                                this.load_tree(&path, cx);
                            })),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(self.render_tree(cx))
                    .into_any_element()
            })
    }

    /// Content area: reading/editor, plus the history context panel on the
    /// right when open.
    fn render_doc_content(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let main = div()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .child(self.render_main_pane(cx));
        if !self.history_open {
            return main;
        }
        let app_handle = cx.entity();
        split_pane::horizontal_right(
            "history-split",
            self.layout.history,
            tokens::HISTORY_W_MIN,
            tokens::HISTORY_W_MAX,
            tokens::HISTORY_W,
            window,
            theme.border,
            theme.list_hover,
            main,
            self.render_history_view(cx),
            move |w, _window, cx| {
                app_handle.update(cx, |app, cx| {
                    app.layout.history = w;
                    config::save_layout(&app.layout);
                    cx.notify();
                });
            },
        )
    }

    /// Main pane: the editor, or the reading view, or a select-a-doc hint.
    fn render_main_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        if let Some(c) = &self.conflict {
            return self
                .render_conflict_panel(c, cx)
                .into_any_element();
        }
        if self.editing {
            return self.render_editor_view(cx).into_any_element();
        }
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .overflow_y_scrollbar()
            .child(if self.doc_loading {
                // Plan §4: skeleton while the page loads.
                div()
                    .p_6()
                    .v_flex()
                    .gap_3()
                    .w_full()
                    .child(
                        div()
                            .w(px(320.0))
                            .h(px(20.0))
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
                            .w(px(480.0))
                            .h(px(12.0))
                            .rounded(px(tokens::RADIUS_SMALL))
                            .bg(theme.skeleton),
                    )
                    .child(
                        div()
                            .w(px(360.0))
                            .h(px(12.0))
                            .rounded(px(tokens::RADIUS_SMALL))
                            .bg(theme.skeleton),
                    )
            } else if let Some(err) = &self.doc_error {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(
                        mono_label("加载失败")
                            .text_color(theme.danger),
                    )
                    .child(
                        div()
                            .px_4()
                            .text_center()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(err.clone()),
                    )
                    .child(
                        Button::new("retry-doc")
                            .rounded(px(tokens::RADIUS))
                            .label("重试")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let path = this
                                    .doc_path
                                    .clone()
                                    .unwrap_or_default();
                                this.open_doc(&path, cx);
                            })),
                    )
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
                                    .font_family(tokens::FONT_MONO)
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
                                            .rounded(px(tokens::RADIUS))
                                            .label("历史")
                                            .on_click(cx.listener(
                                                |this, _, _, cx| {
                                                    this.open_history(cx)
                                                },
                                            )),
                                    )
                                    .child(
                                        Button::new("start-edit")
                                            .rounded(px(tokens::RADIUS))
                                            .label("编辑")
                                            .on_click(cx.listener(
                                                |this, _, _, cx| {
                                                    this.start_edit(cx)
                                                },
                                            )),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(tokens::MEASURE))
                                    .child(
                                        crate::ui::markdown(
                                            "doc-content",
                                            self.doc_content.clone(),
                                        ),
                                    ),
                            ),
                    )
            } else {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("选择左侧文档")
            })
            .into_any_element()
    }

    fn do_login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let server = {
            let v = self.server_input.read(cx).value().to_string();
            if v.trim().is_empty() {
                config::load_server()
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
        self.server_url = server.clone();
        self.login_error = None;
        cx.spawn(async move |this, cx| {
            match client.login(username.trim(), &password).await {
                Ok(user) => {
                    config::save_server(&server);
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
        self.projects_error = None;
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
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.projects_error = Some(e.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn logout(&mut self, cx: &mut Context<Self>) {
        self.screen = Screen::Login;
        self.client = None;
        self.username.clear();
        self.login_error = None;
        *self.projects.write().unwrap() = Vec::new();
        cx.notify();
    }

    fn open_new_project_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let handle = cx.entity();
        open_new_project_dialog_window(window, cx, Some(client), handle);
    }

    fn eyebrow(&self, label: &'static str, cx: &Context<Self>) -> Div {
        div()
            .font_family(tokens::FONT_MONO)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(label)
    }

    fn render_login(&self, cx: &mut Context<Self>) -> Div {
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
                                    .label("登录")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.do_login(window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("session · argon2id · http-only cookie"),
                            ),
                    ),
            )
    }

    fn render_workspace(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
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
                                    .label(if cx.theme().is_dark() { "浅色" } else { "深色" })
                                    .tooltip("切换主题 (⌘⇧T)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_theme(cx)
                                    })),
                            )
                            .child(
                                Button::new("logout")
                                    .rounded(px(tokens::RADIUS))
                                    .label("退出")
                                    .tooltip("退出登录")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.logout(cx)
                                    })),
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
                                        self.selected_project.as_deref()
                                            == Some(p.id.as_str());
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
                                            div()
                                                .w(px(2.0))
                                                .self_stretch()
                                                .bg(if is_selected {
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
                                        .on_click(cx.listener(
                                            move |this, _, _, cx| {
                                                this.open_project(&id, cx)
                                            },
                                        ));
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
                                        .on_click(cx.listener(
                                            |this, _, window, cx| {
                                                this.open_new_project_dialog(
                                                    window, cx,
                                                )
                                            },
                                        )),
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
                                .child(
                                    mono_label("加载失败")
                                        .text_color(theme.danger),
                                )
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
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.load_projects(cx)
                                        })),
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

    /// Status area (plan §2.1): connection, server, lock/save state, and
    /// transient operation feedback. Hairline-topped, mono readouts.
    fn render_status_bar(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let connected = self.client.is_some();
        div()
            .h(px(tokens::STATUS_H))
            .px_3()
            .flex()
            .items_center()
            .gap_3()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded_full()
                    .bg(if connected {
                        theme.success_foreground
                    } else {
                        theme.danger
                    }),
            )
            .child(
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(if connected { "已连接" } else { "未连接" }),
            )
            .child(
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(self.server_url.clone()),
            )
            .child(div().flex_1())
            .child(if let Some(msg) = &self.status_msg {
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.danger)
                    .child(msg.clone())
            } else {
                div()
            })
            .child(if self.editing {
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(if self.lock_held {
                        theme.success_foreground
                    } else {
                        theme.danger
                    })
                    .child(if self.lock_held {
                        "编辑中 · 锁已持有"
                    } else {
                        "编辑中 · 锁丢失"
                    })
            } else {
                div()
            })
            .child(if let Some(v) = &self.meta_version {
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("v{v}"))
            } else {
                div()
            })
    }

    // ----- Window title & screen helpers -----

    fn screen_is_workspace(&self) -> bool {
        matches!(self.screen, Screen::Workspace)
    }

    /// Dynamic window title: AgentDocs — <project> / <doc>.
    fn window_title(&self) -> String {
        if self.screen_is_workspace() {
            if let Some(project) = self.selected_project.as_deref() {
                let name = self
                    .projects
                    .read()
                    .unwrap()
                    .iter()
                    .find(|p| Some(p.id.as_str()) == self.selected_project.as_deref())
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| project.to_string());
                if let Some(path) = &self.doc_path {
                    return format!("AgentDocs — {name} / {path}");
                }
                return format!("AgentDocs — {name}");
            }
            "AgentDocs".into()
        } else if matches!(self.screen, Screen::Settings) {
            "AgentDocs — 设置".into()
        } else {
            "AgentDocs".into()
        }
    }

    // ----- ⌘P quick open -----

    fn quick_items(&self) -> Vec<QuickItem> {
        let mut items = Vec::new();
        if let Some(_project) = &self.selected_project {
            for e in self.tree_entries.iter().filter(|e| e.path != "_sidebar.md") {
                if e.r#type == "tree" {
                    items.push(QuickItem {
                        label: format!("目录 {}/", e.path),
                        hint: "dir".into(),
                        target: QuickTarget::EnterDir(e.path.clone()),
                    });
                } else {
                    items.push(QuickItem {
                        label: e.path.clone(),
                        hint: "doc".into(),
                        target: QuickTarget::OpenDoc(e.path.clone()),
                    });
                }
            }
            items.push(QuickItem {
                label: "返回项目列表".into(),
                hint: "esc".into(),
                target: QuickTarget::BackToProjects,
            });
        }
        if self.client.is_some() {
            for p in self.projects.read().unwrap().iter() {
                items.push(QuickItem {
                    label: format!("项目 {}", p.name),
                    hint: "project".into(),
                    target: QuickTarget::OpenProject(p.id.clone()),
                });
            }
        }
        items
    }

    fn run_quick_open(&mut self, target: QuickTarget, cx: &mut Context<Self>) {
        match target {
            QuickTarget::OpenProject(id) => self.open_project(&id, cx),
            QuickTarget::OpenDoc(path) => self.open_doc(&path, cx),
            QuickTarget::EnterDir(path) => self.load_tree(&path, cx),
            QuickTarget::BackToProjects => self.back_to_projects(cx),
        }
        cx.notify();
    }

    fn toggle_quick_open(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.quick_open = !self.quick_open;
        if self.quick_open {
            let input = self.quick_input.clone();
            if let Some(h) = cx.active_window() {
                let _ = cx.update_window(h, move |_v, window, cx| {
                    input.update(cx, |s, cx| s.set_value(String::new(), window, cx));
                });
            }
        }
        cx.notify();
    }

    /// Quick-open overlay: filterable list of projects + current tree docs.
    fn render_quick_open(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let q = self.quick_input.read(cx).value().to_lowercase();
        let items: Vec<QuickItem> = self
            .quick_items()
            .into_iter()
            .filter(|i| {
                q.is_empty()
                    || i.label.to_lowercase().contains(&q)
                    || i.hint.contains(&q)
            })
            .collect();
        let mut list = div().v_flex().gap_1().w_full().px_4().pb_4();
        if items.is_empty() {
            list = list.child(
                div()
                    .py_6()
                    .text_center()
                    .font_family(tokens::FONT_MONO)
                    .text_size(px(tokens::FONT_SIZE_LABEL))
                    .text_color(theme.muted_foreground)
                    .child("没有匹配的项目或文档"),
            );
        }
        for it in &items {
            let label = it.label.clone();
            let hint = it.hint.clone();
            let target = it.target.clone();
            list = list.child(
                div()
                    .id(format!("quick-{label}"))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded(px(tokens::RADIUS_SMALL))
                    .hover(|s| s.bg(theme.list_hover))
                    .cursor_pointer()
                    .child(div().text_sm().text_color(theme.foreground).child(label))
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_size(px(tokens::FONT_SIZE_LABEL))
                            .text_color(theme.muted_foreground)
                            .child(hint),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.run_quick_open(target.clone(), cx)
                    })),
            );
        }
        div()
            .id("quick-open-overlay")
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .bg(gpui::rgba(0x0b12204d))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(96.0))
            .child(
                div()
                    .w(px(560.0))
                    .rounded(px(tokens::RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .shadow(vec![BoxShadow::new(px(0.0), px(8.0), gpui::rgba(0x0b122066).into())
                        .blur_radius(px(24.0))])
                    .v_flex()
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                mono_label("快速打开 · ⌘P")
                                    .text_color(theme.muted_foreground),
                            ),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(Input::new(&self.quick_input).w_full()),
                    )
                    .child(div().max_h(px(360.0)).overflow_y_scrollbar().child(list)),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.quick_open = false;
                cx.notify();
            }))
    }

    // ----- Revision-conflict recovery (plan §4) -----

    /// Reload the server copy, dropping the local edit.
    fn resolve_conflict_reload(&mut self, cx: &mut Context<Self>) {
        let path = self.conflict.take().map(|c| c.path);
        self.stop_heartbeat();
        self.editing = false;
        self.lock_held = false;
        self.edit_path = None;
        if let Some(p) = path {
            self.open_doc(&p, cx);
        }
        cx.notify();
    }

    /// Retry the save against the fresh revision (last-writer-wins).
    fn resolve_conflict_force(&mut self, cx: &mut Context<Self>) {
        self.conflict = None;
        self.save_edit(cx);
    }

    /// Abandon the edit, releasing the lock.
    fn resolve_conflict_abandon(&mut self, cx: &mut Context<Self>) {
        self.conflict = None;
        self.cancel_edit(cx);
    }

    fn render_conflict_panel(&self, c: &ConflictInfo, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(520.0))
                    .p_6()
                    .rounded(px(tokens::RADIUS))
                    .border_1()
                    .border_color(theme.danger)
                    .bg(theme.popover)
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                mono_label("保存冲突")
                                    .text_color(theme.danger),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_size(px(tokens::FONT_SIZE_LABEL))
                                    .text_color(theme.muted_foreground)
                                    .child(c.path.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.foreground)
                            .child(c.message.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(
                                "服务器上的文档已被修改。选择如何处理你的本地编辑：",
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(
                                Button::new("conflict-abandon")
                                    .rounded(px(tokens::RADIUS))
                                    .label("放弃编辑")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_abandon(cx)
                                    })),
                            )
                            .child(
                                Button::new("conflict-reload")
                                    .rounded(px(tokens::RADIUS))
                                    .label("重新加载")
                                    .tooltip("丢弃本地修改，加载服务器版本")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_reload(cx)
                                    })),
                            )
                            .child(
                                Button::new("conflict-force")
                                    .primary()
                                    .rounded(px(tokens::RADIUS))
                                    .label("覆盖重试")
                                    .tooltip("以最新 revision 重新提交（后写者胜）")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_force(cx)
                                    })),
                            ),
                    ),
            )
    }

    // ----- Settings view -----

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        let url = {
            let v = self.settings_server_input.read(cx).value().to_string();
            if v.trim().is_empty() {
                config::load_server()
            } else {
                v.trim().to_string()
            }
        };
        self.settings_test = None;
        let c = Client::new(&url);
        cx.spawn(async move |this, cx| {
            match c.meta().await {
                Ok(m) => {
                    let _ = this.update(cx, |app, cx| {
                        app.settings_test =
                            Some((true, format!("连接成功 · server v{}", m.version)));
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.settings_test = Some((false, format!("连接失败: {e}")));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn save_server_settings(&mut self, cx: &mut Context<Self>) {
        let url = self.settings_server_input.read(cx).value().trim().to_string();
        if url.is_empty() {
            self.settings_test = Some((false, "地址不能为空".into()));
            cx.notify();
            return;
        }
        config::save_server(&url);
        self.server_url = url;
        self.notify("服务器地址已保存,重新登录后生效".into(), cx);
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Header: mono label + back to workspace.
                div()
                    .h(px(tokens::TOOLBAR_H))
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(mono_label("SETTINGS").text_color(theme.muted_foreground))
                    .child(
                        Button::new("settings-back")
                            .rounded(px(tokens::RADIUS))
                            .label("← 返回工作台")
                            .tooltip("返回工作台 (Esc)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.screen = Screen::Workspace;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .p_6()
                    .child(
                        div()
                            .w(px(560.0))
                            .v_flex()
                            .gap_4()
                            .child(mono_label("服务地址").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&self.settings_server_input).w_full()),
                                    )
                                    .child(
                                        Button::new("settings-test")
                                            .rounded(px(tokens::RADIUS))
                                            .label("测试连接")
                                            .tooltip("检查服务器是否可达")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.test_connection(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("settings-save")
                                            .primary()
                                            .rounded(px(tokens::RADIUS))
                                            .label("保存")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.save_server_settings(cx)
                                            })),
                                    ),
                            )
                            .child(if let Some((ok, msg)) = &self.settings_test {
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(if *ok { theme.success_foreground } else { theme.danger })
                                    .child(msg.clone())
                            } else {
                                div()
                            })
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("当前用户").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(self.username.clone()),
                            )
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("主题").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(if cx.theme().is_dark() { "深色" } else { "浅色" }),
                            )
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("布局").text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "项目侧栏 {}px · 文档树 {}px · 历史面板 {}px",
                                        self.layout.projects_rail as i32,
                                        self.layout.doc_rail as i32,
                                        self.layout.history as i32,
                                    )),
                            ),
                    ),
            )
    }
}

impl Render for XWikiApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ⌘K / ⌘P / ⌘⇧T dispatch here; bindings live on the root element so
        // they register during paint (Window::on_action requires it).
        let palette_weak = cx.weak_entity();
        let quick_weak = cx.weak_entity();
        let theme_weak = cx.weak_entity();
        let ctx_weak = cx.weak_entity();
        let proj_weak = cx.weak_entity();
        let edit_weak = cx.weak_entity();

        // Plan §3.1 keyboard model — Esc closes the innermost layer first.
        // Dialogs own their own Escape binding (gpui-component), so while a
        // dialog is open its CancelDialog consumes the key before this fires.
        let title = self.window_title();
        if title != self.last_title {
            window.set_window_title(&title);
            self.last_title = title;
        }
        div()
            .id("app-root")
            .size_full()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key != "escape" {
                    return;
                }
                if this.quick_open {
                    this.quick_open = false;
                    cx.notify();
                } else if this.editing {
                    this.cancel_edit(cx);
                } else if this.history_open {
                    this.close_history(cx);
                } else if this.screen_is_workspace() && this.selected_project.is_some() {
                    this.back_to_projects(cx);
                } else if matches!(this.screen, Screen::Settings) {
                    this.screen = Screen::Workspace;
                    cx.notify();
                }
            }))
            .on_action(move |_: &TogglePalette, window, cx| {
                let _ = palette_weak.update(cx, |app, cx| app.toggle_palette(window, cx));
            })
            .on_action(move |_: &QuickOpen, window, cx| {
                let _ = quick_weak.update(cx, |app, cx| app.toggle_quick_open(window, cx));
            })
            .on_action(move |_: &ToggleTheme, _window, cx| {
                let _ = theme_weak.update(cx, |app, cx| app.toggle_theme(cx));
            })
            .on_action(move |action: &TreeContextAction, _window, cx| {
                let _ = ctx_weak.update(cx, |app, cx| {
                    if action.is_dir {
                        app.load_tree(&action.path, cx);
                    } else {
                        app.open_doc(&action.path, cx);
                    }
                });
            })
            .on_action(move |action: &ProjectContextAction, _window, cx| {
                let _ = proj_weak.update(cx, |app, cx| {
                    app.open_project(&action.project_id, cx);
                });
            })
            .on_action(move |action: &EditDocAction, _window, cx| {
                let _ = edit_weak.update(cx, |app, cx| {
                    app.pending_edit = Some(action.path.clone());
                    app.open_doc(&action.path, cx);
                });
            })
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .relative()
                    .child(match self.screen {
                        Screen::Login => self.render_login(cx).into_any_element(),
                        Screen::Settings => self.render_settings(cx).into_any_element(),
                        Screen::Workspace => {
                            if self.selected_project.is_some() {
                                self.render_doc_view(window, cx).into_any_element()
                            } else {
                                self.render_workspace(window, cx).into_any_element()
                            }
                        }
                    })
                    .child(if self.quick_open {
                        self.render_quick_open(cx).into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
    }
}

fn open_new_project_dialog_window(
    window: &mut Window,
    cx: &mut App,
    client: Option<Client>,
    handle: Entity<XWikiApp>,
) {
    let Some(client) = client else {
        return;
    };
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
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("名称"),
                            )
                            .child(Input::new(&content_name).w_full())
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("描述"),
                            )
                            .child(Input::new(&content_desc).w_full()),
                    )
            };

            let cancel = Button::new("cancel-project")
                .rounded(px(tokens::RADIUS))
                .label("取消")
                .on_click(move |_, window, cx| window.close_dialog(cx));

            let create_name = name_state.clone();
            let create_desc = desc_state.clone();
            let create_client = client.clone();
            let app_handle = handle.clone();
            let create = Button::new("create-project")
                .primary()
                .rounded(px(tokens::RADIUS))
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
                                h.update(cx, |app, cx| {
                                    app.projects
                                        .write()
                                        .unwrap()
                                        .push(ProjectRow::from_dto(&p));
                                    cx.notify();
                                });
                            }
                            Err(e) => {
                                h.update(cx, |app, cx| {
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
