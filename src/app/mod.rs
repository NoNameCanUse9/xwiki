use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::io::Read;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use gpui::StatefulInteractiveElement;
use gpui::*;
use guise::prelude::*;
use guise::theme::{Size, theme};

use crate::api::{Client, dto};
use crate::config;
mod outline;
pub mod views;
use crate::ui::{mono_label, tokens};
use crate::{QuickOpen, SaveEditor, TogglePalette, ToggleTheme};

const MAX_IMPORT_FILE_BYTES: usize = 5 * 1024 * 1024;
const MAX_IMPORT_TOTAL_BYTES: usize = 128 * 1024 * 1024;
const MAX_IMPORT_FILES: usize = 10_000;
const MAX_IMPORT_DIRECTORIES: usize = 100_000;
const HISTORY_PAGE_SIZE: u32 = 20;

const OTA_GITHUB_OWNER: &str = "NoNameCanUse9";
const OTA_GITHUB_REPO: &str = "xwiki";

#[derive(Clone, Default)]
struct TokenDraft {
    write: bool,
    projects: BTreeSet<String>,
}

impl TokenDraft {
    fn scope(&self) -> &'static str {
        if self.write { "write" } else { "read" }
    }

    fn set_scope(&mut self, scope: &str) {
        match scope {
            "read" => self.write = false,
            "write" => self.write = true,
            _ => {}
        }
    }

    fn set_project(&mut self, project_id: &str, selected: bool) {
        if selected {
            self.projects.insert(project_id.to_string());
        } else {
            self.projects.remove(project_id);
        }
    }

    fn projects(&self) -> Vec<String> {
        self.projects.iter().cloned().collect()
    }
}

fn history_page_offset(reset: bool, loaded: usize) -> u32 {
    if reset { 0 } else { loaded as u32 }
}

fn merge_history_page(
    commits: &mut Vec<dto::Commit>,
    page: dto::CommitListResponse,
    reset: bool,
) -> bool {
    if reset {
        *commits = page.commits;
    } else {
        commits.extend(page.commits);
    }
    page.has_more
}

fn draft_base_revision(server_revision: &str, draft_revision: &str) -> String {
    if draft_revision.is_empty() {
        server_revision.to_string()
    } else {
        draft_revision.to_string()
    }
}

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
    BackToDocumentBrowser,
    BackToProjects,
}

/// Right-click menu action for a doc-tree row.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct TreeContextAction {
    pub path: String,
    pub is_dir: bool,
}

/// Export the current project from a document-row overflow menu.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct ProjectExportAction;

/// Right-click menu action for a project card / rail row.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct ProjectContextAction {
    pub project_id: String,
}

/// Right-click archive/unarchive toggle for a project card.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct ProjectArchiveAction {
    pub project_id: String,
    pub archived: bool,
}

/// Context-menu rename for a project card.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct ProjectRenameAction {
    pub project_id: String,
    pub current_name: String,
}

/// Context-menu delete for a project card.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct ProjectDeleteAction {
    pub project_id: String,
    pub project_name: String,
}

/// Right-click "edit" action: loads the doc, then acquires the lock.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct EditDocAction {
    pub path: String,
}

/// Right-click rename for a doc or directory in the tree.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct DocRenameAction {
    pub path: String,
    pub is_dir: bool,
}

