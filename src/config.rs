//! Client-side persistent settings (theme etc). Credentials never go here —
//! sessions live in the reqwest cookie jar for the process lifetime.

use std::path::PathBuf;

use gpui_component::ThemeMode;
use serde::{Deserialize, Serialize};

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/agentdocs-client")
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

pub fn save_server(url: &str) {
    let dir = config_path();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("server"), url);
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
    let _ = std::fs::write(dir.join("theme"), value);
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
        let _ = std::fs::write(dir.join("layout.json"), json);
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
    }
}
