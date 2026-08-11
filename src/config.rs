//! Client-side persistent settings (theme etc.). Passwords are never
//! persisted; a server-issued session cookie may survive an app restart.

use std::path::PathBuf;

use gpui_component::ThemeMode;
use serde::{Deserialize, Serialize};

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/agentdocs-client")
}

/// Writes a file atomically (tmp + rename) so a crash mid-write never leaves
/// a truncated config. On Unix the tmp file gets `mode` before the rename,
/// closing the umask window where a credential file could be world-readable.
fn write_atomic(path: &std::path::Path, contents: &str, mode: u32) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
    }
    std::fs::rename(&tmp, path)
}

/// Serializes tests that mutate `HOME` (see `cli::tests::isolated_home` and
/// `config::tests`): `set_var` is process-global, so parallel tests would
/// otherwise race on the same config directory.
#[cfg(test)]
pub(crate) static TEST_HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Saved server address (CLI + GUI login prefill).
pub fn load_server() -> String {
    std::fs::read_to_string(config_path().join("server"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "http://127.0.0.1:9090".into())
}

pub fn save_server(url: &str) -> std::io::Result<()> {
    let dir = config_path();
    std::fs::create_dir_all(&dir)?;
    write_atomic(&dir.join("server"), url, 0o644)
}

/// Saved username for login convenience. Passwords are never persisted.
pub fn load_username() -> String {
    std::fs::read_to_string(config_path().join("username"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn save_username(username: &str) {
    let username = username.trim();
    if username.is_empty() {
        return;
    }
    let dir = config_path();
    let _ = std::fs::create_dir_all(&dir);
    let _ = write_atomic(&dir.join("username"), username, 0o644);
}

/// A persisted server session. This contains a server-issued cookie, never a
/// username/password pair; the file is restricted to the current user on Unix.
#[derive(Serialize, Deserialize, Clone)]
pub struct Session {
    pub server: String,
    pub username: String,
    pub cookie: String,
}

pub fn load_session() -> Option<Session> {
    let session = std::fs::read_to_string(config_path().join("session.json"))
        .ok()
        .and_then(|json| serde_json::from_str::<Session>(&json).ok())?;
    if session.server.trim().is_empty() || session.cookie.trim().is_empty() {
        None
    } else {
        Some(session)
    }
}

pub fn save_session(server: &str, username: &str, cookie: &str) {
    if server.trim().is_empty() || cookie.trim().is_empty() {
        return;
    }
    let dir = config_path();
    let _ = std::fs::create_dir_all(&dir);
    let session = Session {
        server: server.trim().to_string(),
        username: username.trim().to_string(),
        cookie: cookie.trim().to_string(),
    };
    if let Ok(json) = serde_json::to_string(&session) {
        let _ = write_atomic(&dir.join("session.json"), &json, 0o600);
    }
}

pub fn clear_session() {
    let _ = std::fs::remove_file(config_path().join("session.json"));
}

/// Returns the persisted theme mode, defaulting to System.
pub fn load_theme() -> ThemeMode {
    match std::fs::read_to_string(config_path().join("theme"))
        .unwrap_or_default()
        .trim()
    {
        "light" => ThemeMode::Light,
        "dark" => ThemeMode::Dark,
        _ => ThemeMode::Light,
    }
}

pub fn save_theme(mode: ThemeMode) {
    let dir = config_path();
    let _ = std::fs::create_dir_all(&dir);
    let value = match mode {
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
    };
    let _ = write_atomic(&dir.join("theme"), value, 0o644);
}

/// Persisted panel layout (plan §0.3: widths + collapse state survive
/// restarts). Defaults match the plan's suggested values.
#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Layout {
    pub projects_rail: f32,
    pub doc_rail: f32,
    pub history: f32,
    pub history_open: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            projects_rail: crate::ui::tokens::PROJECTS_RAIL,
            doc_rail: crate::ui::tokens::DOC_RAIL,
            history: crate::ui::tokens::HISTORY_W,
            history_open: false,
        }
    }
}

pub fn load_layout() -> Layout {
    std::fs::read_to_string(config_path().join("layout.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_layout(layout: &Layout) {
    let dir = config_path();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string(layout) {
        let _ = write_atomic(&dir.join("layout.json"), &json, 0o644);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_roundtrip_and_corrupt_defaults() {
        let _lock = TEST_HOME_LOCK.lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", home.path());
        let l = Layout {
            projects_rail: 300.0,
            doc_rail: 340.0,
            history: 480.0,
            history_open: true,
        };
        save_layout(&l);
        let back = load_layout();
        assert_eq!(back.projects_rail, 300.0);
        assert_eq!(back.doc_rail, 340.0);
        assert_eq!(back.history, 480.0);
        assert!(back.history_open);

        // A corrupt file must fall back to plan defaults, not panic.
        std::fs::write(config_path().join("layout.json"), "not json").unwrap();
        let d = load_layout();
        assert_eq!(d.doc_rail, crate::ui::tokens::DOC_RAIL);
        assert!(!d.history_open);

        save_session("http://server", "alice", "session=abc");
        let session = load_session().unwrap();
        assert_eq!(session.server, "http://server");
        assert_eq!(session.username, "alice");
        assert_eq!(session.cookie, "session=abc");
        save_username("alice");
        assert_eq!(load_username(), "alice");
        clear_session();
        assert!(load_session().is_none());
    }
}
