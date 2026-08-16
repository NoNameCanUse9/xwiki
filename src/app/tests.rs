//! Headless UI regression tests for the folder/file three-dot menus.
//!
//! Every item in those menus (移动 / 重命名 / 导出项目 / 删除) must actually
//! open its dialog. Regression: `OverlayHost` — the entity that renders all
//! modals and toasts — was created but never mounted in the render tree, so
//! every `open_modal` call was silently invisible and the menus appeared to
//! do nothing.
//!
//! These tests run on gpui's test platform: no display, no server. State is
//! injected directly into `XWikiApp`; clicking is simulated at real layout
//! coordinates; dialogs are asserted by their rendered bounds.
//!
//! NOTE: do NOT `use super::*` here. The parent module re-exports
//! `use gpui::*`, which puts gpui's `test` attribute macro in scope; the
//! `#[test]` that `#[gpui::test]` generates would then expand to itself
//! recursively (rustc stack overflow). Import names explicitly.

use super::{Client, ProjectRow, Screen, XWikiApp, audit_page_offset, dto, merge_audit_page};
use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

/// File-row ellipsis menu layout (打开, 编辑, 导出项目, 移动, 重命名, 删除).
const FILE_ITEM_EXPORT: usize = 2;
const FILE_ITEM_MOVE: usize = 3;
const FILE_ITEM_RENAME: usize = 4;
const FILE_ITEM_DELETE: usize = 5;
/// Folder-row ellipsis menu layout (进入目录, 移动, 重命名, 删除).
const DIR_ITEM_MOVE: usize = 1;

/// A logged-in user sitting in the project's document browser with a folder
/// (`docs`) and a file (`docs/README.md`) in the tree. Seed after
/// `add_window_view`.
fn seed_doc_browser(app: &Entity<XWikiApp>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.screen = Screen::Workspace;
            app.client = Some(Client::new("http://127.0.0.1:9"));
            app.selected_project = Some("p1".into());
            app.tree_path = String::new();
            app.tree_entries = vec![
                dto::TreeEntry {
                    name: "docs".into(),
                    r#type: "tree".into(),
                    path: "docs".into(),
                    sha: String::new(),
                },
                dto::TreeEntry {
                    name: "README.md".into(),
                    r#type: "blob".into(),
                    path: "docs/README.md".into(),
                    sha: String::new(),
                },
            ];
            cx.notify();
        });
    });
    draw(cx);
}

/// Holds the shared `TEST_HOME_LOCK` for its lifetime: `set_var` is
/// process-global, so HOME-mutating tests must not run concurrently (see
/// `cli::tests::IsolatedHome` — same contract).
struct TestHome {
    _dir: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
}

/// Install a hermetic HOME so `XWikiApp::new` can't pick up a real session.
fn hermetic_home() -> TestHome {
    let lock = crate::config::TEST_HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().expect("temp home");
    unsafe { std::env::set_var("HOME", dir.path()) };
    TestHome {
        _dir: dir,
        _lock: lock,
    }
}

/// Cobalt must be installed before any guise component renders.
fn install_theme(cx: &mut TestAppContext) {
    cx.update(|cx| crate::ui::tokens::cobalt_dark().init(cx));
}

/// Paint one frame so hit-testing and `debug_bounds` see the current UI.
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
}
///
/// Mirrors guise's `ContextMenu` layout: 4px outer padding, 2px gaps, item
/// height `font * 1.5 + 12`, 220px width. Near the top-left corner of a
/// maximized test window the edge clamp is the identity.
fn menu_item_center(
    cx: &mut VisualTestContext,
    origin: Point<Pixels>,
    index: usize,
) -> Point<Pixels> {
    let font = cx.read(|cx| guise::theme::theme(cx).font_size(guise::theme::Size::Sm));
    let h = font * 1.5 + 12.0;
    let y_offset = px(4.0) + px((index as f32) * (h + 2.0)) + px(h / 2.0);
    Point::new(origin.x + px(110.0), origin.y + y_offset)
}

/// Click the row's ellipsis, then the given menu item.
fn click_row_menu_item(cx: &mut VisualTestContext, row_index: usize, item_index: usize) {
    let selector = match row_index {
        0 => "tree-ellipsis-0",
        1 => "tree-ellipsis-1",
        _ => panic!("test fixture only defines rows 0 and 1"),
    };
    let ellipsis = cx
        .debug_bounds(selector)
        .expect("row ellipsis must be visible");
    let center = ellipsis.center();
    cx.simulate_click(center, Modifiers::none());
    draw(cx);
    let item = menu_item_center(cx, center, item_index);
    cx.simulate_click(item, Modifiers::none());
    draw(cx);
}

/// Close the top modal by clicking the full-viewport backdrop near a corner.
fn close_modal(cx: &mut VisualTestContext) {
    cx.simulate_click(point(px(8.0), px(8.0)), Modifiers::none());
    draw(cx);
}

fn modal_count(app: &Entity<XWikiApp>, cx: &mut VisualTestContext) -> usize {
    cx.read(|cx| app.read(cx).overlay_host.read(cx).modal_count())
}

fn audit_entry(id: &str) -> dto::AuditEntry {
    dto::AuditEntry {
        id: id.into(),
        actor_type: "user".into(),
        actor_id: "u1".into(),
        project_id: "p1".into(),
        action: "change".into(),
        path: String::new(),
        detail: String::new(),
        request_id: String::new(),
        created_at: String::new(),
    }
}

