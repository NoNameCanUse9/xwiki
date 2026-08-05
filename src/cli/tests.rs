//! CLI unit tests: exit codes, usage, config persistence.
//! Network commands need a live server and are covered by the e2e suite.

use super::*;
use std::sync::Once;

static INIT: Once = Once::new();

/// Point HOME at a temp dir so config writes never touch the real one.
fn isolated_home() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", dir.path());
    dir
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
    assert_eq!(run(vec!["config".into(), "set-server".into(), "http://x:1".into()]), 0);
    assert_eq!(config::load_server(), "http://x:1");
}

#[test]
fn server_status_network_error_is_6() {
    let _h = isolated_home();
    let code = rt().block_on(async {
        cmd_server(&["status".into(), "--server".into(), "http://127.0.0.1:1".into()]).await
    });
    assert_eq!(code, 6);
}
