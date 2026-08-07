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
    notification::Notification,
    *,
};

use crate::api::{Client, dto};
use crate::config;
pub mod views;
use crate::ui::{mono_label, tokens};
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
                .icon(IconName::Close)
                .label("取消")
                .on_click(move |_, window, cx| window.close_dialog(cx));

            let create_name = name_state.clone();
            let create_desc = desc_state.clone();
            let create_client = client.clone();
            let app_handle = handle.clone();
            let create = Button::new("create-project")
                .primary()
                .rounded(px(tokens::RADIUS))
                .icon(IconName::Folder)
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