#[test]
fn audit_page_offset_is_zero_on_reset_and_len_after() {
    assert_eq!(audit_page_offset(true, 42), 0);
    assert_eq!(audit_page_offset(false, 42), 42);
    assert_eq!(audit_page_offset(false, 0), 0);
}

#[test]
fn audit_pagination_merge_resets_then_appends() {
    let mut entries = vec![audit_entry("a"), audit_entry("b")];

    // Append page 2: keeps loaded rows, extends them, carries has_more.
    let has_more = merge_audit_page(
        &mut entries,
        dto::AuditResponse {
            entries: vec![audit_entry("c"), audit_entry("d")],
            has_more: true,
        },
        false,
    );
    assert!(has_more, "append page must keep has_more");
    assert_eq!(
        entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        ["a", "b", "c", "d"]
    );

    // Reset (project switch / reload): the list is replaced entirely.
    let has_more = merge_audit_page(
        &mut entries,
        dto::AuditResponse {
            entries: vec![audit_entry("x")],
            has_more: false,
        },
        true,
    );
    assert!(!has_more, "last page must clear has_more");
    assert_eq!(
        entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        ["x"]
    );
}

#[test]
fn delete_gate_accepts_confirm_phrase() {
    // The four characters 「确认删除」 — with or without surrounding space.
    assert!(XWikiApp::delete_confirmation_error("确认删除").is_none());
    assert!(XWikiApp::delete_confirmation_error(" 确认删除 ").is_none());
}

#[test]
fn delete_gate_rejects_wrong_input() {
    // Names, partial phrases and anything else must be rejected.
    assert!(XWikiApp::delete_confirmation_error("README.md").is_some());
    assert!(XWikiApp::delete_confirmation_error("确认").is_some());
    assert!(XWikiApp::delete_confirmation_error("删除").is_some());
    assert!(XWikiApp::delete_confirmation_error("确认删除啊").is_some());
    assert!(XWikiApp::delete_confirmation_error("").is_some());
}

#[gpui::test]
async fn file_row_ellipsis_menu_opens_every_dialog(cx: &mut TestAppContext) {
    let _home = hermetic_home();
    install_theme(cx);
    let (app, cx) = cx.add_window_view(|window, cx| XWikiApp::new(window, cx));
    seed_doc_browser(&app, cx);

    // 移动 — the user's exact flow: ellipsis → item → dialog on screen.
    click_row_menu_item(cx, 1, FILE_ITEM_MOVE);
    assert!(
        cx.debug_bounds("move-modal").is_some(),
        "移动 must open the move dialog"
    );
    assert_eq!(modal_count(&app, cx), 1, "exactly one modal must be open");
    close_modal(cx);
    assert_eq!(
        modal_count(&app, cx),
        0,
        "modal must close on backdrop click"
    );

    // 重命名
    click_row_menu_item(cx, 1, FILE_ITEM_RENAME);
    assert!(
        cx.debug_bounds("rename-modal").is_some(),
        "重命名 must open the rename dialog"
    );
    close_modal(cx);

    // 导出项目
    click_row_menu_item(cx, 1, FILE_ITEM_EXPORT);
    assert!(
        cx.debug_bounds("export-modal").is_some(),
        "导出项目 must open the export dialog"
    );
    close_modal(cx);

    // 删除
    click_row_menu_item(cx, 1, FILE_ITEM_DELETE);
    assert!(
        cx.debug_bounds("delete-modal").is_some(),
        "删除 must open the delete dialog"
    );
}

#[gpui::test]
async fn folder_row_ellipsis_menu_opens_move_dialog(cx: &mut TestAppContext) {
    let _home = hermetic_home();
    install_theme(cx);
    let (app, cx) = cx.add_window_view(|window, cx| XWikiApp::new(window, cx));
    seed_doc_browser(&app, cx);

    click_row_menu_item(cx, 0, DIR_ITEM_MOVE);
    assert!(
        cx.debug_bounds("move-modal").is_some(),
        "folder 移动 must open the move dialog"
    );
}

#[gpui::test]
async fn workspace_project_card_ellipsis_opens_rename_dialog(cx: &mut TestAppContext) {
    // Same OverlayHost path from the project grid: 重命名 on a project card.
    let _home = hermetic_home();
    install_theme(cx);

    let (app, cx) = cx.add_window_view(|window, cx| XWikiApp::new(window, cx));
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.screen = Screen::Workspace;
            app.client = Some(Client::new("http://127.0.0.1:9"));
            app.projects = vec![ProjectRow {
                id: "p1".into(),
                name: "Proj".into(),
                description: "desc".into(),
                updated: "2026-01-01".into(),
                archived: false,
            }];
            cx.notify();
        });
    });
    draw(cx);

    let ellipsis = cx
        .debug_bounds("project-menu-btn-p1")
        .expect("project card ellipsis must be visible");
    let center = ellipsis.center();
    cx.simulate_click(center, Modifiers::none());
    draw(cx);

    // Project menu: 打开项目(0) 归档项目(1) 重命名(2) 删除项目(3).
    let rename = menu_item_center(cx, center, 2);
    cx.simulate_click(rename, Modifiers::none());
    draw(cx);

    assert!(
        cx.debug_bounds("project-rename-modal").is_some(),
        "project 重命名 must open the rename dialog"
    );
}
