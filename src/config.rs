//! Client-side persistent settings (theme etc). Credentials never go here —
//! sessions live in the reqwest cookie jar for the process lifetime.

use std::path::PathBuf;

use gpui_component::ThemeMode;

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/agentdocs-client")
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
