//! CLI unit tests: exit codes, usage, config persistence.
//! Network commands need a live server and are covered by the e2e suite.

use super::*;

/// Point HOME at a temp dir so config writes never touch the real one.
/// Holds the shared `TEST_HOME_LOCK` for its lifetime: `set_var` is
/// process-global, so HOME-mutating tests must not run concurrently.
struct IsolatedHome {
    _dir: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
}

fn isolated_home() -> IsolatedHome {
    let lock = crate::config::TEST_HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    // SAFETY: tests run under the shared TEST_HOME_LOCK, serializing env writes.
    unsafe { std::env::set_var("HOME", dir.path()) };
    IsolatedHome {
        _dir: dir,
        _lock: lock,
    }
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

#[test]
fn usage_returns_2() {
    let _h = isolated_home();
    assert_eq!(run(vec![]), 2);
    assert_eq!(run(vec!["bogus".into()]), 2);
    assert_eq!(run(vec!["project".into(), "create".into()]), 2);
}

#[test]
fn config_set_and_show() {
    let _h = isolated_home();
    assert_eq!(
        run(vec![
            "config".into(),
            "set-server".into(),
            "http://x:1".into()
        ]),
        0
    );
    assert_eq!(config::load_server(), "http://x:1");
}

#[test]
fn server_status_network_error_is_6() {
    let _h = isolated_home();
    let code = rt().block_on(async {
        cmd_server(&[
            "status".into(),
            "--server".into(),
            "http://127.0.0.1:1".into(),
        ])
        .await
    });
    assert_eq!(code, 6);
}

#[test]
fn positional_parser_keeps_command_arguments_and_drops_options() {
    let args = vec![
        "update".into(),
        "prj_1".into(),
        "docs/guide.md".into(),
        "--file".into(),
        "guide.md".into(),
        "--message".into(),
        "release notes".into(),
        "--dry-run".into(),
        "--json".into(),
    ];
    assert_eq!(
        positional_args(&args),
        vec!["update", "prj_1", "docs/guide.md"]
    );
    assert_eq!(
        option_value(&args, "--message").as_deref(),
        Some("release notes")
    );
    assert!(has_flag(&args, "--dry-run"));
}

#[test]
fn exit_codes_follow_api_error_contract() {
    let error = |code: &str, status| crate::api::ApiError {
        code: code.into(),
        message: String::new(),
        request_id: None,
        status,
    };
    assert_eq!(exit_code(&error("invalid_query", 400)), 2);
    assert_eq!(exit_code(&error("authentication_required", 401)), 3);
    assert_eq!(exit_code(&error("doc_not_found", 404)), 4);
    assert_eq!(exit_code(&error("page_locked", 409)), 5);
    assert_eq!(exit_code(&error("internal_error", 500)), 6);
}

#[test]
fn history_json_keeps_pagination_metadata() {
    let page = crate::api::dto::CommitListResponse {
        commits: vec![crate::api::dto::Commit {
            sha: "abc123".into(),
            message: "edit guide".into(),
            author: "admin".into(),
            date: "2026-08-13T00:00:00Z".into(),
        }],
        has_more: true,
    };

    let value: serde_json::Value = serde_json::from_str(&history_json(&page)).unwrap();
    assert_eq!(value["commits"][0]["sha"], "abc123");
    assert_eq!(value["has_more"], true);
}