/// Right-click delete for a doc or directory in the tree.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = app_actions, no_json)]
pub(crate) struct DocDeleteAction {
    pub path: String,
    pub is_dir: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ProjectFilter {
    All,
    Active,
    Archived,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocPanel {
    None,
    Share,
}

#[derive(Clone, Copy)]
enum DocumentImportMode {
    Folder,
    Markdown,
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
    /// Show the initial project skeleton only when loading lasts long enough to
    /// be useful; fast local responses should not flash an empty placeholder.
    project_skeleton_visible: bool,
    /// Forgot-password flow: true swaps the login panel for the reset form.
    reset_mode: bool,
    reset_status: Option<(bool, String)>,
    reset_token_input: Entity<TextInput>,
    reset_password_input: Entity<TextInput>,
    meta_version: Option<String>,
    server_input: Entity<TextInput>,
    user_input: Entity<TextInput>,
    password_input: Entity<TextInput>,
    projects: Vec<ProjectRow>,
    filter_input: Entity<TextInput>,
    project_filter: ProjectFilter,
    // Document workspace state.
    selected_project: Option<String>,
    tree_entries: Vec<dto::TreeEntry>,
    tree_path: String,
    tree_loading: bool,
    doc_path: Option<String>,
    doc_content: String,
    doc_loading: bool,
    doc_outline: outline::ParsedDocument,
    doc_scroll: ScrollHandle,
    active_outline: Option<usize>,
    /// Contextual API-backed panels for the current document.
    doc_panel: DocPanel,
    share_loading: bool,
    share_url: Option<String>,
    share_error: Option<String>,
    backlinks_loading: bool,
    backlinks_error: Option<String>,
    backlinks: Vec<dto::Backlink>,
    attachments_loading: bool,
    attachments_error: Option<String>,
    attachments: Vec<dto::TreeEntry>,
    attachment_source_input: Entity<TextInput>,
    attachment_destination_input: Entity<TextInput>,
    // Edit state: page lock + markdown editor + commit message.
    editing: bool,
    edit_path: Option<String>,
    lock_held: bool,
    /// Heartbeat generation counter: each `start_heartbeat` takes a fresh
    /// generation and every loop iteration bails when the counter moved on,
    /// so a cancelled edit can never resurrect a stale loop for an old path.
    heartbeat_generation: Arc<AtomicU64>,
    status_msg: Option<String>,
    save_error: Option<String>,
    saving: bool,
    commit_msg: Entity<TextInput>,
    editor_input: Entity<Editor>,
    editor_preview: bool,
    // History view state.
    history_open: bool,
    history_file_path: Option<String>,
    history_input: Entity<TextInput>,
    history_loading: bool,
    history_has_more: bool,
    history_generation: u64,
    history_detail_loading: bool,
    history_error: Option<String>,
    history_focus: Option<usize>,
    restoring: bool,
    commits: Vec<dto::Commit>,
    commit_detail: Option<dto::CommitDetail>,
    diff_stats: Vec<dto::DiffStat>,
    /// Unified-diff patch for the selected commit (`format=patch`).
    commit_patch: Option<String>,
    /// Whether the patch/diff panel is expanded (Compare toggle).
    history_compare_open: bool,
    /// Selected commit sha (highlight in the history list).
    selected_sha: Option<String>,
    // Project-root change roadmap.
    project_commits: Vec<dto::Commit>,
    /// Horizontal scroll position for the project roadmap node strip.
    project_changes_scroll: ScrollHandle,
    project_changes_loading: bool,
    project_changes_error: Option<String>,
    project_changes_generation: u64,
    /// Panel layout (widths + history visibility), persisted via config.
    layout: config::Layout,
    /// Server URL as resolved at login (status bar).
    server_url: String,
    /// Current project revision shown in the persistent status area.
    current_revision: Option<String>,
    /// Revision captured when the current editor session loaded its content.
    edit_base_revision: Option<String>,
    /// Tree keyboard-navigation cursor (index into visible entries).
    tree_focus: Option<usize>,
    /// Save-time revision conflict (render_main_pane shows the recovery panel).
    conflict: Option<ConflictInfo>,
    /// Transient load errors with retry (per-area, not the login banner).
    tree_error: Option<String>,
    doc_error: Option<String>,
    projects_error: Option<String>,
    project_action: Option<String>,
    /// Settings view: server URL input + connection-test result.
    settings_server_input: Entity<TextInput>,
    settings_test: Option<(bool, String)>,
    settings_test_detail: Option<String>,
    settings_loading: bool,
    /// GitHub Releases check state for the settings update section.
    settings_ota_loading: bool,
    settings_ota_status: Option<(bool, String)>,
    settings_error: Option<String>,
    settings_tokens: Vec<dto::Token>,
    settings_users: Vec<dto::User>,
    settings_token_secret: Option<String>,
    settings_access_loading: bool,
    /// ⌘P quick-open overlay state.
    quick_open: bool,
    quick_input: Entity<TextInput>,
    /// Project-wide full-text search overlay state.
    search_open: bool,
    search_input: Entity<TextInput>,
    search_results: Vec<dto::SearchResult>,
    search_loading: bool,
    search_error: Option<String>,
    /// Audit log page (project picker + entries, mirrors the web audit page).
    audit_entries: Vec<dto::AuditEntry>,
    audit_loading: bool,
    audit_error: Option<String>,
    audit_projects: Vec<dto::Project>,
    audit_selected_project: Option<String>,
    /// OpenAPI reference is rendered as a native read-only reference page.
    api_reference_loading: bool,
    api_reference_error: Option<String>,
    api_reference: Option<serde_json::Value>,
    api_reference_selected_path: Option<String>,
    api_reference_selected_method: Option<String>,
    /// Edit requested from a context menu while the doc is still loading.
    pending_edit: Option<String>,
    /// Cached window title; `set_window_title` only on change.
    last_title: String,
    /// Keep input subscriptions alive with the app entity.
    _subscriptions: Vec<Subscription>,
    /// Window-level modal stack + toast queue (guise), mounted as the last
    /// child of the root view.
    overlay_host: Entity<OverlayHost>,
}

enum Screen {
    Login,
    Workspace,
    Settings,
    ApiReference,
    Audit,
    /// 文档分析（反向链接）独立页：从文档页"更多"菜单进入。
    Backlinks,
    /// 附件管理独立页：从文档页"更多"菜单进入。
    Attachments,
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
        let server_input = cx.new(|cx| TextInput::new(cx).placeholder(config::load_server()));
        let saved_username = config::load_username();
        let user_input = cx.new(|cx| {
            TextInput::new(cx)
                .placeholder("用户名")
                .value(&saved_username)
        });
        let password_input = cx.new(|cx| TextInput::new(cx).placeholder("密码").password(true));
        let reset_token_input = cx.new(|cx| TextInput::new(cx).placeholder("一次性 token"));
        let reset_password_input = cx.new(|cx| {
            TextInput::new(cx)
                .placeholder("新密码（至少 8 位）")
                .password(true)
        });

        // ponytail: rows are loaded from GET /api/v1/projects on login; this
        // starts empty. Plain Vec — every access happens on the main thread
        // inside update callbacks, so no lock is needed.
        // Match the compact action buttons in the workspace toolbar.
        let filter_input =
            cx.new(|cx| TextInput::new(cx).size(Size::Xs).placeholder("搜索项目…"));

        let commit_msg = cx.new(|cx| TextInput::new(cx).placeholder("提交消息…"));
        let editor_input = cx.new(|cx| {
            Editor::new(cx)
                .line_numbers(false)
                .font_size(14.0)
                .placeholder("# 用 Markdown 写作…")
        });
        let history_input = cx.new(|cx| TextInput::new(cx).placeholder("搜索版本…"));
        let attachment_source_input =
            cx.new(|cx| TextInput::new(cx).placeholder("本地文件路径（≤ 5 MiB）"));
        let attachment_destination_input =
            cx.new(|cx| TextInput::new(cx).placeholder("下载目标路径"));

        let settings_server_input = cx.new(|cx| TextInput::new(cx).value(&config::load_server()));
        let quick_input = cx.new(|cx| TextInput::new(cx).placeholder("项目 / 文档…"));
        let search_input = cx.new(|cx| TextInput::new(cx).placeholder("搜索文档内容…"));
        let mut subs = Vec::new();
        for state in [&server_input, &user_input, &password_input] {
            subs.push(cx.subscribe_in(
                state,
                window,
                |app, _entity, event: &TextInputEvent, window, cx| {
                    if matches!(event, TextInputEvent::Submit(_)) && !app.reset_mode {
                        app.do_login(window, cx);
                    } else {
                        cx.notify();
                    }
                },
            ));
        }
        // Filter keystrokes re-render the project cards.
        {
            let filter = filter_input.clone();
            subs.push(
                cx.subscribe(&filter_input, move |_, _entity, _: &TextInputEvent, cx| {
                    let _ = filter.read(cx);
                    cx.notify();
                }),
            );
        }
        // ⌘P quick-open keystrokes re-render the overlay list.
        {
            let quick = quick_input.clone();
            subs.push(
                cx.subscribe(&quick_input, move |_, _entity, _: &TextInputEvent, cx| {
                    let _ = quick.read(cx);
                    cx.notify();
                }),
            );
        }
        // Project search keystrokes re-render the results and trigger a live
        // search once a query is present.
        {
            let search = search_input.clone();
            subs.push(cx.subscribe(
                &search_input,
                move |app, _entity, _: &TextInputEvent, cx| {
                    let q = search.read(cx).text().trim().to_string();
                    if q.is_empty() {
                        app.search_results.clear();
                        app.search_error = None;
                    } else if !app.search_loading {
                        app.run_project_search(cx);
                    } else {
                        cx.notify();
                    }
                },
            ));
        }
        // Preview mode renders the current editor buffer, so changes need to
        // invalidate the app even while the editor is open.
        {
            let editor = editor_input.clone();
            subs.push(
                cx.subscribe(&editor_input, move |app, _entity, event: &EditorEvent, cx| {
                    if matches!(event, EditorEvent::Change(_)) {
                        let _ = editor.read(cx);
                        if app.editing {
                            app.persist_draft(cx);
                        }
                        cx.notify();
                    }
                }),
            );
        }
        // History search is server-backed so it covers every ref and commit,
        // not only the currently loaded page.
        {
            let history = history_input.clone();
            subs.push(cx.subscribe(
                &history_input,
                move |app, _entity, _: &TextInputEvent, cx| {
                    let _ = history.read(cx);
                    if app.history_open {
                        app.load_history_page(true, cx);
                    } else {
                        cx.notify();
                    }
                },
            ));
        }
        // Commit-message edits update dirty-state and local drafts.
        {
            let commit = commit_msg.clone();
            subs.push(
                cx.subscribe(&commit_msg, move |app, _entity, _: &TextInputEvent, cx| {
                    let _ = commit.read(cx);
                    if app.editing {
                        app.persist_draft(cx);
                    }
                    cx.notify();
                }),
            );
        }
        // Changing the server address invalidates the previous connection result.
        {
            let settings_input = settings_server_input.clone();
            subs.push(cx.subscribe(
                &settings_server_input,
                move |app, _entity, _: &TextInputEvent, cx| {
                    let _ = settings_input.read(cx);
                    app.settings_test = None;
                    app.settings_test_detail = None;
                    app.settings_ota_status = None;
                    cx.notify();
                },
            ));
        }
        // A window close bypasses the editor's Cancel button. Stop the lease
        // loop and make the same best-effort release request before the root
        // entity disappears. Use a weak handle to avoid a subscription cycle.
        let app_handle = cx.entity().downgrade();
        subs.push(cx.on_window_closed(move |cx| {
            if let Some(app_handle) = app_handle.upgrade() {
                app_handle.update(cx, |app, cx| {
                    app.stop_heartbeat();
                    app.release_current_lock(cx);
                    app.editing = false;
                    app.saving = false;
                    app.lock_held = false;
                    app.edit_base_revision = None;
                    app.edit_path = None;
                });
            }
        }));

        let mut app = Self {
            screen: Screen::Login,
            client: None,
            username: String::new(),
            login_error: None,
            loading: false,
            project_skeleton_visible: false,
            reset_mode: false,
            reset_status: None,
            reset_token_input,
            reset_password_input,
            meta_version: None,
            server_input,
            user_input,
            password_input,
            projects: Vec::new(),
            filter_input,
            project_filter: ProjectFilter::All,
            selected_project: None,
            tree_entries: Vec::new(),
            tree_path: String::new(),
            tree_loading: false,
            doc_path: None,
            doc_content: String::new(),
            doc_loading: false,
            doc_outline: outline::ParsedDocument {
                entries: Vec::new(),
                sections: Vec::new(),
            },
            doc_scroll: ScrollHandle::new(),
            active_outline: None,
            doc_panel: DocPanel::None,
            share_loading: false,
            share_url: None,
            share_error: None,
            backlinks_loading: false,
            backlinks_error: None,
            backlinks: Vec::new(),
            attachments_loading: false,
            attachments_error: None,
            attachments: Vec::new(),
            attachment_source_input,
            attachment_destination_input,
            editing: false,
            edit_path: None,
            lock_held: false,
            heartbeat_generation: Arc::new(AtomicU64::new(0)),
            status_msg: None,
            save_error: None,
            saving: false,
            commit_msg,
            editor_input,
            editor_preview: false,
            history_open: false,
            history_file_path: None,
            history_input,
            history_loading: false,
            history_has_more: false,
            history_generation: 0,
            history_detail_loading: false,
            history_error: None,
            history_focus: None,
            restoring: false,
            commits: Vec::new(),
            commit_detail: None,
            diff_stats: Vec::new(),
            commit_patch: None,
            history_compare_open: false,
            selected_sha: None,
            project_commits: Vec::new(),
            project_changes_scroll: ScrollHandle::new(),
            project_changes_loading: false,
            project_changes_error: None,
            project_changes_generation: 0,
            layout: config::load_layout(),
            server_url: config::load_server(),
            current_revision: None,
            edit_base_revision: None,
            tree_focus: None,
            conflict: None,
            tree_error: None,
            doc_error: None,
            projects_error: None,
            project_action: None,
            settings_server_input,
            settings_test: None,
            settings_test_detail: None,
            settings_loading: false,
            settings_ota_loading: false,
            settings_ota_status: None,
            settings_error: None,
            settings_tokens: Vec::new(),
            settings_users: Vec::new(),
            settings_token_secret: None,
            settings_access_loading: false,
            quick_open: false,
            quick_input,
            search_open: false,
            search_input,
            search_results: Vec::new(),
            search_loading: false,
            search_error: None,
            audit_entries: Vec::new(),
            audit_loading: false,
            audit_error: None,
            audit_projects: Vec::new(),
            audit_selected_project: None,
            api_reference_loading: false,
            api_reference_error: None,
            api_reference: None,
            api_reference_selected_path: None,
            api_reference_selected_method: None,
            pending_edit: None,
            last_title: String::new(),
            _subscriptions: subs,
            overlay_host: cx.new(OverlayHost::new),
        };
        app.restore_session(window, cx);
        app
    }

    fn restore_session(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = config::load_session() else {
            return;
        };
        let server = session.server.clone();
        let username = session.username.clone();
        let client = Client::with_session(&server, Some(session.cookie));
        let _ = config::save_server(&server);
        self.server_input.update(cx, |state, cx| {
            state.set_text(&server, cx);
        });
        self.user_input.update(cx, |state, cx| {
            state.set_text(&username, cx);
        });
        self.server_url = server;
        self.username = username;
        self.client = Some(client.clone());
        self.loading = true;
        cx.spawn(async move |this, cx| {
            let result = match client.meta().await {
                Ok(meta) if meta.api_version == "1" => client.me().await,
                Ok(meta) => Err(crate::api::ApiError {
                    code: "unsupported_api_version".into(),
                    message: format!(
                        "服务器 API 版本 {} 不受支持，请升级客户端",
                        meta.api_version
                    ),
                    request_id: None,
                    status: 400,
                }),
                Err(error) => Err(error),
            };
            match result {
                Ok(user) => {
                    let _ = this.update(cx, |app, cx| app.on_login_ok(user, cx));
                }
                Err(_) => {
                    config::clear_session();
                    let _ = this.update(cx, |app, cx| {
                        app.client = None;
                        app.username.clear();
                        app.loading = false;
                        app.login_error = Some("保存的会话已失效，请重新登录。".into());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn open_project(&mut self, project_id: &str, cx: &mut Context<Self>) {
        if self.editing {
            return;
        }
        self.release_current_lock(cx);
        self.stop_heartbeat();
        self.edit_path = None;
        self.lock_held = false;
        self.reset_project_changes();
        self.selected_project = Some(project_id.to_string());
        self.selected_sha = None;
        self.current_revision = None;
        self.edit_base_revision = None;
        self.tree_path.clear();
        self.doc_path = None;
        self.doc_content.clear();
        self.doc_outline = outline::ParsedDocument {
            entries: Vec::new(),
            sections: Vec::new(),
        };
        self.doc_scroll = ScrollHandle::new();
        self.active_outline = None;
        self.load_tree("", cx);
        self.load_revision(cx);
    }

    fn load_revision(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        cx.spawn(
            async move |this, cx| match client.revision(&project).await {
                Ok(revision) => {
                    let _ = this.update(cx, |app, cx| {
                        // Discard stale responses: the user may have switched
                        // projects while this request was in flight.
                        if app.selected_project.as_deref() != Some(project.as_str()) {
                            return;
                        }
                        app.current_revision = Some(revision);
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        if app.selected_project.as_deref() != Some(project.as_str()) {
                            return;
                        }
                        app.status_msg = Some(format!("读取 revision 失败: {e}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    fn load_tree(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some(project) = self.selected_project.clone() else {
            return;
        };
        self.history_open = false;
        self.history_file_path = None;
        self.history_loading = false;
        self.history_detail_loading = false;
        self.restoring = false;
        self.clear_history_data();
        self.tree_path = path.to_string();
        self.tree_focus = None;
        self.doc_path = None;
        self.doc_content.clear();
        self.doc_outline = outline::ParsedDocument {
            entries: Vec::new(),
            sections: Vec::new(),
        };
        self.doc_scroll = ScrollHandle::new();
        self.active_outline = None;
        self.doc_loading = false;
        self.tree_loading = true;
        self.tree_error = None;
        if path.is_empty() {
            self.load_project_changes(cx);
        }
        let path = path.to_string();
        cx.spawn(
            async move |this, cx| match client.tree(&project, &path).await {
                Ok(entries) => {
                    let _ = this.update(cx, |app, cx| {
                        // Discard stale responses: the user may have switched
                        // projects or directories while this was in flight.
                        if app.selected_project.as_deref() != Some(project.as_str())
                            || app.tree_path != path
                        {
                            return;
                        }
                        app.tree_loading = false;
                        app.tree_entries = entries;
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        if app.selected_project.as_deref() != Some(project.as_str())
                            || app.tree_path != path
                        {
                            return;
                        }
                        app.tree_loading = false;
                        app.tree_error = Some(Self::friendly_api_error(&e));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    fn reset_project_changes(&mut self) {
        self.project_changes_generation = self.project_changes_generation.wrapping_add(1);
        self.project_commits.clear();
        self.project_changes_scroll = ScrollHandle::new();
        self.project_changes_loading = false;
        self.project_changes_error = None;
    }

    fn load_project_changes(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        if self.project_changes_loading {
            return;
        }
        self.reset_project_changes();
        let offset = 0;
        let generation = self.project_changes_generation;
        self.project_changes_loading = true;
        self.project_changes_error = None;
        cx.spawn(async move |this, cx| {
            let result = client.commits_page(&project, 20, offset).await;
            let _ = this.update(cx, |app, cx| {
                if app.selected_project.as_deref() != Some(project.as_str())
                    || app.project_commits.len() as u32 != offset
                    || app.project_changes_generation != generation
                {
                    return;
                }
                app.project_changes_loading = false;
                match result {
                    Ok(commits) => {
                        app.project_commits.extend(commits);
                    }
                    Err(error) => {
                        app.project_changes_error = Some(format!(
                            "加载项目变更失败: {}",
                            Self::friendly_api_error(&error)
                        ));
                    }
                }
                cx.notify();
            });
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
        self.history_open = false;
        self.history_file_path = None;
        self.doc_path = Some(path.to_string());
        self.doc_loading = true;
        self.doc_error = None;
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            match client.page(&project, &path).await {
                Ok(page) => {
                    let _ = this.update(cx, |app, cx| {
                        // Discard stale responses: the user may have opened a
                        // different doc (or switched projects) while this
                        // request was in flight. Writing stale content here
                        // would silently corrupt the wrong document on save.
                        if app.selected_project.as_deref() != Some(project.as_str())
                            || app.doc_path.as_deref() != Some(path.as_str())
                        {
                            return;
                        }
                        app.doc_content = page.content;
                        app.current_revision = Some(page.revision.clone());
                        if !app.editing {
                            app.edit_base_revision = Some(page.revision);
                        }
                        app.doc_outline = outline::parse_document(&app.doc_content);
                        app.doc_scroll = ScrollHandle::new();
                        app.active_outline = app.doc_outline.entries.first().map(|_| 0);
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
                        if app.selected_project.as_deref() != Some(project.as_str())
                            || app.doc_path.as_deref() != Some(path.as_str())
                        {
                            return;
                        }
                        app.doc_loading = false;
                        app.doc_error = Some(Self::friendly_api_error(&e));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    // ----- Document API panels: share, backlinks, attachments, file history -----

    pub(crate) fn open_share_panel(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project), Some(path)) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.doc_path.clone(),
        ) else {
            return;
        };
        self.doc_panel = DocPanel::Share;
        self.share_loading = true;
        self.share_error = None;
        cx.notify();
        let server = self.server_url.clone();
        cx.spawn(
            async move |this, cx| match client.create_share(&project, &path).await {
                Ok(share) => {
                    let url =
                        if share.url.starts_with("http://") || share.url.starts_with("https://") {
                            share.url
                        } else {
                            format!("{}{}", server.trim_end_matches('/'), share.url)
                        };
                    let _ = this.update(cx, |app, cx| {
                        app.share_loading = false;
                        app.share_url = Some(url);
                        app.share_error = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.share_loading = false;
                        app.share_error = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    pub(crate) fn copy_share_url(&mut self, cx: &mut Context<Self>) {
        if let Some(url) = self.share_url.clone() {
            cx.write_to_clipboard(ClipboardItem::new_string(url));
            self.notify("完整分享 URL 已复制".into(), cx);
        }
    }

    pub(crate) fn open_backlinks_panel(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project), Some(path)) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.doc_path.clone(),
        ) else {
            return;
        };
        self.doc_panel = DocPanel::None;
        self.screen = Screen::Backlinks;
        self.backlinks_loading = true;
        self.backlinks_error = None;
        cx.notify();
        cx.spawn(
            async move |this, cx| match client.backlinks(&project, &path).await {
                Ok(items) => {
                    let _ = this.update(cx, |app, cx| {
                        app.backlinks_loading = false;
                        app.backlinks = items;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.backlinks_loading = false;
                        app.backlinks_error = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    pub(crate) fn open_attachments_panel(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        self.doc_panel = DocPanel::None;
        self.screen = Screen::Attachments;
        self.attachments_loading = true;
        self.attachments_error = None;
        cx.notify();
        cx.spawn(
            async move |this, cx| match client.tree(&project, "attachments").await {
                Ok(items) => {
                    let _ = this.update(cx, |app, cx| {
                        app.attachments_loading = false;
                        app.attachments =
                            items.into_iter().filter(|e| e.r#type == "blob").collect();
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.attachments_loading = false;
                        app.attachments_error = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    pub(crate) fn open_file_history_panel(&mut self, cx: &mut Context<Self>) {
        if self.selected_project.is_none() || self.doc_path.is_none() {
            return;
        }
        self.doc_panel = DocPanel::None;
        self.history_open = true;
        self.history_file_path = self.doc_path.clone();
        self.history_loading = true;
        self.history_detail_loading = false;
        self.history_error = None;
        self.history_focus = None;
        self.commits.clear();
        self.commit_detail = None;
        self.diff_stats.clear();
        self.commit_patch = None;
        self.history_compare_open = false;
        self.selected_sha = None;
        config::save_layout(&self.layout);
        cx.notify();
        self.load_file_history(cx);
    }

    pub(crate) fn close_doc_panel(&mut self, cx: &mut Context<Self>) {
        self.doc_panel = DocPanel::None;
        cx.notify();
    }

    pub(crate) fn upload_attachment(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let source = self
            .attachment_source_input
            .read(cx)
            .text()
            .trim()
            .to_string();
        if source.is_empty() {
            self.attachments_error = Some("请输入本地文件路径。".into());
            cx.notify();
            return;
        }
        let path = std::path::PathBuf::from(&source);
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("attachment")
            .to_string();
        let content = match read_file_limited(&path, MAX_IMPORT_FILE_BYTES) {
            Ok(content) => content,
            Err(error) => {
                self.attachments_error = Some(format!("读取附件失败: {error}"));
                cx.notify();
                return;
            }
        };
        self.attachments_loading = true;
        self.attachments_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = async {
                let revision = client.revision(&project).await?;
                let encoded = BASE64.encode(content);
                client
                    .upload_attachment(
                        &project,
                        &revision,
                        &format!("attachments/{filename}"),
                        &encoded,
                    )
                    .await
            }
            .await;
            match result {
                Ok(_) => {
                    let _ = this.update(cx, |app, cx| {
                        app.attachments_loading = false;
                        app.notify(format!("附件 {filename} 已上传"), cx);
                        app.open_attachments_panel(cx);
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.attachments_loading = false;
                        app.attachments_error = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub(crate) fn download_attachment(&mut self, path: &str, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let destination = self
            .attachment_destination_input
            .read(cx)
            .text()
            .trim()
            .to_string();
        let fallback = path.rsplit('/').next().unwrap_or("attachment").to_string();
        let destination = if destination.is_empty() {
            fallback.clone()
        } else {
            destination
        };
        self.attachments_loading = true;
        self.attachments_error = None;
        cx.notify();
        let path = path.to_string();
        cx.spawn(
            async move |this, cx| match client.download_attachment(&project, &path).await {
                Ok(bytes) => {
                    let result = std::fs::write(&destination, bytes);
                    let _ = this.update(cx, |app, cx| {
                        app.attachments_loading = false;
                        match result {
                            Ok(_) => app.notify(format!("附件已保存到 {destination}"), cx),
                            Err(error) => {
                                app.attachments_error = Some(format!("保存附件失败: {error}"));
                                cx.notify();
                            }
                        }
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.attachments_loading = false;
                        app.attachments_error = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    pub(crate) fn confirm_delete_attachment(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        path: String,
    ) {
        let handle = cx.entity();
        let target = handle.clone();
        let delete_path = path.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let target = target.clone();
                let delete_path = delete_path.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("删除附件？")
                    .message("删除会创建一个新的文档提交。")
                    .confirm_label("删除")
                    .cancel_label("取消")
                    .danger()
                    .on_confirm(move |_, window, cx| {
                        let path = delete_path.clone();
                        target.update(cx, |app, cx| app.delete_attachment(&path, cx));
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    fn delete_attachment(&mut self, path: &str, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        self.attachments_loading = true;
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            let result = async {
                let revision = client.revision(&project).await?;
                client.delete_attachment(&project, &revision, &path).await
            }
            .await;
            match result {
                Ok(_) => {
                    let _ = this.update(cx, |app, cx| {
                        app.notify("附件已删除".into(), cx);
                        app.open_attachments_panel(cx);
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.attachments_loading = false;
                        app.attachments_error = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub(crate) fn open_api_reference(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.screen = Screen::ApiReference;
        self.api_reference_loading = true;
        self.api_reference_error = None;
        self.api_reference_selected_path = None;
        self.api_reference_selected_method = None;
        cx.notify();
        cx.spawn(async move |this, cx| match client.openapi().await {
            Ok(spec) => {
                let _ = this.update(cx, |app, cx| {
                    app.api_reference_loading = false;
                    // Keep the parsed Value: rendering parses the spec on
                    // every frame, so a raw string would be re-parsed each
                    // time (the JSON copy is produced on demand instead).
                    app.api_reference = Some(spec);
                    cx.notify();
                });
            }
            Err(error) => {
                let _ = this.update(cx, |app, cx| {
                    app.api_reference_loading = false;
                    app.api_reference_error = Some(Self::friendly_api_error(&error));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn back_to_projects(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            return;
        }
        self.release_current_lock(cx);
        self.stop_heartbeat();
        self.edit_path = None;
        self.lock_held = false;
        self.reset_project_changes();
        self.selected_project = None;
        self.current_revision = None;
        self.tree_entries.clear();
        self.tree_path.clear();
        self.tree_loading = false;
        self.doc_path = None;
        self.doc_content.clear();
        self.doc_outline = outline::ParsedDocument {
            entries: Vec::new(),
            sections: Vec::new(),
        };
        self.doc_scroll = ScrollHandle::new();
        self.active_outline = None;
        self.doc_panel = DocPanel::None;
        self.share_url = None;
        self.backlinks.clear();
        self.attachments.clear();
        self.history_open = false;
        self.history_file_path = None;
        self.history_loading = false;
        self.history_detail_loading = false;
        self.restoring = false;
        self.clear_history_data();
        cx.notify();
    }

    fn back_to_document_browser(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            return;
        }
        self.release_current_lock(cx);
        self.stop_heartbeat();
        self.edit_path = None;
        self.lock_held = false;
        let path = self.tree_path.clone();
        self.doc_path = None;
        self.doc_content.clear();
        self.doc_outline = outline::ParsedDocument {
            entries: Vec::new(),
            sections: Vec::new(),
        };
        self.doc_scroll = ScrollHandle::new();
        self.active_outline = None;
        self.doc_panel = DocPanel::None;
        self.share_url = None;
        self.share_error = None;
        self.backlinks.clear();
        self.backlinks_error = None;
        self.attachments.clear();
        self.attachments_error = None;
        self.history_open = false;
        self.history_file_path = None;
        self.history_error = None;
        if self.tree_entries.is_empty() {
            self.load_tree(&path, cx);
        } else {
            if path.is_empty() {
                self.load_project_changes(cx);
            }
            cx.notify();
        }
    }

    // ----- Edit flow: acquire lock -> edit -> changeset commit -----

    fn start_edit(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let Some(path) = self.doc_path.clone() else {
            return;
        };
        self.status_msg = None;
        cx.spawn(
            async move |this, cx| match client.acquire_lock(&project, &path).await {
                Ok(_) => {
                    let _ = this.update(cx, |app, cx| app.begin_editing(&path, cx));
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.status_msg = Some(format!("无法编辑: {e}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    fn begin_editing(&mut self, path: &str, cx: &mut Context<Self>) {
        let mut content = self.doc_content.clone();
        let editor = self.editor_input.clone();
        let commit = self.commit_msg.clone();
        let mut message = String::new();
        if let Some(project) = self.selected_project.as_ref()
            && let Some(draft) = config::load_drafts().into_iter().find(|d| {
                d.server == self.server_url
                    && d.username == self.username
                    && d.project == *project
                    && d.original_path == path
            })
        {
            content = draft.content;
            message = draft.message;
            let server_revision = self.current_revision.as_deref().unwrap_or_default();
            let base_revision = draft_base_revision(server_revision, &draft.base_revision);
            if !base_revision.is_empty() {
                self.edit_base_revision = Some(base_revision);
            }
            self.status_msg = Some("已恢复本地草稿".into());
        }
        if let Some(handle) = cx.active_window() {
            let _ = cx.update_window(handle, |_view, _window, cx| {
                editor.update(cx, |s, cx| s.set_text(&content, cx));
                commit.update(cx, |s, cx| s.set_text(&message, cx));
            });
        }
        self.edit_path = Some(path.to_string());
        if self.edit_base_revision.is_none() {
            self.edit_base_revision = self.current_revision.clone();
        }
        self.editor_preview = false;
        self.editing = true;
        self.lock_held = true;
        self.saving = false;
        self.save_error = None;
        self.start_heartbeat(cx);
        cx.notify();
    }

    fn persist_draft(&self, cx: &Context<Self>) {
        let (Some(project), Some(path)) = (self.selected_project.as_ref(), self.edit_path.as_ref())
        else {
            return;
        };
        let content = self.editor_input.read(cx).text().to_string();
        config::upsert_draft(config::Draft {
            server: self.server_url.clone(),
            username: self.username.clone(),
            project: project.clone(),
            original_path: path.clone(),
            target_path: path.clone(),
            content,
            message: self.commit_msg.read(cx).text().to_string(),
            base_revision: self.edit_base_revision.clone().unwrap_or_default(),
            updated_at: chrono::Utc::now().timestamp(),
        });
    }

    fn start_heartbeat(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project), Some(path)) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.edit_path.clone(),
        ) else {
            return;
        };
        let generation = self.heartbeat_generation.clone();
        let gen_id = generation.fetch_add(1, Ordering::Relaxed) + 1;
        cx.spawn(async move |this, cx| {
            loop {
                if generation.load(Ordering::Relaxed) != gen_id {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_secs(25))
                    .await;
                if generation.load(Ordering::Relaxed) != gen_id {
                    break;
                }
                if client.heartbeat_lock(&project, &path).await.is_err() {
                    let _ = this.update(cx, |app, cx| {
                        // The lock is gone: stop pretending we hold it, or the
                        // status bar lies and saves keep firing with a dead lock.
                        app.lock_held = false;
                        app.status_msg = Some("锁续租失败，请取消编辑后重新编辑。".into());
                        cx.notify();
                    });
                    break;
                }
            }
        })
        .detach();
    }

    fn stop_heartbeat(&self) {
        self.heartbeat_generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Best-effort release of the current server-side document lock.
    /// Callers clear local edit state separately so the request can still use
    /// the cloned project/path after navigation or logout.
    fn release_current_lock(&self, cx: &mut Context<Self>) {
        let (Some(client), Some(project), Some(path)) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.edit_path.clone(),
        ) else {
            return;
        };
        cx.spawn(async move |_this, _cx| {
            let _ = client.release_lock(&project, &path).await;
        })
        .detach();
    }

    fn reacquire_lock(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project), Some(path)) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.edit_path.clone(),
        ) else {
            return;
        };
        self.status_msg = Some("正在重新获取文档锁…".into());
        cx.spawn(
            async move |this, cx| match client.acquire_lock(&project, &path).await {
                Ok(_) => {
                    let _ = this.update(cx, |app, cx| {
                        app.lock_held = true;
                        app.status_msg = Some("文档锁已恢复，内容仍保留在编辑器中。".into());
                        app.start_heartbeat(cx);
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.lock_held = false;
                        app.status_msg = Some(format!("无法重新获取文档锁：{error}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        self.stop_heartbeat();
        self.editing = false;
        self.edit_base_revision = None;
        self.editor_preview = false;
        self.lock_held = false;
        self.saving = false;
        self.save_error = None;
        let path = self.edit_path.clone();
        self.release_current_lock(cx);
        self.edit_path = None;
        if let (Some(project), Some(path)) = (self.selected_project.as_deref(), path.as_deref()) {
            config::remove_draft(&self.server_url, &self.username, project, path);
        }
        self.status_msg = None;
        cx.notify();
    }

    fn has_unsaved_edits(&self, cx: &Context<Self>) -> bool {
        if !self.editing {
            return false;
        }
        let content_changed = self.editor_input.read(cx).text() != self.doc_content;
        let message_changed = !self.commit_msg.read(cx).text().trim().is_empty();
        content_changed || message_changed
    }

    fn request_cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_unsaved_edits(cx) {
            self.cancel_edit(cx);
            return;
        }
        let handle = cx.entity();
        let discard_target = handle.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let discard_target = discard_target.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("放弃未保存的修改？")
                    .message("当前文档包含尚未提交的内容。继续离开会丢弃这些修改。")
                    .confirm_label("放弃修改")
                    .cancel_label("继续编辑")
                    .danger()
                    .on_confirm(move |_, window, cx| {
                        let handle = discard_target.clone();
                        handle.update(cx, |app, cx| app.cancel_edit(cx));
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        if self.saving || !self.lock_held {
            return;
        }
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let Some(path) = self.edit_path.clone() else {
            return;
        };
        let content = self.editor_input.read(cx).text().to_string();
        let msg = self.commit_msg.read(cx).text().to_string();
        if msg.trim().is_empty() {
            self.save_error = Some("需要提交消息".into());
            cx.notify();
            return;
        }
        let changes = vec![dto::Change {
            op: "update".into(),
            path: path.clone(),
            new_path: None,
            content: Some(content),
        }];
        let message = msg.trim().to_string();
        self.saving = true;
        self.save_error = None;
        self.status_msg = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let base = match this.update(cx, |app, _| app.edit_base_revision.clone()) {
                Ok(Some(rev)) => rev,
                _ => {
                    let _ = this.update(cx, |app, cx| {
                        app.saving = false;
                        app.save_error = Some("文档版本尚未加载，请稍后再试".into());
                        cx.notify();
                    });
                    return;
                }
            };
            match client
                .apply_changeset(&project, &base, &message, changes)
                .await
            {
                Ok(result) => {
                    let _ = this.update(cx, |app, cx| {
                        app.edit_path = Some(path.clone());
                        app.doc_path = Some(path.clone());
                        app.after_save(result.revision, cx);
                    });
                }
                Err(e) => {
                    let is_conflict = e.code == "revision_conflict";
                    let message = e.message.clone();
                    let _ = this.update(cx, |app, cx| {
                        app.saving = false;
                        if is_conflict {
                            app.conflict = Some(ConflictInfo {
                                message,
                                path: path.clone(),
                            });
                        } else {
                            app.save_error = Some(format!("提交失败: {e}"));
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn after_save(&mut self, revision: String, cx: &mut Context<Self>) {
        self.notify("已提交".into(), cx);
        self.current_revision = Some(revision);
        self.edit_base_revision = self.current_revision.clone();
        self.saving = false;
        self.save_error = None;
        self.stop_heartbeat();
        self.editing = false;
        self.editor_preview = false;
        self.lock_held = false;
        let path = self.edit_path.clone();
        self.release_current_lock(cx);
        self.edit_path = None;
        if let (Some(project), Some(path)) = (self.selected_project.as_deref(), path.as_deref()) {
            config::remove_draft(&self.server_url, &self.username, project, path);
        }
        let path = self.doc_path.clone().unwrap_or_default();
        self.open_doc(&path, cx);
    }

    // ----- History: commit list + per-commit diff stats -----

    fn close_history(&mut self, cx: &mut Context<Self>) {
        self.history_open = false;
        self.history_file_path = None;
        self.history_detail_loading = false;
        self.restoring = false;
        self.clear_history_data();
        config::save_layout(&self.layout);
        cx.notify();
    }

    fn clear_history_data(&mut self) {
        self.history_generation = self.history_generation.wrapping_add(1);
        self.commits.clear();
        self.history_has_more = false;
        self.commit_detail = None;
        self.diff_stats.clear();
        self.commit_patch = None;
        self.selected_sha = None;
        self.history_focus = None;
        self.history_error = None;
    }

    fn load_file_history(&mut self, cx: &mut Context<Self>) {
        self.load_history_page(true, cx);
    }

    fn load_more_history(&mut self, cx: &mut Context<Self>) {
        if self.history_has_more {
            self.load_history_page(false, cx);
        }
    }

    fn load_history_page(&mut self, reset: bool, cx: &mut Context<Self>) {
        let (Some(client), Some(project), Some(path)) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.history_file_path.clone(),
        ) else {
            return;
        };
        if self.history_loading && !reset {
            return;
        }
        if reset {
            self.history_generation = self.history_generation.wrapping_add(1);
            self.commits.clear();
            self.history_has_more = false;
            self.history_focus = None;
            self.selected_sha = None;
            self.commit_detail = None;
            self.diff_stats.clear();
            self.commit_patch = None;
        }
        let generation = self.history_generation;
        let query = self.history_input.read(cx).text().trim().to_string();
        let offset = history_page_offset(reset, self.commits.len());
        self.history_loading = true;
        self.history_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let page = if path.is_empty() {
                // 项目级历史：服务端搜索 + 分页
                client
                    .commits_search_page(&project, &query, HISTORY_PAGE_SIZE, offset)
                    .await
            } else {
                // 文件历史：服务端返回该路径全部提交（--follow），客户端过滤 + 分页
                client.file_history(&project, &path).await.map(|commits| {
                    let q = query.to_lowercase();
                    let mut filtered: Vec<_> = if q.is_empty() {
                        commits
                    } else {
                        commits
                            .into_iter()
                            .filter(|c| c.message.to_lowercase().contains(&q))
                            .collect()
                    };
                    let start = (offset as usize).min(filtered.len());
                    let has_more = filtered.len() > start + HISTORY_PAGE_SIZE as usize;
                    filtered.drain(..start);
                    filtered.truncate(HISTORY_PAGE_SIZE as usize);
                    dto::CommitListResponse {
                        commits: filtered,
                        has_more,
                    }
                })
            };
            match page {
                Ok(page) => {
                    let _ = this.update(cx, |app, cx| {
                        if !app.history_open
                            || app.history_file_path.as_deref() != Some(path.as_str())
                            || app.history_generation != generation
                        {
                            return;
                        }
                        app.history_loading = false;
                        app.history_error = None;
                        app.history_has_more = merge_history_page(&mut app.commits, page, reset);
                        let first_sha = app.commits.first().map(|commit| commit.sha.clone());
                        if reset {
                            app.history_focus = first_sha.as_ref().map(|_| 0);
                            app.selected_sha = first_sha.clone();
                        }
                        if reset {
                            if let Some(sha) = first_sha {
                                app.select_commit(&sha, cx);
                            } else {
                                cx.notify();
                            }
                        } else {
                            cx.notify();
                        }
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        if !app.history_open
                            || app.history_file_path.as_deref() != Some(path.as_str())
                            || app.history_generation != generation
                        {
                            return;
                        }
                        app.history_loading = false;
                        app.history_error = Some(format!("加载版本历史失败: {e}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn select_commit(&mut self, sha: &str, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let sha = sha.to_string();
        self.selected_sha = Some(sha.clone());
        self.history_detail_loading = true;
        self.history_error = None;
        self.commit_detail = None;
        self.diff_stats.clear();
        self.commit_patch = None;
        cx.spawn(async move |this, cx| {
            let detail = client.commit_detail(&project, &sha).await;
            let stats = client.diff_stats(&project, &sha).await;
            let patch = client.commit_patch(&project, &sha).await;
            let _ = this.update(cx, |app, cx| {
                if !app.history_open || app.selected_sha.as_deref() != Some(sha.as_str()) {
                    return;
                }
                app.history_detail_loading = false;
                let mut errors = Vec::new();
                match detail {
                    Ok(d) => app.commit_detail = Some(d),
                    Err(e) => errors.push(format!("版本详情: {e}")),
                }
                match stats {
                    Ok(s) => app.diff_stats = s,
                    Err(e) => errors.push(format!("差异统计: {e}")),
                }
                match patch {
                    Ok(p) => app.commit_patch = Some(p.patch),
                    Err(e) => errors.push(format!("Diff: {e}")),
                }
                if !errors.is_empty() {
                    app.history_error = Some(errors.join("；"));
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn confirm_revert_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sha) = self.selected_sha.clone() else {
            return;
        };
        let short: String = sha.chars().take(7).collect();
        let handle = cx.entity();
        let confirm_handle = handle.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let confirm_handle = confirm_handle.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("恢复历史版本？")
                    .message(format!(
                        "将以 {short} 创建一个新的恢复提交，当前文档内容不会被静默覆盖。\n此操作会生成新的提交，请确认后继续。"
                    ))
                    .confirm_label("确认恢复")
                    .cancel_label("取消")
                    .danger()
                    .on_confirm(move |_, window, cx| {
                        let handle = confirm_handle.clone();
                        handle.update(cx, |app, cx| app.revert_selected(cx));
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    /// Restore the selected commit by reverting it server-side (new commit).
    fn revert_selected(&mut self, cx: &mut Context<Self>) {
        if self.restoring {
            return;
        }
        let (Some(client), Some(project), Some(sha)) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.selected_sha.clone(),
        ) else {
            return;
        };
        self.restoring = true;
        self.history_error = None;
        let file_path = self.history_file_path.clone();
        let base_revision = self.current_revision.clone();
        let username = self.username.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = if let Some(path) = file_path {
                let base = match base_revision.or(None) {
                    Some(rev) => rev,
                    None => match client.revision(&project).await {
                        Ok(rev) => rev,
                        Err(e) => {
                            let _ = this.update(cx, |app, cx| {
                                app.restoring = false;
                                app.history_error = Some(format!("读取 revision 失败: {e}"));
                                cx.notify();
                            });
                            return;
                        }
                    },
                };
                match client.page_at(&project, &path, &sha).await {
                    Ok(page) => client
                        .apply_changeset(
                            &project,
                            &base,
                            "恢复历史版本",
                            vec![dto::Change {
                                op: "update".into(),
                                path,
                                new_path: None,
                                content: Some(page.content),
                            }],
                        )
                        .await
                        .map(|r| dto::Commit {
                            sha: r.commit,
                            message: "恢复历史版本".into(),
                            author: username.clone(),
                            date: chrono::Utc::now().to_rfc3339(),
                        }),
                    Err(e) => Err(e),
                }
            } else {
                client.revert_commit(&project, &sha, "恢复历史版本").await
            };
            match result {
                Ok(c) => {
                    let short: String = c.sha.chars().take(7).collect();
                    let _ = this.update(cx, |app, cx| {
                        app.restoring = false;
                        app.notify(format!("已恢复提交 {short} · {}", c.message), cx);
                        app.load_file_history(cx);
                        app.load_revision(cx);
                        let path = app.doc_path.clone().unwrap_or_default();
                        if !path.is_empty() {
                            app.open_doc(&path, cx);
                        }
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.restoring = false;
                        app.history_error = Some(format!("恢复失败: {e}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn confirm_archive_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        project_id: String,
        archived: bool,
    ) {
        if self.project_action.is_some() {
            return;
        }
        let handle = cx.entity();
        let confirm_handle = handle.clone();
        let confirm_project = project_id.clone();
        let action_label = if archived {
            "归档项目"
        } else {
            "恢复项目"
        };
        self.overlay_host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let confirm_handle = confirm_handle.clone();
                let confirm_project = confirm_project.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title(format!("{action_label}？"))
                    .message(if archived {
                        "归档后项目仍保留，但会从活跃项目视图中移除。"
                    } else {
                        "恢复后项目会重新出现在活跃项目视图中。"
                    })
                    .confirm_label(action_label)
                    .cancel_label("取消")
                    .danger()
                    .on_confirm(move |_, window, cx| {
                        let handle = confirm_handle.clone();
                        let project_id = confirm_project.clone();
                        handle.update(cx, |app, cx| {
                            app.set_project_archived(&project_id, archived, cx);
                        });
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    /// Archive or unarchive the given project (right-click context menu).
    fn set_project_archived(&mut self, project_id: &str, archived: bool, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.project_action.is_some() {
            return;
        }
        self.project_action = Some(project_id.to_string());
        self.status_msg = None;
        let id = project_id.to_string();
        cx.spawn(
            async move |this, cx| match client.set_archived(&id, archived).await {
                Ok(p) => {
                    let _ = this.update(cx, |app, cx| {
                        if let Some(row) = app.projects.iter_mut().find(|r| r.id == p.id) {
                            row.archived = p.archived;
                        }
                        app.project_action = None;
                        app.notify(
                            if p.archived {
                                "项目已归档".into()
                            } else {
                                "项目已恢复".into()
                            },
                            cx,
                        );
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.project_action = None;
                        app.status_msg = Some(format!("操作失败: {e}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    /// Open the rename dialog for a project (three-dot menu -> 重命名).
    fn rename_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        project_id: String,
        current_name: String,
    ) {
        if self.project_action.is_some() {
            return;
        }
        let handle = cx.entity();
        let confirm_handle = handle.clone();
        let project_id_for_dialog = project_id.clone();
        let default_name = current_name.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let name_state = cx.new(|cx| {
                    TextInput::new(cx)
                        .placeholder("新名称")
                        .value(&default_name)
                });
                let content_theme = tokens::Cobalt::from_theme(theme(cx));
                let submit_state = name_state.clone();
                let submit_handle = confirm_handle.clone();
                let submit_project = project_id_for_dialog.clone();
                let close_cancel = close.clone();
                let close_confirm = close.clone();
                let cancel = Button::new("cancel-rename-project", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Close).size(Size::Sm))
                    .on_click(move |_, window, cx| close_cancel(window, cx));
                let rename = Button::new("confirm-rename-project", "重命名")
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Check).size(Size::Sm))
                    .on_click(move |_, window, cx| {
                        let new_name = submit_state.read(cx).text().trim().to_string();
                        if new_name.is_empty() {
                            return;
                        }
                        let handle = submit_handle.clone();
                        let project_id = submit_project.clone();
                        handle.update(cx, |app, cx| {
                            app.do_rename_project(&project_id, new_name, cx);
                        });
                        close_confirm(window, cx);
                    });
                Modal::new()
                    .title("重命名项目")
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w_full()
                            .child(mono_label("新名称").text_color(content_theme.ink_3))
                            .child(div().w_full().child(name_state.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(rename),
                    )
                    .into_any_element()
            });
        });
    }

    /// Perform the project rename against the API and refresh the list.
    fn do_rename_project(&mut self, project_id: &str, new_name: String, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.project_action.is_some() {
            return;
        }
        self.project_action = Some(project_id.to_string());
        self.status_msg = None;
        let id = project_id.to_string();
        cx.spawn(
            async move |this, cx| match client.rename_project(&id, &new_name).await {
                Ok(p) => {
                    let _ = this.update(cx, |app, cx| {
                        if let Some(row) = app.projects.iter_mut().find(|r| r.id == p.id) {
                            row.name = p.name.clone();
                        }
                        app.project_action = None;
                        app.notify(format!("已重命名为 {}", p.name), cx);
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.project_action = None;
                        app.status_msg = Some(format!("重命名失败: {e}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    /// Two-step delete confirmation for a project (three-dot menu -> 删除).
    fn confirm_delete_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        project_id: String,
        project_name: String,
    ) {
        if self.project_action.is_some() {
            return;
        }
        let handle = cx.entity();
        let action_handle = handle.clone();
        let confirm_project = project_id.clone();
        let confirm_name = project_name.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let confirm_handle = action_handle.clone();
                let delete_id = confirm_project.clone();
                let delete_name = confirm_name.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("删除项目？")
                    .message(format!(
                        "项目「{delete_name}」及其全部 Git 历史将被彻底删除，无法恢复。"
                    ))
                    .confirm_label("删除项目")
                    .cancel_label("取消")
                    .danger()
                    .on_confirm(move |_, window, cx| {
                        let handle = confirm_handle.clone();
                        let id = delete_id.clone();
                        let name = delete_name.clone();
                        handle.update(cx, |app, cx| app.do_delete_project(&id, &name, cx));
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    /// Perform the project delete and refresh the list.
    fn do_delete_project(&mut self, project_id: &str, _name: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.project_action.is_some() {
            return;
        }
        self.project_action = Some(project_id.to_string());
        self.status_msg = None;
        let id = project_id.to_string();
        cx.spawn(
            async move |this, cx| match client.delete_project(&id).await {
                Ok(()) => {
                    let _ = this.update(cx, |app, cx| {
                        app.projects.retain(|r| r.id != id);
                        if app.selected_project.as_deref() == Some(id.as_str()) {
                            app.selected_project = None;
                        }
                        app.project_action = None;
                        app.notify("项目已删除".into(), cx);
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.project_action = None;
                        app.status_msg = Some(format!("删除失败: {e}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    fn move_target_error(from: &str, to: &str) -> Option<&'static str> {
        if to.is_empty() {
            return Some("目标路径不能为空");
        }
        if to.starts_with('/')
            || to.ends_with('/')
            || to.contains('\\')
            || to
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Some("目标路径必须是相对路径，且不能包含 .、.. 或反斜杠");
        }
        if to == from {
            return Some("目标路径与当前位置相同");
        }
        // A path cannot live under itself: this also rejects files moved to
        // their own "sub-path" (e.g. `a.md` -> `a.md/x`), which the server
        // would reject anyway.
        if to
            .strip_prefix(from)
            .is_some_and(|rest| rest.starts_with('/'))
        {
            return Some("不能移动到自身子路径");
        }
        None
    }

    /// Map `current` to its location after `from` is moved to `to`.
    /// Returns `None` when `current` is not inside the moved subtree.
    /// Prefix matches are guarded by a following `/` so `docs` does not
    /// capture `docs2/...`.
    fn mapped_path(from: &str, to: &str, is_dir: bool, current: &str) -> Option<String> {
        if current == from {
            return Some(to.to_string());
        }
        if is_dir {
            return current
                .strip_prefix(from)
                .filter(|rest| rest.starts_with('/'))
                .map(|rest| format!("{to}{rest}"));
        }
        None
    }

    /// Open a safe move dialog for a doc or directory in the tree.
    fn confirm_move_doc(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        path: String,
        is_dir: bool,
    ) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some(project) = self.selected_project.clone() else {
            return;
        };
        if self.editing {
            return;
        }
        let handle = cx.entity();
        let project_for_dialog = project.clone();
        let client_for_dialog = client.clone();
        let current_path = path.clone();
        let host_handle = self.overlay_host.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                // Start empty: pre-filling the current path would make a
                // direct confirm collide with the same-path guard.
                let path_state = cx.new(|cx| TextInput::new(cx).placeholder("docs/README.md"));
                let content_theme = tokens::Cobalt::from_theme(theme(cx));
                let submit_state = path_state.clone();
                let submit_handle = handle.clone();
                let submit_project = project_for_dialog.clone();
                let submit_client = client_for_dialog.clone();
                let submit_path = current_path.clone();
                let submit_dir = is_dir;
                let toast_host = host_handle.clone();
                let close_cancel = close.clone();
                let close_confirm = close.clone();
                let cancel = Button::new("cancel-move-doc", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Close).size(Size::Sm))
                    .on_click(move |_, window, cx| close_cancel(window, cx));
                let move_button = Button::new("confirm-move-doc", "移动")
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Check).size(Size::Sm))
                    .on_click(move |_, window, cx| {
                        let to = submit_state.read(cx).text().trim().to_string();
                        if let Some(error) = XWikiApp::move_target_error(&submit_path, &to) {
                            let h = toast_host.clone();
                            h.update(cx, |host, cx| host.toast(error, cx));
                            return;
                        }
                        let handle = submit_handle.clone();
                        let project_id = submit_project.clone();
                        let c = submit_client.clone();
                        let from = submit_path.clone();
                        handle.update(cx, |app, cx| {
                            app.move_tree_path(&c, &project_id, &from, &to, submit_dir, true, cx);
                        });
                        close_confirm(window, cx);
                    });
                Modal::new()
                    .title(if is_dir {
                        "移动目录"
                    } else {
                        "移动文档"
                    })
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w_full()
                            .child(mono_label("当前路径").text_color(content_theme.ink_3))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(content_theme.ink_2)
                                    .child(current_path.clone()),
                            )
                            .child(mono_label("目标路径").text_color(content_theme.ink_3))
                            .child(div().w_full().child(path_state.clone()))
                            .child(
                                div().text_xs().text_color(content_theme.ink_3).child(
                                    "移动会创建一次独立提交；目标路径需填写项目内相对路径。",
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(move_button),
                    )
                    .into_any_element()
            });
        });
    }

    /// Open the rename dialog for a doc or directory in the tree.
    fn confirm_rename_doc(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        path: String,
        is_dir: bool,
    ) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some(project) = self.selected_project.clone() else {
            return;
        };
        if self.editing {
            return;
        }
        let handle = cx.entity();
        let confirm_handle = handle.clone();
        let current_path = path.clone();
        let project_for_dialog = project.clone();
        let client_for_dialog = client.clone();
        let default_name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let host_handle = self.overlay_host.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let name_state = cx.new(|cx| {
                    TextInput::new(cx)
                        .placeholder("新名称")
                        .value(&default_name)
                });
                let content_theme = tokens::Cobalt::from_theme(theme(cx));
                let submit_state = name_state.clone();
                let submit_handle = confirm_handle.clone();
                let submit_project = project_for_dialog.clone();
                let submit_client = client_for_dialog.clone();
                let submit_path = current_path.clone();
                let submit_dir = is_dir;
                let toast_host = host_handle.clone();
                let close_cancel = close.clone();
                let close_confirm = close.clone();
                let cancel = Button::new("cancel-rename-doc", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Close).size(Size::Sm))
                    .on_click(move |_, window, cx| close_cancel(window, cx));
                let rename = Button::new("confirm-rename-doc", "重命名")
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Check).size(Size::Sm))
                    .on_click(move |_, window, cx| {
                        let new_name = submit_state.read(cx).text().trim().to_string();
                        if new_name.is_empty() || new_name.contains('/') {
                            let h = toast_host.clone();
                            h.update(cx, |host, cx| host.toast("名称不能为空且不能包含斜杠", cx));
                            return;
                        }
                        let from = submit_path.clone();
                        let to = match from.rsplit_once('/') {
                            Some((parent, _)) => format!("{parent}/{new_name}"),
                            None => new_name.clone(),
                        };
                        let handle = submit_handle.clone();
                        let project_id = submit_project.clone();
                        let c = submit_client.clone();
                        let from_move = from.clone();
                        let to_move = to.clone();
                        handle.update(cx, |app, cx| {
                            app.move_tree_path(
                                &c,
                                &project_id,
                                &from_move,
                                &to_move,
                                submit_dir,
                                false,
                                cx,
                            );
                        });
                        close_confirm(window, cx);
                    });
                Modal::new()
                    .title(if is_dir {
                        "重命名目录"
                    } else {
                        "重命名文档"
                    })
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w_full()
                            .child(mono_label("新名称").text_color(content_theme.ink_3))
                            .child(div().w_full().child(name_state.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(rename),
                    )
                    .into_any_element()
            });
        });
    }

    /// Execute a tree move/rename and refresh the tree.
    fn move_tree_path(
        &mut self,
        client: &Client,
        project: &str,
        from: &str,
        to: &str,
        is_dir: bool,
        moving: bool,
        cx: &mut Context<Self>,
    ) {
        let c = client.clone();
        let project = project.to_string();
        let from = from.to_string();
        let to = to.to_string();
        let action = if moving { "移动" } else { "重命名" };
        let message = format!("{action}{}", if is_dir { "目录" } else { "文档" });
        self.tree_loading = true;
        cx.notify();
        cx.spawn(
            async move |this, cx| match c.move_doc(&project, &from, &to, &message).await {
                Ok(_) => {
                    let _ = this.update(cx, |app, cx| {
                        app.tree_loading = false;
                        // Keep the doc being viewed open after the move:
                        // map its path into the new location and reload it,
                        // because load_tree clears the document state.
                        let keep_doc = app
                            .doc_path
                            .clone()
                            .and_then(|current| Self::mapped_path(&from, &to, is_dir, &current));
                        let tree_path = Self::mapped_path(&from, &to, is_dir, &app.tree_path)
                            .unwrap_or_else(|| app.tree_path.clone());
                        app.load_tree(&tree_path, cx);
                        if let Some(new_doc) = keep_doc {
                            app.open_doc(&new_doc, cx);
                        }
                        app.notify(format!("已{action}为 {to}"), cx);
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.tree_loading = false;
                        app.status_msg = Some(format!("{action}失败: {e}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    /// Two-step delete confirmation for a doc or directory.
    fn confirm_delete_doc(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        path: String,
        is_dir: bool,
    ) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some(project) = self.selected_project.clone() else {
            return;
        };
        if self.editing {
            return;
        }
        let handle = cx.entity();
        let action_handle = handle.clone();
        let confirm_project = project.clone();
        let confirm_client = client.clone();
        let confirm_path = path.clone();
        let confirm_dir = is_dir;
        let confirm_name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let host_handle = self.overlay_host.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let content_theme = tokens::Cobalt::from_theme(theme(cx));
                let content_name = confirm_name.clone();
                let content_dir = confirm_dir;
                // Hard-delete gate: the user must type the full path to enable
                // the irreversible purge button.
                let confirm_input = cx.new(|cx| {
                    TextInput::new(cx).placeholder("输入完整路径以确认")
                });
                let gate_state = confirm_input.clone();
                let gate_handle = action_handle.clone();
                let gate_client = confirm_client.clone();
                let gate_project = confirm_project.clone();
                let gate_path = confirm_path.clone();
                let gate_dir = confirm_dir;
                let gate_toast = host_handle.clone();
                let gate_close = close.clone();
                let hard_delete = Button::new("confirm-delete-doc-hard", "彻底删除（重写历史）")
                    .variant(Variant::Outline)
                    .color(ColorName::Red)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::CircleX).size(Size::Sm).color(ColorName::Red))
                    .on_click(move |_, window, cx| {
                        let typed = gate_state.read(cx).text().trim().to_string();
                        if typed != gate_path {
                            let h = gate_toast.clone();
                            h.update(cx, |host, cx| {
                                host.toast("请输入完整的路径以确认彻底删除", cx)
                            });
                            return;
                        }
                        let handle = gate_handle.clone();
                        let c = gate_client.clone();
                        let project = gate_project.clone();
                        let path = gate_path.clone();
                        let dir = gate_dir;
                        handle.update(cx, |app, cx| {
                            app.purge_tree_path(&c, &project, &path, dir, cx);
                        });
                        gate_close(window, cx);
                    });
                let cancel_close = close.clone();
                let cancel = Button::new("cancel-delete-doc", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Close).size(Size::Sm))
                    .on_click(move |_, window, cx| cancel_close(window, cx));
                let confirm_handle = action_handle.clone();
                let delete_client = confirm_client.clone();
                let delete_project = confirm_project.clone();
                let delete_path = confirm_path.clone();
                let delete_dir = confirm_dir;
                let confirm_close = close.clone();
                let confirm = Button::new("confirm-delete-doc", "删除（保留历史）")
                    .variant(Variant::Filled)
                    .color(ColorName::Red)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Delete).size(Size::Sm).color(ColorName::Red))
                    .on_click(move |_, window, cx| {
                        let handle = confirm_handle.clone();
                        let c = delete_client.clone();
                        let project = delete_project.clone();
                        let path = delete_path.clone();
                        let dir = delete_dir;
                        handle.update(cx, |app, cx| {
                            app.delete_tree_path(&c, &project, &path, dir, cx);
                        });
                        confirm_close(window, cx);
                    });
                Modal::new()
                    .title("删除？")
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex().flex_col()
                            .gap_3()
                            .w_full()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(content_theme.ink_3)
                                    .child(if content_dir {
                                        format!(
                                            "目录「{content_name}」及其全部内容将被删除。\n保留历史：可恢复；彻底删除：重写 Git 历史，不可恢复。"
                                        )
                                    } else {
                                        format!(
                                            "文档「{content_name}」将被删除。\n保留历史：可恢复；彻底删除：重写 Git 历史，不可恢复。"
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .flex().flex_col()
                                    .gap_1()
                                    .w_full()
                                    .child(
                                        mono_label("彻底删除需输入完整路径").text_color(
                                            content_theme.danger,
                                        ),
                                    )
                                    .child(div().w_full().child(confirm_input.clone())),
                            ),
                    )
                    .child(
                        div().flex().items_center()
                            .flex_wrap()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(hard_delete)
                            .child(confirm),
                    )
                    .into_any_element()
            });
        });
    }

    /// Execute a tree delete (recursive for directories) and refresh.
    fn delete_tree_path(
        &mut self,
        client: &Client,
        project: &str,
        path: &str,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        let c = client.clone();
        let project = project.to_string();
        let path = path.to_string();
        let message = format!("删除{}", if is_dir { "目录" } else { "文档" });
        self.tree_loading = true;
        cx.notify();
        cx.spawn(
            async move |this, cx| match c.delete_doc(&project, &path, &message).await {
                Ok(_) => {
                    let _ = this.update(cx, |app, cx| {
                        app.tree_loading = false;
                        if app.doc_path.as_deref() == Some(path.as_str()) {
                            app.doc_path = None;
                            app.doc_content.clear();
                        }
                        let tree_path = app.tree_path.clone();
                        app.load_tree(&tree_path, cx);
                        app.notify("已删除".into(), cx);
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.tree_loading = false;
                        app.status_msg = Some(format!("删除失败: {e}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    /// Execute a hard delete: rewrite git history to remove the path
    /// completely (irreversible).
    fn purge_tree_path(
        &mut self,
        client: &Client,
        project: &str,
        path: &str,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        let c = client.clone();
        let project = project.to_string();
        let path = path.to_string();
        let message = format!("彻底删除{}", if is_dir { "目录" } else { "文档" });
        self.tree_loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            match c
                .purge_paths(&project, std::slice::from_ref(&path), &message)
                .await
            {
                Ok(()) => {
                    let _ = this.update(cx, |app, cx| {
                        app.tree_loading = false;
                        if app.doc_path.as_deref() == Some(path.as_str()) {
                            app.doc_path = None;
                            app.doc_content.clear();
                        }
                        let tree_path = app.tree_path.clone();
                        app.load_tree(&tree_path, cx);
                        app.load_revision(cx);
                        app.notify("已从历史中彻底删除".into(), cx);
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.tree_loading = false;
                        app.status_msg = Some(format!("彻底删除失败: {e}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn palette_commands(&self) -> Vec<PaletteCmd> {
        let mut cmds = Vec::new();
        if self.selected_project.is_some() {
            if !self.editing {
                cmds.push(PaletteCmd {
                    id: "back",
                    label: if self.doc_path.is_some() {
                        "返回文件列表"
                    } else {
                        "返回项目列表"
                    },
                    hint: "esc",
                });
            }
            if !self.history_open && self.doc_path.is_some() {
                cmds.push(PaletteCmd {
                    id: "history",
                    label: "查看历史",
                    hint: "file",
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
        if self.client.is_some() && !self.editing {
            cmds.push(PaletteCmd {
                id: "settings",
                label: "设置",
                hint: "server · 连接",
            });
            cmds.push(PaletteCmd {
                id: "logout",
                label: "退出登录",
                hint: "session",
            });
        }
        cmds
    }

    fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = cx.entity();
        let cmds = self.palette_commands();
        let content_cmds = cmds.clone();
        let content_handle = handle.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let theme = tokens::Cobalt::from_theme(theme(cx));
                let cc = content_cmds.clone();
                let ch = content_handle.clone();
                let mut list = div().flex().flex_col().gap_1().w_full();
                for (index, c) in cc.iter().enumerate() {
                    let handle = ch.clone();
                    let id = c.id;
                    let label = c.label;
                    let hint = c.hint;
                    let close_item = close.clone();
                    list = list.child(
                        div()
                            .id(ElementId::named_usize("cmd", index))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_2()
                            .py_1_5()
                            .rounded(px(tokens::RADIUS_SMALL))
                            .hover(|s| s.bg(theme.surface_accent))
                            .cursor_pointer()
                            .child(div().text_sm().text_color(theme.ink).child(label))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.ink_3)
                                    .child(hint),
                            )
                            .on_click(move |_, window, cx| {
                                close_item(window, cx);
                                let h = handle.clone();
                                h.update(cx, |app, cx| app.run_command(id, cx));
                            }),
                    );
                }
                Modal::new()
                    .title("命令面板 · ⌘K")
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(list)
                    .into_any_element()
            });
        });
    }

    fn run_command(&mut self, id: &str, cx: &mut Context<Self>) {
        match id {
            "back" if self.editing => {}
            "back" if self.doc_path.is_some() => self.back_to_document_browser(cx),
            "back" => self.back_to_projects(cx),
            "history" => self.open_file_history_panel(cx),
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
                self.load_settings_access(cx);
            }
            "logout" => self.logout(cx),
            _ => {}
        }
        cx.notify();
    }

    fn refresh_current_view(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            return;
        }
        match self.screen {
            Screen::Workspace => {
                if let Some(path) = self.doc_path.clone() {
                    self.open_doc(&path, cx);
                } else if self.selected_project.is_some() {
                    let path = self.tree_path.clone();
                    self.load_tree(&path, cx);
                } else {
                    self.load_projects(cx);
                }
            }
            Screen::Settings => self.load_settings_access(cx),
            Screen::ApiReference => self.open_api_reference(cx),
            Screen::Audit => self.open_audit(cx),
            Screen::Backlinks => self.open_backlinks_panel(cx),
            Screen::Attachments => self.open_attachments_panel(cx),
            Screen::Login => {}
        }
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let next = if theme(cx).scheme.is_dark() {
            config::ThemeMode::Light
        } else {
            config::ThemeMode::Dark
        };
        config::save_theme(next);
        let t = match next {
            config::ThemeMode::Light => tokens::cobalt_light(),
            config::ThemeMode::Dark => tokens::cobalt_dark(),
        };
        cx.set_global(t);
        cx.notify();
    }

    fn notify(&mut self, message: String, cx: &mut Context<Self>) {
        self.overlay_host
            .update(cx, |host, cx| host.toast(message, cx));
    }
    fn do_login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let server = {
            let v = self.server_input.read(cx).text().to_string();
            if v.trim().is_empty() {
                config::load_server()
            } else {
                v.trim().to_string()
            }
        };
        let username = self.user_input.read(cx).text().to_string();
        let password = self.password_input.read(cx).text().to_string();
        if username.trim().is_empty() || password.is_empty() {
            self.login_error = Some("请输入用户名和密码".into());
            cx.notify();
            return;
        }
        config::clear_session();
        let client = Client::new(&server);
        self.client = Some(client.clone());
        self.server_url = server.clone();
        self.login_error = None;
        self.loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let login_result = match client.meta().await {
                Ok(meta) if meta.api_version == "1" => {
                    client.login_with_session(username.trim(), &password).await
                }
                Ok(meta) => Err(crate::api::ApiError {
                    code: "unsupported_api_version".into(),
                    message: format!(
                        "服务器 API 版本 {} 不受支持，请升级客户端",
                        meta.api_version
                    ),
                    request_id: None,
                    status: 400,
                }),
                Err(error) => Err(error),
            };
            match login_result {
                Ok((user, cookie)) => {
                    let _ = config::save_server(&server);
                    config::save_username(&user.username);
                    if let Some(cookie) = cookie {
                        config::save_session(&server, &user.username, &cookie);
                    } else {
                        config::clear_session();
                    }
                    let _ = this.update(cx, |app, cx| app.on_login_ok(user, cx));
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.login_error = Some(login_error_message(&e));
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

    /// Toggle the login panel between sign-in and forgot-password forms.
    fn toggle_reset_mode(&mut self, cx: &mut Context<Self>) {
        self.reset_mode = !self.reset_mode;
        self.reset_status = None;
        self.login_error = None;
        cx.notify();
    }

    /// Request a reset token for the username in the form. Self-hosted:
    /// the token is written to the server log; the UI tells the user to
    /// ask the operator for it.
    fn request_reset(&mut self, cx: &mut Context<Self>) {
        let server = {
            let v = self.server_input.read(cx).text().to_string();
            if v.trim().is_empty() {
                config::load_server()
            } else {
                v.trim().to_string()
            }
        };
        let username = self.user_input.read(cx).text().to_string();
        if username.trim().is_empty() {
            self.reset_status = Some((false, "请输入用户名".into()));
            cx.notify();
            return;
        }
        let client = Client::new(&server);
        self.loading = true;
        self.reset_status = None;
        cx.spawn(async move |this, cx| {
            let result = client.forgot_password(username.trim()).await;
            let _ = this.update(cx, |app, cx| {
                app.loading = false;
                match result {
                    Ok(true) => {
                        app.reset_status = Some((
                            true,
                            "已请求重置。请联系服务器运维获取一次性 token（已写入服务端日志）。"
                                .into(),
                        ));
                    }
                    Ok(_) => {
                        app.reset_status = Some((false, "服务器未返回确认，请稍后重试".into()));
                    }
                    Err(e) => {
                        app.reset_status = Some((false, format!("请求失败: {e}")));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Submit token + new password to complete the reset.
    fn submit_reset(&mut self, cx: &mut Context<Self>) {
        let server = {
            let v = self.server_input.read(cx).text().to_string();
            if v.trim().is_empty() {
                config::load_server()
            } else {
                v.trim().to_string()
            }
        };
        let token = self.reset_token_input.read(cx).text().to_string();
        let new_password = self.reset_password_input.read(cx).text().to_string();
        if token.trim().is_empty() || new_password.len() < 8 {
            self.reset_status = Some((false, "需要 token 且新密码至少 8 个字符".into()));
            cx.notify();
            return;
        }
        let client = Client::new(&server);
        self.loading = true;
        self.reset_status = None;
        cx.spawn(async move |this, cx| {
            let result = client.reset_password(token.trim(), &new_password).await;
            let _ = this.update(cx, |app, cx| {
                app.loading = false;
                match result {
                    Ok(true) => {
                        app.reset_status = Some((true, "密码已重置，请使用新密码登录".into()));
                        app.reset_mode = false;
                        let token_input = app.reset_token_input.clone();
                        let password_input = app.reset_password_input.clone();
                        if let Some(window) = cx.active_window() {
                            let _ = cx.update_window(window, |_v, _window, cx| {
                                token_input.update(cx, |s, cx| {
                                    s.set_text("", cx);
                                });
                                password_input.update(cx, |s, cx| {
                                    s.set_text("", cx);
                                });
                            });
                        }
                    }
                    Ok(_) => {
                        app.reset_status = Some((false, "服务器未确认，请重试".into()));
                    }
                    Err(e) => {
                        app.reset_status = Some((false, format!("重置失败: {e}")));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_projects(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.loading = true;
        self.projects_error = None;
        self.project_skeleton_visible = false;
        cx.notify();

        // Do not flash a skeleton for fast local/server responses. Existing
        // project cards remain visible while a refresh is in flight.
        cx.spawn(async move |this, cx| {
            Timer::after(Duration::from_millis(250)).await;
            let _ = this.update(cx, |app, cx| {
                if app.loading && app.projects.is_empty() && app.projects_error.is_none() {
                    app.project_skeleton_visible = true;
                    cx.notify();
                }
            });
        })
        .detach();

        cx.spawn(async move |this, cx| {
            let projects = client.projects().await;
            let meta = client.meta().await;
            match projects {
                Ok(list) => {
                    let _ = this.update(cx, |app, cx| {
                        app.projects = list.iter().map(ProjectRow::from_dto).collect();
                        app.loading = false;
                        app.project_skeleton_visible = false;
                        if let Ok(m) = meta {
                            app.meta_version = Some(m.version);
                        } else {
                            // Non-fatal: the project list still works.
                            app.meta_version = None;
                        }
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.project_skeleton_visible = false;
                        app.projects_error = Some(e.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn logout(&mut self, cx: &mut Context<Self>) {
        self.stop_heartbeat();
        config::clear_session();
        self.screen = Screen::Login;
        self.client = None;
        self.username.clear();
        self.login_error = None;
        self.current_revision = None;
        self.project_action = None;
        self.history_open = false;
        self.history_file_path = None;
        self.history_loading = false;
        self.history_detail_loading = false;
        self.restoring = false;
        self.clear_history_data();
        self.selected_project = None;
        self.tree_entries.clear();
        self.tree_path.clear();
        self.doc_path = None;
        self.doc_content.clear();
        self.doc_outline = outline::ParsedDocument {
            entries: Vec::new(),
            sections: Vec::new(),
        };
        self.backlinks.clear();
        self.attachments.clear();
        self.audit_entries.clear();
        self.audit_projects.clear();
        self.audit_selected_project = None;
        self.api_reference = None;
        self.search_results.clear();
        self.search_open = false;
        self.quick_open = false;
        self.share_url = None;
        self.settings_tokens.clear();
        self.settings_users.clear();
        self.settings_token_secret = None;
        self.settings_ota_loading = false;
        self.settings_ota_status = None;
        self.projects.clear();
        cx.notify();
    }
    fn open_new_project_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let handle = cx.entity();
        open_new_project_dialog_window(window, cx, Some(client), handle);
    }

    /// Rename / move the doc currently being edited via a dialog.
    fn open_rename_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(client), Some(project), Some(path)) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.edit_path.clone(),
        ) else {
            return;
        };
        let handle = cx.entity();
        let default_name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let current_path = path.clone();
        let project_id_outer = project.clone();
        let client_outer = client.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                // Fresh locals per invocation: safe to move into inner closures.
                let content_path = current_path.clone();
                let submit_path = current_path.clone();
                let new_path_state = cx.new(|cx| {
                    TextInput::new(cx)
                        .placeholder("docs/new-name.md")
                        .value(&default_name)
                });
                let theme = tokens::Cobalt::from_theme(theme(cx));

                let close_cancel = close.clone();
                let cancel = Button::new("cancel-rename", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Close).size(Size::Sm))
                    .on_click(move |_, window, cx| close_cancel(window, cx));

                let submit_state = new_path_state.clone();
                let submit_client = client_outer.clone();
                let app_handle = handle.clone();
                let project_id = project_id_outer.clone();
                let close_rename = close.clone();
                let rename = Button::new("confirm-rename", "重命名")
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Check).size(Size::Sm))
                    .on_click(move |_, window, cx| {
                        let new_path = submit_state.read(cx).text().to_string();
                        if new_path.trim().is_empty() {
                            return;
                        }
                        let c = submit_client.clone();
                        let h = app_handle.clone();
                        let pid = project_id.clone();
                        let to = new_path.trim().to_string();
                        let from = submit_path.clone();
                        cx.spawn(async move |cx| {
                            match c.move_doc(&pid, &from, &to, "重命名文档").await {
                                Ok(_) => {
                                    let _ = h.update(cx, |app, cx| {
                                        app.notify(format!("已重命名为 {to}"), cx);
                                        app.edit_path = Some(to.clone());
                                        app.doc_path = Some(to.clone());
                                        // The heartbeat loop captured the old
                                        // path; restart it so the lock on the new
                                        // path stays fresh while editing.
                                        app.start_heartbeat(cx);
                                        cx.notify();
                                    });
                                }
                                Err(e) => {
                                    let _ = h.update(cx, |app, cx| {
                                        app.status_msg = Some(format!("重命名失败: {e}"));
                                        cx.notify();
                                    });
                                }
                            }
                        })
                        .detach();
                        close_rename(window, cx);
                    });

                Modal::new()
                    .title("重命名文档")
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w_full()
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.ink_3)
                                    .child(format!("当前路径 · {content_path}")),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.ink_3)
                                    .child("新路径"),
                            )
                            .child(div().w_full().child(new_path_state.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(rename),
                    )
                    .into_any_element()
            });
        });
    }

    fn render_status_bar(&self, cx: &Context<Self>) -> Div {
        let theme = tokens::Cobalt::from_theme(theme(cx));
        let connected = self.client.is_some();
        let connection_color = if connected {
            theme.success
        } else {
            theme.danger
        };
        let connection_label = if connected { "已连接" } else { "未连接" };
        div()
            .h(px(tokens::STATUS_H))
            .px_3()
            .flex()
            .items_center()
            .gap_3()
            .border_t_1()
            .border_color(theme.rule)
            .bg(theme.paper_2)
            .child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded_full()
                    .bg(connection_color),
            )
            .child(
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(connection_color)
                    .child(connection_label),
            )
            .child(
                div()
                    .max_w(px(360.0))
                    .overflow_x_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.ink_3)
                    .child(tokens::truncate(&self.server_url, 72)),
            )
            .child(div().flex_1())
            .child(if let Some(msg) = &self.status_msg {
                div()
                    .max_w(px(360.0))
                    .overflow_x_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
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
                        theme.success
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
            .child(if let Some(revision) = &self.current_revision {
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.ink_3)
                    .child(format!("revision {}", tokens::truncate(revision, 12)))
            } else {
                div()
            })
            .child(if let Some(version) = &self.meta_version {
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.ink_3)
                    .child(format!("server v{version}"))
            } else {
                div()
            })
    }

    // ----- Window title & screen helpers -----

    fn screen_is_workspace(&self) -> bool {
        matches!(self.screen, Screen::Workspace)
    }

    /// Dynamic window title: XWiki — <project> / <doc>.
    fn window_title(&self) -> String {
        if self.screen_is_workspace() {
            if let Some(project) = self.selected_project.as_deref() {
                let name = self
                    .projects
                    .iter()
                    .find(|p| Some(p.id.as_str()) == self.selected_project.as_deref())
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| project.to_string());
                if let Some(path) = &self.doc_path {
                    return format!("XWiki — {name} / {path}");
                }
                return format!("XWiki — {name}");
            }
            "XWiki".into()
        } else if matches!(self.screen, Screen::Settings) {
            "XWiki — 设置".into()
        } else if matches!(self.screen, Screen::Backlinks) {
            "XWiki — 文档分析".into()
        } else if matches!(self.screen, Screen::Attachments) {
            "XWiki — 附件".into()
        } else {
            "XWiki".into()
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
                label: if self.doc_path.is_some() {
                    "返回文件列表".into()
                } else {
                    "返回项目列表".into()
                },
                hint: "esc".into(),
                target: if self.doc_path.is_some() {
                    QuickTarget::BackToDocumentBrowser
                } else {
                    QuickTarget::BackToProjects
                },
            });
        }
        if self.client.is_some() {
            for p in self.projects.iter() {
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
        if self.editing {
            return;
        }
        match target {
            QuickTarget::OpenProject(id) => self.open_project(&id, cx),
            QuickTarget::OpenDoc(path) => self.open_doc(&path, cx),
            QuickTarget::EnterDir(path) => self.load_tree(&path, cx),
            QuickTarget::BackToDocumentBrowser => self.back_to_document_browser(cx),
            QuickTarget::BackToProjects => self.back_to_projects(cx),
        }
        cx.notify();
    }

    fn toggle_quick_open(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.quick_open = !self.quick_open;
        if self.quick_open {
            let input = self.quick_input.clone();
            if let Some(h) = cx.active_window() {
                let _ = cx.update_window(h, move |_v, _window, cx| {
                    input.update(cx, |s, cx| s.set_text("", cx));
                });
            }
        }
        cx.notify();
    }

    /// Open the project-wide search overlay (document view toolbar).
    fn open_project_search(&mut self, cx: &mut Context<Self>) {
        if self.editing || self.selected_project.is_none() {
            return;
        }
        self.search_open = true;
        self.search_results.clear();
        self.search_error = None;
        let input = self.search_input.clone();
        if let Some(h) = cx.active_window() {
            let _ = cx.update_window(h, move |_v, _window, cx| {
                input.update(cx, |s, cx| s.set_text("", cx));
            });
        }
        cx.notify();
    }

    /// Live search against the current project (debounced by the input
    /// subscription: only fires when the query changes and no request is
    /// in flight).
    fn run_project_search(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let q = self.search_input.read(cx).text().trim().to_string();
        if q.is_empty() {
            return;
        }
        self.search_loading = true;
        self.search_error = None;
        cx.notify();
        cx.spawn(
            async move |this, cx| match client.search(&project, &q).await {
                Ok(results) => {
                    let _ = this.update(cx, |app, cx| {
                        // Discard stale responses: the query may have changed
                        // (or been cleared) while the request was in flight.
                        if app.search_input.read(cx).text().trim() != q {
                            return;
                        }
                        app.search_loading = false;
                        app.search_results = results;
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        if app.search_input.read(cx).text().trim() != q {
                            return;
                        }
                        app.search_loading = false;
                        app.search_error = Some(format!("搜索失败: {e}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    /// Project search overlay: query input + clickable result list.
    fn render_project_search(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = tokens::Cobalt::from_theme(theme(cx));
        let q = self.search_input.read(cx).text().trim().to_string();
        let content: AnyElement = if self.search_loading {
            div()
                .px_4()
                .py_3()
                .text_sm()
                .text_color(theme.ink_3)
                .child("搜索中…")
                .into_any_element()
        } else if let Some(err) = &self.search_error {
            div()
                .px_4()
                .py_3()
                .text_sm()
                .text_color(theme.danger)
                .child(err.clone())
                .into_any_element()
        } else if q.is_empty() {
            div()
                .px_4()
                .py_3()
                .text_sm()
                .text_color(theme.ink_3)
                .child("输入关键词，搜索当前项目全部文档内容。")
                .into_any_element()
        } else if self.search_results.is_empty() {
            div()
                .px_4()
                .py_3()
                .text_sm()
                .text_color(theme.ink_3)
                .child("没有匹配的文档。")
                .into_any_element()
        } else {
            let mut list = div().flex().flex_col().w_full();
            for (index, r) in self.search_results.iter().enumerate() {
                let path = r.path.clone();
                let snippet = r.snippet.clone();
                list = list.child(
                    div()
                        .id(ElementId::named_usize("search-result", index))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .px_4()
                        .py_2()
                        .hover(|s| s.bg(theme.surface_accent))
                        .cursor_pointer()
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.accent)
                                .child(path.clone()),
                        )
                        .child(div().text_sm().text_color(theme.ink).child(snippet))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.search_open = false;
                            this.open_doc(&path, cx);
                            cx.notify();
                        })),
                );
            }
            list.into_any_element()
        };
        div()
            .id("project-search-overlay")
            .absolute()
            .top(px(48.0))
            .right(px(16.0))
            .w(px(480.0))
            .max_h(px(420.0))
            .rounded(px(tokens::RADIUS))
            .border_1()
            .border_color(theme.rule)
            .bg(theme.paper_2)
            .shadow(vec![BoxShadow {
                color: theme.ink.opacity(0.15),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(16.0),
                spread_radius: px(0.0),
            }])
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.rule)
                    .child(
                        div()
                            .text_color(theme.ink_3)
                            .child(Icon::new(IconName::Search)),
                    )
                    .child(div().flex_1().child(self.search_input.clone())),
            )
            .child(
                div()
                    .id("project-search-results")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .child(content),
            )
    }

    /// Quick-open overlay: filterable list of projects + current tree docs.
    fn render_quick_open(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = tokens::Cobalt::from_theme(theme(cx));
        let q = self.quick_input.read(cx).text().to_lowercase();
        let items: Vec<QuickItem> = self
            .quick_items()
            .into_iter()
            .filter(|i| q.is_empty() || i.label.to_lowercase().contains(&q) || i.hint.contains(&q))
            .collect();
        let mut list = div().flex().flex_col().gap_1().w_full().px_4().pb_4();
        if items.is_empty() {
            list = list.child(
                div()
                    .py_6()
                    .text_center()
                    .font_family(tokens::FONT_MONO)
                    .text_size(px(tokens::FONT_SIZE_LABEL))
                    .text_color(theme.ink_3)
                    .child("没有匹配的项目或文档"),
            );
        }
        for (index, it) in items.iter().enumerate() {
            let label = it.label.clone();
            let hint = it.hint.clone();
            let target = it.target.clone();
            list = list.child(
                div()
                    .id(ElementId::named_usize("quick-item", index))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded(px(tokens::RADIUS_SMALL))
                    .hover(|s| s.bg(theme.surface_accent))
                    .cursor_pointer()
                    .child(div().text_sm().text_color(theme.ink).child(label))
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_size(px(tokens::FONT_SIZE_LABEL))
                            .text_color(theme.ink_3)
                            .child(hint),
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.run_quick_open(target.clone(), cx)),
                    ),
            );
        }
        div()
            .id("quick-open-overlay")
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .bg(theme.ink.opacity(0.28))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(96.0))
            .child(
                div()
                    .w(px(560.0))
                    .rounded(px(tokens::RADIUS))
                    .border_1()
                    .border_color(theme.rule)
                    .bg(theme.popover)
                    .shadow(vec![BoxShadow {
                        color: theme.ink.opacity(0.14),
                        offset: point(px(0.0), px(8.0)),
                        blur_radius: px(24.0),
                        spread_radius: px(0.0),
                    }])
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(theme.rule)
                            .child(mono_label("快速打开 · ⌘P").text_color(theme.ink_3)),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(theme.rule)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(mono_label("搜索").text_color(theme.ink_3))
                            .child(div().w_full().child(self.quick_input.clone())),
                    )
                    .child(
                        div()
                            .id("quick-open-list")
                            .max_h(px(360.0))
                            .overflow_y_scroll()
                            .child(list),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.quick_open = false;
                    cx.notify();
                }),
            )
    }

    // ----- Revision-conflict recovery (plan §4) -----

    /// Reload the server copy, dropping the local edit.
    fn resolve_conflict_reload(&mut self, cx: &mut Context<Self>) {
        let path = self.conflict.take().map(|c| c.path);
        self.stop_heartbeat();
        self.editing = false;
        self.lock_held = false;
        self.saving = false;
        self.save_error = None;
        let (client, project, edit_path) = (
            self.client.clone(),
            self.selected_project.clone(),
            self.edit_path.take(),
        );
        if let (Some(client), Some(project), Some(path)) = (client, project, edit_path) {
            config::remove_draft(&self.server_url, &self.username, &project, &path);
            // Same as cancel_edit: the server-side lock lingers until its TTL
            // otherwise, blocking other editors of this doc.
            cx.spawn(async move |_this, _cx| {
                let _ = client.release_lock(&project, &path).await;
            })
            .detach();
        }
        if let Some(p) = path {
            self.open_doc(&p, cx);
        }
        cx.notify();
    }

    /// Retry the save against the fresh revision (last-writer-wins).
    fn resolve_conflict_force(&mut self, cx: &mut Context<Self>) {
        self.conflict = None;
        self.save_error = None;
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        cx.spawn(
            async move |this, cx| match client.revision(&project).await {
                Ok(revision) => {
                    let _ = this.update(cx, |app, cx| {
                        app.edit_base_revision = Some(revision);
                        app.save_edit(cx);
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.save_error = Some(format!("读取最新 revision 失败: {error}"));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    /// Abandon the edit, releasing the lock.
    fn resolve_conflict_abandon(&mut self, cx: &mut Context<Self>) {
        self.conflict = None;
        self.cancel_edit(cx);
    }

    fn load_settings_access(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.settings_access_loading = true;
        self.settings_error = None;
        cx.spawn(async move |this, cx| {
            let tokens = client.tokens().await;
            let users = client.users().await;
            let _ = this.update(cx, |app, cx| {
                app.settings_access_loading = false;
                let mut errors = Vec::new();
                match tokens {
                    // Revoked tokens remain in the audit/API response, but they
                    // are no longer actionable credentials. Keep them out of
                    // the settings list so it only represents usable keys.
                    Ok(items) => {
                        app.settings_tokens = items
                            .into_iter()
                            .filter(|token| token.revoked_at.is_empty())
                            .collect()
                    }
                    Err(e) => errors.push(format!("Token：{}", Self::friendly_api_error(&e))),
                }
                match users {
                    Ok(items) => app.settings_users = items,
                    Err(e) => errors.push(format!("用户：{}", Self::friendly_api_error(&e))),
                }
                if !errors.is_empty() {
                    app.settings_error = Some(format!("访问控制加载失败：{}", errors.join("；")));
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Open the audit page: load the project list, default the picker to the
    /// currently open project (else the first one, like the web audit page),
    /// then load its audit log.
    pub(crate) fn open_audit(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.screen = Screen::Audit;
        self.audit_loading = true;
        self.audit_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| match client.projects().await {
            Ok(projects) => {
                let _ = this.update(cx, |app, cx| {
                    app.audit_projects = projects;
                    let contains = |id: &str| app.audit_projects.iter().any(|p| p.id == id);
                    if !app
                        .audit_selected_project
                        .as_ref()
                        .is_some_and(|id| contains(id))
                    {
                        app.audit_selected_project = app
                            .selected_project
                            .clone()
                            .filter(|id| contains(id))
                            .or_else(|| app.audit_projects.first().map(|p| p.id.clone()));
                    }
                    app.load_audit(cx);
                });
            }
            Err(error) => {
                let _ = this.update(cx, |app, cx| {
                    app.audit_loading = false;
                    app.audit_error = Some(format!(
                        "项目列表加载失败：{}",
                        Self::friendly_api_error(&error)
                    ));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Load the audit log for the project selected on the audit page.
    /// With no projects on the server there is nothing to show, so stay
    /// silent (no error) instead of surfacing a red error — the web audit
    /// page renders nothing in that case.
    fn load_audit(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) =
            (self.client.clone(), self.audit_selected_project.clone())
        else {
            self.audit_entries.clear();
            self.audit_loading = false;
            self.audit_error = None;
            cx.notify();
            return;
        };
        self.audit_loading = true;
        self.audit_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| match client.audit(&project).await {
            Ok(entries) => {
                let _ = this.update(cx, |app, cx| {
                    // Discard stale responses: the user may have switched the
                    // project picker while the request was in flight.
                    if app.audit_selected_project.as_deref() != Some(project.as_str()) {
                        return;
                    }
                    app.audit_loading = false;
                    app.audit_entries = entries;
                    cx.notify();
                });
            }
            Err(e) => {
                let _ = this.update(cx, |app, cx| {
                    if app.audit_selected_project.as_deref() != Some(project.as_str()) {
                        return;
                    }
                    app.audit_loading = false;
                    app.audit_error = Some(format!(
                        "审计日志加载失败：{}",
                        Self::friendly_api_error(&e)
                    ));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn open_create_token_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_access_loading || self.client.is_none() {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        let projects: Vec<(String, String)> = self
            .projects
            .iter()
            .map(|project| (project.id.clone(), project.name.clone()))
            .collect();
        let handle = cx.entity();
        let host_handle = self.overlay_host.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let name_state = cx.new(|cx| TextInput::new(cx).placeholder("ci-docs"));
                let draft = Rc::new(RefCell::new(TokenDraft::default()));
                let _content_name = name_state.clone();
                let content_draft = draft.clone();
                let content_projects = projects.clone();
                let theme = tokens::Cobalt::from_theme(theme(cx));
                let scope = content_draft.borrow().scope();
                let read_draft = content_draft.clone();
                let write_draft = content_draft.clone();
                let scope_read = Button::new("token-scope-read", "只读")
                    .variant(if scope == "read" {
                        Variant::Filled
                    } else {
                        Variant::Subtle
                    })
                    .size(Size::Xs)
                    .on_click(move |_, window, _| {
                        read_draft.borrow_mut().set_scope("read");
                        window.refresh();
                    });
                let scope_write = Button::new("token-scope-write", "读写")
                    .variant(if scope == "write" {
                        Variant::Filled
                    } else {
                        Variant::Subtle
                    })
                    .size(Size::Xs)
                    .on_click(move |_, window, _| {
                        write_draft.borrow_mut().set_scope("write");
                        window.refresh();
                    });
                let project_checks: Vec<AnyElement> = content_projects
                    .iter()
                    .enumerate()
                    .map(|(index, (id, name))| {
                        let project_id = id.clone();
                        let project_draft = content_draft.clone();
                        let binding = Binding::new(
                            {
                                let d = project_draft.clone();
                                let project_id = project_id.clone();
                                move |_cx: &App| d.borrow().projects.contains(&project_id)
                            },
                            {
                                let d = project_draft.clone();
                                let project_id = project_id.clone();
                                move |_cx: &mut App, v: bool| {
                                    d.borrow_mut().set_project(&project_id, v)
                                }
                            },
                        );
                        Checkbox::new(ElementId::named_usize("token-project", index))
                            .checked(binding.get(cx))
                            .bind(binding)
                            .label(format!("{name} · {id}"))
                            .into_any_element()
                    })
                    .collect();
                let cancel_close = close.clone();
                let cancel = Button::new("cancel-create-token", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Close).size(Size::Sm))
                    .on_click(move |_, window, cx| cancel_close(window, cx));
                let create_name = name_state.clone();
                let create_draft = draft.clone();
                let create_client = client.clone();
                let create_handle = handle.clone();
                let create_toast = host_handle.clone();
                let create_close = close.clone();
                let create = Button::new("confirm-create-token", "创建 Token")
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Plus).size(Size::Sm))
                    .on_click(move |_, window, cx| {
                        let name = create_name.read(cx).text().trim().to_string();
                        if name.is_empty() {
                            let h = create_toast.clone();
                            h.update(cx, |host, cx| host.toast("Token 名称不能为空", cx));
                            return;
                        }
                        let scope = create_draft.borrow().scope().to_string();
                        let projects = create_draft.borrow().projects();
                        if projects.is_empty() {
                            let h = create_toast.clone();
                            h.update(cx, |host, cx| host.toast("至少选择一个项目", cx));
                            return;
                        }
                        let c = create_client.clone();
                        let h = create_handle.clone();
                        h.update(cx, |app, cx| {
                            app.settings_access_loading = true;
                            app.settings_error = None;
                            cx.notify();
                        });
                        cx.spawn(async move |cx| {
                            match c.create_token(&name, &scope, projects).await {
                                Ok((_token, secret)) => {
                                    let _ = h.update(cx, |app, cx| {
                                        app.settings_access_loading = false;
                                        app.settings_token_secret = Some(secret);
                                        app.notify("Token 已创建，请立即复制密钥".into(), cx);
                                        app.load_settings_access(cx);
                                    });
                                }
                                Err(e) => {
                                    let _ = h.update(cx, |app, cx| {
                                        app.settings_access_loading = false;
                                        app.settings_error = Some(format!(
                                            "创建 Token 失败：{}",
                                            Self::friendly_api_error(&e)
                                        ));
                                        cx.notify();
                                    });
                                }
                            }
                        })
                        .detach();
                        create_close(window, cx);
                    });
                Modal::new()
                    .title("创建访问 Token")
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w_full()
                            .child(mono_label("名称").text_color(theme.ink_3))
                            .child(div().w_full().child(name_state.clone()))
                            .child(mono_label("权限范围").text_color(theme.ink_3))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(scope_read)
                                    .child(scope_write),
                            )
                            .child(mono_label("项目范围（至少选择一个）").text_color(theme.ink_3))
                            .child(
                                div()
                                    .id("token-project-list")
                                    .max_h(px(220.0))
                                    .overflow_y_scroll()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .children(project_checks),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(create),
                    )
                    .into_any_element()
            });
        });
    }

    fn open_create_user_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_access_loading || self.client.is_none() {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        let handle = cx.entity();
        let host_handle = self.overlay_host.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let username_state = cx.new(|cx| TextInput::new(cx).placeholder("operator"));
                let password_state = cx.new(|cx| {
                    TextInput::new(cx)
                        .placeholder("至少 8 个字符")
                        .password(true)
                });
                let theme = tokens::Cobalt::from_theme(theme(cx));
                let cancel_close = close.clone();
                let cancel = Button::new("cancel-create-user", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Close).size(Size::Sm))
                    .on_click(move |_, window, cx| cancel_close(window, cx));
                let create_username = username_state.clone();
                let create_password = password_state.clone();
                let create_client = client.clone();
                let create_handle = handle.clone();
                let create_toast = host_handle.clone();
                let create_close = close.clone();
                let create = Button::new("confirm-create-user", "创建用户")
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::Plus).size(Size::Sm))
                    .on_click(move |_, window, cx| {
                        let username = create_username.read(cx).text().trim().to_string();
                        let password = create_password.read(cx).text().to_string();
                        if username.is_empty() || password.len() < 8 {
                            let h = create_toast.clone();
                            h.update(cx, |host, cx| {
                                host.toast("用户名不能为空，密码至少需要 8 个字符", cx)
                            });
                            return;
                        }
                        let c = create_client.clone();
                        let h = create_handle.clone();
                        h.update(cx, |app, cx| {
                            app.settings_access_loading = true;
                            app.settings_error = None;
                            cx.notify();
                        });
                        cx.spawn(
                            async move |cx| match c.create_user(&username, &password).await {
                                Ok(_) => {
                                    let _ = h.update(cx, |app, cx| {
                                        app.settings_access_loading = false;
                                        app.notify("用户已创建".into(), cx);
                                        app.load_settings_access(cx);
                                    });
                                }
                                Err(e) => {
                                    let _ = h.update(cx, |app, cx| {
                                        app.settings_access_loading = false;
                                        app.settings_error = Some(format!(
                                            "创建用户失败：{}",
                                            Self::friendly_api_error(&e)
                                        ));
                                        cx.notify();
                                    });
                                }
                            },
                        )
                        .detach();
                        create_close(window, cx);
                    });
                Modal::new()
                    .title("创建用户")
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w_full()
                            .child(mono_label("用户名").text_color(theme.ink_3))
                            .child(div().w_full().child(username_state.clone()))
                            .child(mono_label("初始密码").text_color(theme.ink_3))
                            .child(div().w_full().child(password_state.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(create),
                    )
                    .into_any_element()
            });
        });
    }

    fn confirm_revoke_token(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        id: String,
        name: String,
    ) {
        if self.settings_access_loading {
            return;
        }
        let handle = cx.entity();
        let confirm_handle = handle.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let confirm_target = confirm_handle.clone();
                let token_id = id.clone();
                let content_name = name.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("撤销访问 Token？")
                    .message(format!("Token「{content_name}」撤销后将无法继续使用。"))
                    .confirm_label("撤销 Token")
                    .cancel_label("取消")
                    .danger()
                    .on_confirm(move |_, window, cx| {
                        let handle = confirm_target.clone();
                        let token_id = token_id.clone();
                        handle.update(cx, |app, cx| app.revoke_token(&token_id, cx));
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    fn confirm_disable_user(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        id: String,
        name: String,
    ) {
        if self.settings_access_loading {
            return;
        }
        let handle = cx.entity();
        let confirm_handle = handle.clone();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let confirm_target = confirm_handle.clone();
                let user_id = id.clone();
                let content_name = name.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("停用用户？")
                    .message(format!("用户「{content_name}」将无法继续登录。"))
                    .confirm_label("停用用户")
                    .cancel_label("取消")
                    .danger()
                    .on_confirm(move |_, window, cx| {
                        let handle = confirm_target.clone();
                        let user_id = user_id.clone();
                        handle.update(cx, |app, cx| app.set_user_enabled(&user_id, false, cx));
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    fn revoke_token(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.settings_access_loading {
            return;
        }
        self.settings_access_loading = true;
        self.settings_error = None;
        let id = id.to_string();
        cx.spawn(async move |this, cx| match client.revoke_token(&id).await {
            Ok(()) => {
                let _ = this.update(cx, |app, cx| {
                    app.notify("Token 已撤销".into(), cx);
                    app.load_settings_access(cx);
                });
            }
            Err(e) => {
                let _ = this.update(cx, |app, cx| {
                    app.settings_access_loading = false;
                    app.settings_error =
                        Some(format!("撤销 Token 失败：{}", Self::friendly_api_error(&e)));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn set_user_enabled(&mut self, id: &str, enabled: bool, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.settings_access_loading {
            return;
        }
        self.settings_access_loading = true;
        self.settings_error = None;
        let id = id.to_string();
        cx.spawn(
            async move |this, cx| match client.set_user_enabled(&id, enabled).await {
                Ok(_) => {
                    let _ = this.update(cx, |app, cx| {
                        app.notify(
                            if enabled {
                                "用户已启用"
                            } else {
                                "用户已停用"
                            }
                            .into(),
                            cx,
                        );
                        app.load_settings_access(cx);
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.settings_access_loading = false;
                        app.settings_error = Some(format!(
                            "更新用户状态失败：{}",
                            Self::friendly_api_error(&e)
                        ));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    fn friendly_api_error(error: &crate::api::ApiError) -> String {
        if error.code == "response_too_large" {
            return error.message.clone();
        }
        match error.status {
            0 => "无法连接到服务，请检查地址和网络连接。".into(),
            401 | 403 => "服务拒绝了请求，请重新登录后再试。".into(),
            404 => "服务已响应，但未找到 XWiki 接口，请检查服务地址。".into(),
            400..=499 => format!("请求未被接受（HTTP {}），请检查服务配置。", error.status),
            500..=599 => "服务暂时不可用，请稍后重试。".into(),
            status => format!("连接失败（HTTP {}），请稍后重试。", status),
        }
    }

    fn check_ota_update(&mut self, cx: &mut Context<Self>) {
        if self.settings_ota_loading {
            return;
        }

        self.settings_ota_loading = true;
        self.settings_ota_status = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = Client::latest_github_release(OTA_GITHUB_OWNER, OTA_GITHUB_REPO).await;
            let _ = this.update(cx, |app, cx| {
                app.settings_ota_loading = false;
                match result {
                    Ok(release) => {
                        let current = Self::parse_ota_version(env!("CARGO_PKG_VERSION"));
                        let latest = Self::parse_ota_version(&release.tag_name);
                        app.settings_ota_status = Some(match (current, latest) {
                            (Some(current), Some(latest)) if latest > current => (
                                true,
                                format!(
                                    "发现新版本 {} · {}",
                                    Self::format_ota_version(latest),
                                    release.html_url
                                ),
                            ),
                            (Some(current), Some(_)) => (
                                true,
                                format!(
                                    "当前已是最新版本 {} · GitHub Releases",
                                    Self::format_ota_version(current)
                                ),
                            ),
                            _ => (
                                false,
                                format!("无法识别 GitHub Release 版本号：{}", release.tag_name),
                            ),
                        });
                    }
                    Err(error) => {
                        app.settings_ota_status = Some((
                            false,
                            format!(
                                "检查 GitHub Releases 失败：{}",
                                Self::github_release_error(&error)
                            ),
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn parse_ota_version(version: &str) -> Option<(u32, u32, u32)> {
        let version = version
            .strip_prefix("app-v")
            .or_else(|| version.strip_prefix('v'))
            .unwrap_or(version);
        let version = version.split_once('-').map_or(version, |(core, _)| core);
        let mut parts = version.split('.');
        Some((
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        ))
    }

    fn format_ota_version(version: (u32, u32, u32)) -> String {
        format!("v{}.{}.{}", version.0, version.1, version.2)
    }

    fn github_release_error(error: &crate::api::ApiError) -> String {
        match error.status {
            0 => "无法连接 GitHub，请检查网络连接。".into(),
            403 | 429 => "GitHub API 请求受限，请稍后重试。".into(),
            404 => "GitHub Releases 中没有可用的正式版本。".into(),
            500..=599 => "GitHub 暂时不可用，请稍后重试。".into(),
            status => format!("请求失败（HTTP {}），请稍后重试。", status),
        }
    }

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        let url = {
            let v = self.settings_server_input.read(cx).text().to_string();
            if v.trim().is_empty() {
                config::load_server()
            } else {
                v.trim().to_string()
            }
        };
        self.settings_test = None;
        self.settings_test_detail = None;
        self.settings_loading = true;
        let c = Client::new(&url);
        cx.spawn(async move |this, cx| match c.meta().await {
            Ok(m) => {
                let detail = format!("服务版本 {}", m.version);
                let _ = this.update(cx, |app, cx| {
                    app.settings_loading = false;
                    app.settings_test = Some((true, "连接成功".into()));
                    app.settings_test_detail = Some(detail);
                    cx.notify();
                });
            }
            Err(e) => {
                let message = Self::friendly_api_error(&e);
                let detail = e.request_id.map(|id| format!("请求 ID · {}", id));
                let _ = this.update(cx, |app, cx| {
                    app.settings_loading = false;
                    app.settings_test = Some((false, message));
                    app.settings_test_detail = detail;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn save_server_settings(&mut self, cx: &mut Context<Self>) {
        let url = self
            .settings_server_input
            .read(cx)
            .text()
            .trim()
            .to_string();
        if url.is_empty() {
            self.settings_test = Some((false, "请输入服务地址。".into()));
            self.settings_test_detail = None;
            cx.notify();
            return;
        }
        if self.server_url != url {
            config::clear_session();
        }
        if config::save_server(&url).is_err() {
            self.settings_test = Some((false, "服务地址保存失败，请检查配置目录权限。".into()));
            self.settings_test_detail = None;
            cx.notify();
            return;
        }
        self.server_url = url;
        self.settings_test = None;
        self.settings_test_detail = None;
        self.notify("服务地址已保存，重新登录后生效".into(), cx);
    }

    // ----- Workspace import/export -----

    pub(crate) fn open_import_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = cx.entity();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let theme = tokens::Cobalt::from_theme(theme(cx));
                let name = cx.new(|cx| TextInput::new(cx).placeholder("项目名"));
                let description = cx.new(|cx| TextInput::new(cx).placeholder("描述（可选）"));
                let folder = cx.new(|cx| TextInput::new(cx).placeholder("文件夹路径"));
                let repo_url = cx.new(|cx| TextInput::new(cx).placeholder("Git 仓库 URL"));
                let bundle = cx.new(|cx| TextInput::new(cx).placeholder("Bundle 文件路径"));

                let folder_name = name.clone();
                let folder_description = description.clone();
                let folder_path = folder.clone();
                let folder_picker_state = folder.clone();
                let folder_picker_handle = handle.clone();
                let folder_handle = handle.clone();
                let folder_button = Button::new("import-folder", "预览文件夹")
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| {
                        let name = folder_name.read(cx).text().trim().to_string();
                        let description = folder_description.read(cx).text().trim().to_string();
                        let path = folder_path.read(cx).text().trim().to_string();
                        folder_handle.update(cx, |app, cx| {
                            app.prepare_folder_import(window, name, description, path, cx)
                        });
                    });

                let repo_name = name.clone();
                let repo_input = repo_url.clone();
                let repo_handle = handle.clone();
                let repo_button = Button::new("import-repo", "确认导入仓库")
                    .variant(Variant::Outline)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| {
                        let name = repo_name.read(cx).text().trim().to_string();
                        let url = repo_input.read(cx).text().trim().to_string();
                        repo_handle
                            .update(cx, |app, cx| app.confirm_repo_import(window, name, url, cx));
                    });

                let bundle_name = name.clone();
                let bundle_input = bundle.clone();
                let bundle_handle = handle.clone();
                let bundle_button = Button::new("import-bundle", "预览 Bundle")
                    .variant(Variant::Outline)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| {
                        let name = bundle_name.read(cx).text().trim().to_string();
                        let path = bundle_input.read(cx).text().trim().to_string();
                        bundle_handle.update(cx, |app, cx| {
                            app.prepare_bundle_import(window, name, path, cx)
                        });
                    });

                let close_cancel = close.clone();
                let cancel = Button::new("cancel-import", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| close_cancel(window, cx));

                Modal::new()
                    .title("导入项目")
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w_full()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.ink_3)
                                    .child("填写来源后先预览；确认后才会读取并上传本地内容。"),
                            )
                            .child(mono_label("项目名").text_color(theme.ink_3))
                            .child(div().w_full().child(name.clone()))
                            .child(mono_label("描述").text_color(theme.ink_3))
                            .child(div().w_full().child(description.clone()))
                            .child(mono_label("文件夹路径").text_color(theme.ink_3))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .w_full()
                                    .child(div().flex_1().child(folder.clone()))
                                    .child(import_folder_picker_button(
                                        folder_picker_state.clone(),
                                        folder_picker_handle.clone(),
                                    )),
                            )
                            .child(mono_label("Git URL").text_color(theme.ink_3))
                            .child(div().w_full().child(repo_url.clone()))
                            .child(mono_label("Bundle 路径").text_color(theme.ink_3))
                            .child(div().w_full().child(bundle.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(bundle_button)
                            .child(repo_button)
                            .child(folder_button),
                    )
                    .into_any_element()
            });
        });
    }

    pub(crate) fn open_document_folder_import_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_document_import_dialog(window, cx, DocumentImportMode::Folder);
    }

    pub(crate) fn open_document_markdown_import_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_document_import_dialog(window, cx, DocumentImportMode::Markdown);
    }

    fn open_document_import_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        mode: DocumentImportMode,
    ) {
        if self.selected_project.is_none() {
            return;
        }
        let handle = cx.entity();
        let base_path = self.tree_path.clone();
        let (title, placeholder, action_label) = match mode {
            DocumentImportMode::Folder => {
                ("导入文件夹到当前项目", "本地文件夹路径", "预览文件夹导入")
            }
            DocumentImportMode::Markdown => (
                "导入 Markdown 到当前项目",
                "Markdown 文件路径",
                "预览 Markdown 导入",
            ),
        };
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let theme = tokens::Cobalt::from_theme(theme(cx));
                let source = cx.new(|cx| TextInput::new(cx).placeholder(placeholder));
                let action_source = source.clone();
                let action_handle = handle.clone();
                let action_base_path = base_path.clone();
                let action = Button::new("preview-document-import", action_label)
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| {
                        let source = action_source.read(cx).text().trim().to_string();
                        action_handle.update(cx, |app, cx| {
                            app.prepare_document_import(
                                window,
                                mode,
                                source,
                                action_base_path.clone(),
                                cx,
                            )
                        });
                    });
                let close_cancel = close.clone();
                let cancel = Button::new("cancel-document-import", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| close_cancel(window, cx));
                Modal::new()
                    .title(title)
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w_full()
                            .child(div().text_sm().text_color(theme.ink_3).child(match mode {
                                DocumentImportMode::Folder => {
                                    "只导入文件夹中的 Markdown 文件，保留相对目录。"
                                }
                                DocumentImportMode::Markdown => {
                                    "选择一个 .md 文件，导入到当前文档目录。"
                                }
                            }))
                            .child(mono_label("来源路径").text_color(theme.ink_3))
                            .child(div().w_full().child(source.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(action),
                    )
                    .into_any_element()
            });
        });
    }

    fn prepare_document_import(
        &mut self,
        window: &mut Window,
        mode: DocumentImportMode,
        source: String,
        base_path: String,
        cx: &mut Context<Self>,
    ) {
        if source.trim().is_empty() {
            self.status_msg = Some("请输入来源路径。".into());
            cx.notify();
            return;
        }
        let files = match mode {
            DocumentImportMode::Folder => collect_import_files(std::path::Path::new(&source))
                .and_then(|files| {
                    let mut imported = Vec::new();
                    for file in files {
                        if !is_markdown_import_path(&file.path) {
                            continue;
                        }
                        let content = String::from_utf8(file.content)
                            .map_err(|_| format!("文件 {} 不是 UTF-8 Markdown。", file.path))?;
                        imported.push(dto::ImportFile {
                            path: join_document_import_path(&base_path, &file.path),
                            content,
                        });
                    }
                    Ok(imported)
                }),
            DocumentImportMode::Markdown => (|| {
                let path = std::path::Path::new(&source);
                if !path.is_file() {
                    Err("路径不是 Markdown 文件。".into())
                } else if !is_markdown_import_path(&source) {
                    Err("请选择 .md 或 .markdown 文件。".into())
                } else {
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| "imported.md".into());
                    let content = String::from_utf8(
                        read_file_limited(path, MAX_IMPORT_FILE_BYTES)
                            .map_err(|error| format!("读取 Markdown 失败: {error}"))?,
                    )
                    .map_err(|_| "Markdown 文件不是 UTF-8。".to_string())?;
                    Ok(vec![dto::ImportFile {
                        path: join_document_import_path(&base_path, &name),
                        content,
                    }])
                }
            })(),
        };
        match files {
            Ok(files) if !files.is_empty() => self.confirm_document_import(window, files, cx),
            Ok(_) => {
                self.status_msg = Some("来源文件夹中没有 Markdown 文件。".into());
                cx.notify();
            }
            Err(error) => {
                self.status_msg = Some(error);
                cx.notify();
            }
        }
    }

    fn confirm_document_import(
        &mut self,
        window: &mut Window,
        files: Vec<dto::ImportFile>,
        cx: &mut Context<Self>,
    ) {
        let handle = cx.entity();
        let count = files.len();
        let summary = files
            .iter()
            .take(5)
            .map(|file| file.path.clone())
            .collect::<Vec<_>>()
            .join("、");
        let more = count.saturating_sub(5);
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let theme = tokens::Cobalt::from_theme(theme(cx));
                let confirm_handle = handle.clone();
                let confirm_files = files.clone();
                let close_confirm = close.clone();
                let confirm = Button::new("confirm-document-import", "确认导入")
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| {
                        let files = confirm_files.clone();
                        confirm_handle.update(cx, |app, cx| app.execute_document_import(files, cx));
                        close_confirm(window, cx);
                    });
                let summary_for_content = summary.clone();
                let close_cancel = close.clone();
                let cancel = Button::new("cancel-confirm-document-import", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| close_cancel(window, cx));
                let suffix = if more > 0 {
                    format!(" 等 {} 个文件", more)
                } else {
                    String::new()
                };
                Modal::new()
                    .title("确认导入文档")
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(div().text_sm().text_color(theme.ink_3).child(format!(
                        "将导入 {} 个文件：{}{}",
                        count, summary_for_content, suffix
                    )))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(confirm),
                    )
                    .into_any_element()
            });
        });
    }

    fn execute_document_import(&mut self, files: Vec<dto::ImportFile>, cx: &mut Context<Self>) {
        let (Some(client), Some(project)) = (self.client.clone(), self.selected_project.clone())
        else {
            return;
        };
        let current_revision = self.current_revision.clone();
        self.loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = async {
                let base_revision = if let Some(revision) = current_revision {
                    revision
                } else {
                    client.revision(&project).await?
                };
                client
                    .import_files(&project, &base_revision, "导入文档", files)
                    .await
            }
            .await;
            match result {
                Ok(result) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.current_revision = Some(result.revision);
                        app.notify(format!("已导入 {} 个 Markdown 文件", result.imported), cx);
                        let path = app.tree_path.clone();
                        app.load_tree(&path, cx);
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.status_msg = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub(crate) fn open_export_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.selected_project.clone() else {
            return;
        };
        let handle = cx.entity();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, cx| {
                let theme = tokens::Cobalt::from_theme(theme(cx));
                let destination =
                    cx.new(|cx| TextInput::new(cx).placeholder("保存路径（可选）"));
                let zip_destination = destination.clone();
                let zip_project = project.clone();
                let zip_handle = handle.clone();
                let zip = Button::new("export-zip", "预览 ZIP 导出")
                    .variant(Variant::Filled)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| {
                        let destination = zip_destination.read(cx).text().trim().to_string();
                        zip_handle.update(cx, |app, cx| {
                            app.confirm_export(window, zip_project.clone(), "zip", destination, cx)
                        });
                    });
                let bundle_destination = destination.clone();
                let bundle_project = project.clone();
                let bundle_handle = handle.clone();
                let bundle = Button::new("export-bundle", "预览 Bundle 导出")
                    .variant(Variant::Outline)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| {
                        let destination = bundle_destination.read(cx).text().trim().to_string();
                        bundle_handle.update(cx, |app, cx| {
                            app.confirm_export(
                                window,
                                bundle_project.clone(),
                                "bundle",
                                destination,
                                cx,
                            )
                        });
                    });
                let close_cancel = close.clone();
                let cancel = Button::new("cancel-export", "取消")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .on_click(move |_, window, cx| close_cancel(window, cx));
                Modal::new()
                    .title("导出项目")
                    .on_close(move |_ev, window, cx| close(window, cx))
                    .child(
                        div()
                            .flex().flex_col()
                            .gap_2()
                            .w_full()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.ink_3)
                                    .child("ZIP 是工作树快照；Bundle 保留完整 Git 历史。确认后才会写入本地路径。"),
                            )
                            .child(mono_label("保存路径").text_color(theme.ink_3))
                            .child(div().w_full().child(destination.clone())),
                    )
                    .child(
                        div().flex().items_center()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(cancel)
                            .child(bundle)
                            .child(zip),
                    )
                    .into_any_element()
            });
        });
    }

    fn prepare_folder_import(
        &mut self,
        window: &mut Window,
        name: String,
        description: String,
        source: String,
        cx: &mut Context<Self>,
    ) {
        if name.trim().is_empty() || source.trim().is_empty() {
            self.status_msg = Some("项目名和文件夹路径不能为空。".into());
            cx.notify();
            return;
        }
        match collect_import_files(std::path::Path::new(&source)) {
            Ok(files) if !files.is_empty() => {
                self.confirm_folder_import(window, name, description, files, cx);
            }
            Ok(_) => {
                self.status_msg = Some("文件夹中没有可导入的文件。".into());
                cx.notify();
            }
            Err(error) => {
                self.status_msg = Some(format!("读取文件夹失败: {error}"));
                cx.notify();
            }
        }
    }

    fn confirm_folder_import(
        &mut self,
        window: &mut Window,
        name: String,
        description: String,
        files: Vec<dto::UploadFile>,
        cx: &mut Context<Self>,
    ) {
        let handle = cx.entity();
        let files = Arc::new(files);
        let count = files.len();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let confirm_files = files.clone();
                let confirm_name = name.clone();
                let confirm_description = description.clone();
                let confirm_handle = handle.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("确认文件夹导入")
                    .message(format!("将上传 {count} 个文件并创建一个新项目。"))
                    .confirm_label("确认并上传")
                    .cancel_label("取消")
                    .on_confirm(move |_, window, cx| {
                        confirm_handle.update(cx, |app, cx| {
                            app.execute_folder_import(
                                confirm_name.clone(),
                                confirm_description.clone(),
                                confirm_files.clone(),
                                cx,
                            )
                        });
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    fn confirm_repo_import(
        &mut self,
        window: &mut Window,
        name: String,
        url: String,
        cx: &mut Context<Self>,
    ) {
        if name.trim().is_empty() || url.trim().is_empty() {
            self.status_msg = Some("项目名和 Git URL 不能为空。".into());
            cx.notify();
            return;
        }
        let handle = cx.entity();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let confirm_handle = handle.clone();
                let confirm_name = name.clone();
                let confirm_url = url.clone();
                let summary_url = url.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("确认 Git 仓库导入")
                    .message(format!("将从 {summary_url} 克隆为新项目。"))
                    .confirm_label("确认并克隆")
                    .cancel_label("取消")
                    .on_confirm(move |_, window, cx| {
                        confirm_handle.update(cx, |app, cx| {
                            app.execute_repo_import(confirm_name.clone(), confirm_url.clone(), cx)
                        });
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    fn prepare_bundle_import(
        &mut self,
        window: &mut Window,
        name: String,
        source: String,
        cx: &mut Context<Self>,
    ) {
        if name.trim().is_empty() || source.trim().is_empty() {
            self.status_msg = Some("项目名和 Bundle 路径不能为空。".into());
            cx.notify();
            return;
        }
        match read_file_limited(std::path::Path::new(&source), MAX_IMPORT_TOTAL_BYTES) {
            Ok(bytes) if !bytes.is_empty() => {
                self.confirm_bundle_import(window, name, Arc::new(bytes), cx);
            }
            Ok(_) => {
                self.status_msg = Some("Bundle 文件为空。".into());
                cx.notify();
            }
            Err(error) => {
                self.status_msg = Some(format!("读取 Bundle 失败: {error}"));
                cx.notify();
            }
        }
    }

    fn confirm_bundle_import(
        &mut self,
        window: &mut Window,
        name: String,
        bytes: Arc<Vec<u8>>,
        cx: &mut Context<Self>,
    ) {
        let handle = cx.entity();
        let size = bytes.len();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let confirm_handle = handle.clone();
                let confirm_name = name.clone();
                let confirm_bytes = bytes.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("确认 Bundle 导入")
                    .message(format!("将上传 {} 后创建新项目。", format_bytes(size)))
                    .confirm_label("确认并上传")
                    .cancel_label("取消")
                    .on_confirm(move |_, window, cx| {
                        confirm_handle.update(cx, |app, cx| {
                            app.execute_bundle_import(
                                confirm_name.clone(),
                                confirm_bytes.clone(),
                                cx,
                            )
                        });
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    fn execute_folder_import(
        &mut self,
        name: String,
        description: String,
        files: Arc<Vec<dto::UploadFile>>,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            match client.import_folder(&name, &description, files).await {
                Ok(result) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.notify(
                            format!(
                                "项目 {} 已导入（{} 个提交）",
                                result.project.name, result.commits
                            ),
                            cx,
                        );
                        app.load_projects(cx);
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.status_msg = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn execute_repo_import(&mut self, name: String, url: String, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.loading = true;
        cx.notify();
        cx.spawn(
            async move |this, cx| match client.import_repo(&name, &url).await {
                Ok(result) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.notify(
                            format!(
                                "项目 {} 已导入（{} 个提交）",
                                result.project.name, result.commits
                            ),
                            cx,
                        );
                        app.load_projects(cx);
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.status_msg = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    fn execute_bundle_import(&mut self, name: String, bytes: Arc<Vec<u8>>, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.loading = true;
        cx.notify();
        cx.spawn(
            async move |this, cx| match client.import_bundle(&name, bytes).await {
                Ok(result) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.notify(
                            format!(
                                "项目 {} 已导入（{} 个提交）",
                                result.project.name, result.commits
                            ),
                            cx,
                        );
                        app.load_projects(cx);
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.status_msg = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            },
        )
        .detach();
    }

    fn confirm_export(
        &mut self,
        window: &mut Window,
        project: String,
        format: &str,
        destination: String,
        cx: &mut Context<Self>,
    ) {
        let format = format.to_string();
        let path = if destination.trim().is_empty() {
            if format == "zip" {
                "project.zip".to_string()
            } else {
                "project.bundle".to_string()
            }
        } else {
            destination
        };
        let handle = cx.entity();
        let host = self.overlay_host.clone();
        host.update(cx, |host, cx| {
            host.open_modal(window, cx, move |close, _window, _cx| {
                let confirm_handle = handle.clone();
                let confirm_project = project.clone();
                let confirm_format = format.clone();
                let confirm_path = path.clone();
                let summary_format = format.clone();
                let summary_path = path.clone();
                let close_confirm = close.clone();
                let close_cancel = close.clone();
                ConfirmModal::new()
                    .title("确认导出")
                    .message(format!(
                        "将生成 {} 并写入 {}。",
                        summary_format.to_uppercase(),
                        summary_path
                    ))
                    .confirm_label("确认并保存")
                    .cancel_label("取消")
                    .on_confirm(move |_, window, cx| {
                        confirm_handle.update(cx, |app, cx| {
                            app.execute_export(
                                confirm_project.clone(),
                                confirm_format.clone(),
                                confirm_path.clone(),
                                cx,
                            )
                        });
                        close_confirm(window, cx);
                    })
                    .on_cancel(move |_, window, cx| close_cancel(window, cx))
                    .into_any_element()
            });
        });
    }

    fn execute_export(
        &mut self,
        project: String,
        format: String,
        path: String,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = if format == "zip" {
                client.export_zip(&project).await
            } else {
                client.export_bundle(&project).await
            };
            match result {
                Ok(bytes) => {
                    let write = std::fs::write(&path, bytes);
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        match write {
                            Ok(_) => app.notify(format!("导出已保存到 {path}"), cx),
                            Err(error) => {
                                app.status_msg = Some(format!("保存导出失败: {error}"));
                                cx.notify();
                            }
                        }
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        app.loading = false;
                        app.status_msg = Some(Self::friendly_api_error(&error));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
}

impl Render for XWikiApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ⌘K / ⌘P / ⌘⇧T dispatch here; bindings live on the root element so
        // they register during paint (Window::on_action requires it).
        let palette_weak = cx.weak_entity();
        let quick_weak = cx.weak_entity();
        let theme_weak = cx.weak_entity();
        let save_weak = cx.weak_entity();
        let ctx_weak = cx.weak_entity();
        let proj_weak = cx.weak_entity();
        let export_weak = cx.weak_entity();
        let archive_weak = cx.weak_entity();
        let rename_weak = cx.weak_entity();
        let delete_weak = cx.weak_entity();
        let edit_weak = cx.weak_entity();
        let doc_rename_weak = cx.weak_entity();
        let doc_delete_weak = cx.weak_entity();

        // Plan §3.1 keyboard model — Esc closes the innermost layer first.
        // Dialogs own their own Escape binding (gpui-component), so while a
        // dialog is open its CancelDialog consumes the key before this fires.
        let title = self.window_title();
        if title != self.last_title {
            window.set_window_title(&title);
            self.last_title = title;
        }
        // gpui-component's `Root` used to paint the theme background; guise
        // has no equivalent, so the root view paints it itself.
        let root_theme = tokens::Cobalt::from_theme(theme(cx));
        div()
            .id("app-root")
            .size_full()
            .bg(root_theme.paper)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key != "escape" {
                    return;
                }
                if this.quick_open {
                    this.quick_open = false;
                    cx.notify();
                } else if this.search_open {
                    this.search_open = false;
                    cx.notify();
                } else if this.history_open {
                    this.close_history(cx);
                } else if this.editing {
                    if !this.saving {
                        this.request_cancel_edit(window, cx);
                    }
                } else if this.screen_is_workspace() && this.selected_project.is_some() {
                    if this.doc_path.is_some() {
                        this.back_to_document_browser(cx);
                    } else {
                        this.back_to_projects(cx);
                    }
                } else if matches!(
                    this.screen,
                    Screen::Settings
                        | Screen::ApiReference
                        | Screen::Audit
                        | Screen::Backlinks
                        | Screen::Attachments
                ) {
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
            .on_action(move |_: &SaveEditor, _window, cx| {
                let _ = save_weak.update(cx, |app, cx| {
                    if app.editing {
                        app.save_edit(cx);
                    }
                });
            })
            .on_action(move |action: &TreeContextAction, _window, cx| {
                let _ = ctx_weak.update(cx, |app, cx| {
                    if app.editing {
                        return;
                    }
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
            .on_action(move |_: &ProjectExportAction, window, cx| {
                let _ = export_weak.update(cx, |app, cx| {
                    app.open_export_dialog(window, cx);
                });
            })
            .on_action(move |action: &ProjectArchiveAction, window, cx| {
                let project_id = action.project_id.clone();
                let archived = action.archived;
                let _ = archive_weak.update(cx, |app, cx| {
                    app.confirm_archive_project(window, cx, project_id, archived);
                });
            })
            .on_action(move |action: &ProjectRenameAction, window, cx| {
                let project_id = action.project_id.clone();
                let current_name = action.current_name.clone();
                let _ = rename_weak.update(cx, |app, cx| {
                    app.rename_project(window, cx, project_id, current_name);
                });
            })
            .on_action(move |action: &ProjectDeleteAction, window, cx| {
                let project_id = action.project_id.clone();
                let project_name = action.project_name.clone();
                let _ = delete_weak.update(cx, |app, cx| {
                    app.confirm_delete_project(window, cx, project_id, project_name);
                });
            })
            .on_action(move |action: &EditDocAction, _window, cx| {
                let _ = edit_weak.update(cx, |app, cx| {
                    if app.editing {
                        return;
                    }
                    app.pending_edit = Some(action.path.clone());
                    app.open_doc(&action.path, cx);
                });
            })
            .on_action(move |action: &DocRenameAction, window, cx| {
                let path = action.path.clone();
                let is_dir = action.is_dir;
                let _ = doc_rename_weak.update(cx, |app, cx| {
                    app.confirm_rename_doc(window, cx, path, is_dir);
                });
            })
            .on_action(move |action: &DocDeleteAction, window, cx| {
                let path = action.path.clone();
                let is_dir = action.is_dir;
                let _ = doc_delete_weak.update(cx, |app, cx| {
                    app.confirm_delete_doc(window, cx, path, is_dir);
                });
            })
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .relative()
                    .child(match self.screen {
                        Screen::Login => self.render_login(cx).into_any_element(),
                        Screen::Settings
                        | Screen::Workspace
                        | Screen::ApiReference
                        | Screen::Audit
                        | Screen::Backlinks
                        | Screen::Attachments => self
                            .render_authenticated_shell(window, cx)
                            .into_any_element(),
                    })
                    .child(if self.quick_open {
                        self.render_quick_open(cx).into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if self.search_open {
                        self.render_project_search(cx).into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
    }
}

fn collect_import_files(root: &std::path::Path) -> Result<Vec<dto::UploadFile>, String> {
    if !std::fs::symlink_metadata(root)
        .map_err(|error| error.to_string())?
        .is_dir()
    {
        return Err("路径不是文件夹".into());
    }
    let mut pending = vec![(root.to_path_buf(), String::new())];
    let mut files = Vec::new();
    let mut total_bytes = 0usize;
    let mut directories_seen = 0usize;
    while let Some((directory, relative)) = pending.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|error| error.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".git" || name == ".DS_Store" {
                continue;
            }
            let child_relative = if relative.is_empty() {
                name
            } else {
                format!("{relative}/{name}")
            };
            if file_type.is_dir() {
                directories_seen = directories_seen.saturating_add(1);
                if directories_seen > MAX_IMPORT_DIRECTORIES {
                    return Err(format!(
                        "文件夹层级项目过多，超过 {} 个目录限制",
                        MAX_IMPORT_DIRECTORIES
                    ));
                }
                pending.push((path, child_relative));
            } else if file_type.is_file() {
                if files.len() >= MAX_IMPORT_FILES {
                    return Err(format!("文件数量超过 {} 个限制", MAX_IMPORT_FILES));
                }
                let content = read_file_limited(&path, MAX_IMPORT_FILE_BYTES)
                    .map_err(|error| format!("文件 {child_relative}: {error}"))?;
                total_bytes = total_bytes.saturating_add(content.len());
                if total_bytes > MAX_IMPORT_TOTAL_BYTES {
                    return Err(format!(
                        "导入内容超过 {} MiB 总大小限制",
                        MAX_IMPORT_TOTAL_BYTES / (1 << 20)
                    ));
                }
                files.push(dto::UploadFile {
                    path: child_relative,
                    content,
                });
            }
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn read_file_limited(path: &std::path::Path, max_bytes: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut content = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut content)
        .map_err(|error| error.to_string())?;
    if content.len() > max_bytes {
        return Err(format!("文件超过 {} MiB 限制", max_bytes / (1 << 20)));
    }
    Ok(content)
}

fn folder_path_prompt_options() -> PathPromptOptions {
    PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("选择要导入的文件夹".into()),
    }
}

fn import_folder_picker_button(folder: Entity<TextInput>, handle: Entity<XWikiApp>) -> Button {
    Button::new("choose-import-folder", "选择文件夹")
        .variant(Variant::Outline)
        .radius(Size::Sm)
        .left_section(Icon::new(IconName::FolderOpen).size(Size::Sm))
        .on_click(move |_, window, cx| {
            let selected = cx.prompt_for_paths(folder_path_prompt_options());
            let window_handle = window.window_handle();
            let folder = folder.clone();
            let handle = handle.clone();
            cx.spawn(async move |cx| match selected.await {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.into_iter().next() {
                        let value = path.to_string_lossy().to_string();
                        let _ = cx.update_window(window_handle, |_, _window, cx| {
                            folder.update(cx, |state, cx| {
                                state.set_text(&value, cx);
                            });
                        });
                    }
                }
                Ok(Err(error)) => {
                    let _ = handle.update(cx, |app, cx| {
                        app.status_msg = Some(format!("打开文件夹选择器失败: {error}"));
                        cx.notify();
                    });
                }
                Ok(Ok(None)) | Err(_) => {}
            })
            .detach();
        })
}

#[cfg(test)]
mod import_path_prompt_tests {
    #[cfg(unix)]
    use super::collect_import_files;
    use super::{
        TokenDraft, XWikiApp, draft_base_revision, folder_path_prompt_options, history_page_offset,
        merge_history_page, read_file_limited,
    };
    use crate::api::dto::{Commit, CommitListResponse};

    #[test]
    fn project_import_picker_selects_one_directory() {
        let options = folder_path_prompt_options();

        assert!(!options.files);
        assert!(options.directories);
        assert!(!options.multiple);
        assert!(options.prompt.is_some());
    }

    #[test]
    fn move_target_validation_rejects_unsafe_paths() {
        assert!(XWikiApp::move_target_error("docs/a.md", "").is_some());
        assert!(XWikiApp::move_target_error("docs/a.md", "../a.md").is_some());
        assert!(XWikiApp::move_target_error("docs/a.md", "/abs.md").is_some());
        assert!(XWikiApp::move_target_error("docs/a.md", "docs/a.md").is_some());
        assert!(XWikiApp::move_target_error("docs", "docs/archive").is_some());
        assert!(XWikiApp::move_target_error("a.md", "a.md/x").is_some());
        assert!(XWikiApp::move_target_error("docs/a.md", "archive/a.md").is_none());
    }

    #[test]
    fn move_path_mapping_follows_moved_subtree() {
        // A moved file remaps only itself.
        assert_eq!(
            XWikiApp::mapped_path("docs/a.md", "archive/a.md", false, "docs/a.md"),
            Some("archive/a.md".to_string())
        );
        assert_eq!(
            XWikiApp::mapped_path("docs/a.md", "archive/a.md", false, "docs/b.md"),
            None
        );
        // A moved directory remaps its subtree but not look-alike prefixes.
        assert_eq!(
            XWikiApp::mapped_path("docs", "archive", true, "docs/a.md"),
            Some("archive/a.md".to_string())
        );
        assert_eq!(
            XWikiApp::mapped_path("docs", "archive", true, "docs/sub/x.md"),
            Some("archive/sub/x.md".to_string())
        );
        assert_eq!(
            XWikiApp::mapped_path("docs", "archive", true, "docs2/a.md"),
            None
        );
        assert_eq!(
            XWikiApp::mapped_path("docs", "archive", true, "docs"),
            Some("archive".to_string())
        );
    }

    #[test]
    fn bounded_file_reader_stops_before_loading_unbounded_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.bin");
        std::fs::write(&path, b"1234").unwrap();

        assert!(read_file_limited(&path, 3).is_err());
        assert_eq!(read_file_limited(&path, 4).unwrap(), b"1234");
    }

    #[test]
    fn history_pagination_resets_then_appends_and_keeps_has_more() {
        let commit = |sha: &str| Commit {
            sha: sha.into(),
            message: sha.into(),
            author: "admin".into(),
            date: "2026-08-13T00:00:00Z".into(),
        };
        let mut commits = vec![commit("stale")];

        let has_more = merge_history_page(
            &mut commits,
            CommitListResponse {
                commits: vec![commit("a")],
                has_more: true,
            },
            true,
        );
        assert_eq!(
            commits.iter().map(|c| c.sha.as_str()).collect::<Vec<_>>(),
            ["a"]
        );
        assert!(has_more);
        assert_eq!(history_page_offset(false, commits.len()), 1);

        let has_more = merge_history_page(
            &mut commits,
            CommitListResponse {
                commits: vec![commit("b")],
                has_more: false,
            },
            false,
        );
        assert_eq!(
            commits.iter().map(|c| c.sha.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(!has_more);
        assert_eq!(history_page_offset(true, commits.len()), 0);
    }

    #[test]
    fn token_draft_starts_unscoped_and_only_accepts_known_permissions() {
        let mut draft = TokenDraft::default();
        assert_eq!(draft.scope(), "read");
        assert!(draft.projects().is_empty());

        draft.set_scope("write");
        draft.set_project("project-b", true);
        draft.set_project("project-a", true);
        draft.set_project("project-b", false);

        assert_eq!(draft.scope(), "write");
        assert_eq!(draft.projects(), vec!["project-a".to_string()]);
        draft.set_scope("owner");
        assert_eq!(draft.scope(), "write");
    }

    #[test]
    fn persisted_draft_keeps_original_revision_for_conflict_detection() {
        assert_eq!(draft_base_revision("server-rev", ""), "server-rev");
        assert_eq!(draft_base_revision("server-rev", "draft-rev"), "draft-rev");
    }

    #[cfg(unix)]
    #[test]
    fn folder_import_skips_symlinked_directories() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        symlink(dir.path(), nested.join("loop")).unwrap();

        let files = collect_import_files(dir.path()).unwrap();
        assert!(files.is_empty());
    }
}

fn is_markdown_import_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

fn join_document_import_path(base: &str, relative: &str) -> String {
    let relative = relative.trim_matches('/').replace('\\', "/");
    if base.trim_matches('/').is_empty() {
        relative
    } else if relative.is_empty() {
        base.trim_matches('/').to_string()
    } else {
        format!("{}/{}", base.trim_matches('/'), relative)
    }
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn login_error_message(error: &crate::api::ApiError) -> String {
    let message = match error.code.as_str() {
        "invalid_credentials" => "用户名或密码不正确，请检查后重试。".to_string(),
        "account_disabled" => "账号已停用，请联系管理员。".to_string(),
        "network_error" => "无法连接服务，请检查服务地址和网络。".to_string(),
        "authentication_required" | "invalid_token" => {
            "认证失败，请检查账号信息和服务地址。".to_string()
        }
        _ if error.status >= 500 => "服务暂时不可用，请稍后重试。".to_string(),
        _ => {
            let detail = error.message.trim();
            if detail.is_empty() {
                "登录失败，请稍后重试。".to_string()
            } else {
                format!("登录失败：{}", tokens::truncate(detail, 160))
            }
        }
    };
    if let Some(request_id) = error.request_id.as_deref() {
        format!("{message} · 请求 ID {}", tokens::truncate(request_id, 32))
    } else {
        message
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
    let host = handle.read(cx).overlay_host.clone();
    host.update(cx, |host, cx| {
        host.open_modal(window, cx, move |close, _window, cx| {
            let name_state = cx.new(|cx| TextInput::new(cx).placeholder("docs-site"));
            let desc_state = cx.new(|cx| TextInput::new(cx).placeholder("项目描述（可选）"));
            let theme = tokens::Cobalt::from_theme(theme(cx));

            let cancel_handle = handle.clone();
            let close_cancel = close.clone();
            let cancel = Button::new("cancel-project", "取消")
                .variant(Variant::Subtle)
                .size(Size::Xs)
                .left_section(Icon::new(IconName::Close).size(Size::Sm))
                .on_click(move |_, window, cx| {
                    cancel_handle.update(cx, |app, cx| {
                        app.project_action = None;
                        cx.notify();
                    });
                    close_cancel(window, cx);
                });

            let create_name = name_state.clone();
            let create_desc = desc_state.clone();
            let create_client = client.clone();
            let app_handle = handle.clone();
            let action_handle = handle.clone();
            let close_create = close.clone();
            let create = Button::new("create-project", "创建")
                .variant(Variant::Filled)
                .size(Size::Xs)
                .left_section(Icon::new(IconName::Folder).size(Size::Sm))
                .on_click(move |_, window, cx| {
                    let name = create_name.read(cx).text().to_string();
                    if name.trim().is_empty() {
                        return;
                    }
                    let desc = create_desc.read(cx).text().to_string();
                    let c = create_client.clone();
                    let h = app_handle.clone();
                    action_handle.update(cx, |app, cx| {
                        app.project_action = Some("__new-project__".into());
                        app.status_msg = None;
                        cx.notify();
                    });
                    cx.spawn(async move |cx| {
                        match c.create_project(name.trim(), desc.trim()).await {
                            Ok(p) => {
                                let _ = h.update(cx, |app, cx| {
                                    app.project_action = None;
                                    app.projects.push(ProjectRow::from_dto(&p));
                                    app.notify("项目已创建".into(), cx);
                                    cx.notify();
                                });
                            }
                            Err(e) => {
                                let _ = h.update(cx, |app, cx| {
                                    app.project_action = None;
                                    app.status_msg = Some(format!("创建项目失败: {e}"));
                                    cx.notify();
                                });
                            }
                        }
                    })
                    .detach();
                    close_create(window, cx);
                });

            Modal::new()
                .title("新建项目")
                .on_close(move |_ev, window, cx| close(window, cx))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .w_full()
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.ink_3)
                                .child("名称"),
                        )
                        .child(div().w_full().child(name_state.clone()))
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.ink_3)
                                .child("描述"),
                        )
                        .child(div().w_full().child(desc_state.clone())),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .justify_end()
                        .w_full()
                        .child(cancel)
                        .child(create),
                )
                .into_any_element()
        });
    });
}
