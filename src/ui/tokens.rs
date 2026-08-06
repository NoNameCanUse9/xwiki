//! Cobalt design tokens for the desktop app.
//!
//! Source of truth: `web/src/index.css` (Hallmark · Cobalt). Primitive oklch
//! values from the web are converted to sRGB hex (`scripts` / one-off) and
//! exposed through `gpui_component::Theme` slots (see `themes/cobalt.json`)
//! plus the semantic accessors below for the few tokens that have no slot.

use gpui::{rgb, rgba, Hsla, Rgba};
use gpui_component::Theme;

// ---- Radius (ruler-drawn, never pills) ----
pub const RADIUS_SMALL: f32 = 4.0;
pub const RADIUS: f32 = 6.0;

/// Reserved spec constants for upcoming views (code-card radius, spacing
/// grid). The `px_*`/`gap_*` gpui-component helpers already implement the
/// 4px grid, so views use those; these document the scale.
#[allow(dead_code)]
pub mod spec {
    pub const RADIUS_LARGE: f32 = 10.0;

    pub const SPACE_1: f32 = 4.0;
    pub const SPACE_2: f32 = 8.0;
    pub const SPACE_3: f32 = 12.0;
    pub const SPACE_4: f32 = 16.0;
    pub const SPACE_5: f32 = 20.0;
    pub const SPACE_6: f32 = 24.0;
    pub const SPACE_8: f32 = 32.0;
    pub const SPACE_10: f32 = 40.0;
}

// ---- Type scale (px) ----
/// Mono machine-readout labels (web `mono-label`: 0.6875rem).
pub const FONT_SIZE_LABEL: f32 = 11.0;
/// Body copy (web body: 0.9375rem).
pub const FONT_SIZE_BODY: f32 = 15.0;
/// Display voice for page/brand titles.
pub const FONT_SIZE_DISPLAY: f32 = 24.0;

// ---- Font families (system-first + fallback) ----
// GPUI resolves a family name and falls back to the platform default when
// missing; `TextStyle::font_fallbacks` is not reachable through the `Styled`
// helpers, so we pick the best installed face per platform here.
#[cfg(target_os = "windows")]
pub const FONT_BODY: &str = "Segoe UI";
#[cfg(target_os = "macos")]
pub const FONT_BODY: &str = ".SystemUIFont";
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub const FONT_BODY: &str = "Inter";

#[cfg(target_os = "windows")]
pub const FONT_DISPLAY: &str = "Space Grotesk";
#[cfg(target_os = "macos")]
pub const FONT_DISPLAY: &str = "Space Grotesk";
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub const FONT_DISPLAY: &str = "Space Grotesk";

/// Mono register: JetBrains Mono preferred, GPUI falls back otherwise.
pub const FONT_MONO: &str = "JetBrains Mono";

#[cfg(target_os = "macos")]
pub const MOD_KEY: &str = "⌘";
#[cfg(not(target_os = "macos"))]
pub const MOD_KEY: &str = "Ctrl";

// ---- Panel geometry (plan §0.3: defaults + drag ranges) ----
pub const TOOLBAR_H: f32 = 44.0;
pub const STATUS_H: f32 = 26.0;

/// Workspace project rail: default 260, drag 220–360.
pub const PROJECTS_RAIL: f32 = 260.0;
pub const PROJECTS_RAIL_MIN: f32 = 220.0;
pub const PROJECTS_RAIL_MAX: f32 = 360.0;

/// Document tree rail: default 280, drag 240–400.
pub const DOC_RAIL: f32 = 280.0;
pub const DOC_RAIL_MIN: f32 = 240.0;
pub const DOC_RAIL_MAX: f32 = 400.0;

/// History context panel: default 360, drag 300–520.
pub const HISTORY_W: f32 = 360.0;
pub const HISTORY_W_MIN: f32 = 300.0;
pub const HISTORY_W_MAX: f32 = 520.0;

/// Draggable divider: 7px hit area around a 1px hairline.
pub const SPLITTER_HIT: f32 = 7.0;
/// Uniform project card height keeps metadata and actions aligned.
pub const CARD_HEIGHT: f32 = 176.0;
pub const CARD_MIN_WIDTH: f32 = 280.0;
pub const CARD_MAX_WIDTH: f32 = 380.0;
pub const PROJECT_GRID_MAX: f32 = 1560.0;
/// Compact project rail used below the desktop workspace breakpoint.
pub const PROJECTS_RAIL_COMPACT: f32 = 72.0;
/// Reading measure (web `max-w` for prose).
pub const MEASURE: f32 = 720.0;
/// Mono meta column (numstat).
pub const NUMSTAT_W: f32 = 56.0;

// ---- Login geometry ----
pub const LOGIN_WIDTH: f32 = 1024.0;
pub const LOGIN_GAP: f32 = 64.0;
pub const LOGIN_PANEL: f32 = 380.0;
pub const LOGIN_TEXT: f32 = 420.0;

/// Semantic Cobalt palette mapped from the active gpui-component theme.
///
/// Most slots are already backed by `themes/cobalt.json`; `graphite` (the
/// code-card surface) and friends have no slot and live here.
///
/// `#[allow(dead_code)]`: only `graphite`/`graphite_soft` are consumed so
/// far; views still read `theme.*` directly. The remaining fields are the
/// migration target for the next pass (semantic names over raw theme reads).
#[allow(dead_code)]
#[derive(Clone)]
pub struct Cobalt {
    /// App background (web `paper`).
    pub paper: Hsla,
    /// Elevated / sidebar surface (web `paper-2`).
    pub paper_2: Hsla,
    /// Hairline rule (web `rule`).
    pub rule: Hsla,
    /// Strong text (web `ink`).
    pub ink: Hsla,
    /// Secondary text (web `ink-2`).
    pub ink_2: Hsla,
    /// Muted text (web `ink-3`).
    pub ink_3: Hsla,
    /// The one signal color.
    pub accent: Hsla,
    /// Text on accent surfaces.
    pub accent_ink: Hsla,
    /// Hover surface (web `surface-accent`) — never a full flood.
    pub surface_accent: Hsla,
    /// Code card surface (light: dark card on paper; dark: near-paper).
    pub graphite: Hsla,
    /// Softer text on the code card.
    pub graphite_soft: Hsla,
    /// Destructive / error.
    pub danger: Hsla,
    /// Text on danger surfaces.
    pub danger_ink: Hsla,
}

impl Cobalt {
    pub fn from_theme(theme: &Theme) -> Self {
        let dark = theme.is_dark();
        // Web graphite: light oklch(22% 0.016 260) / dark oklch(18% 0.016 258).
        let (graphite, graphite_2) = if dark {
            (rgb(0x0d1219), rgb(0x151b23))
        } else {
            (rgb(0x161b22), rgb(0x21272f))
        };
        Self {
            paper: theme.background,
            paper_2: theme.sidebar,
            rule: theme.border,
            ink: theme.foreground,
            ink_2: theme.sidebar_foreground,
            ink_3: theme.muted_foreground,
            accent: theme.accent,
            accent_ink: theme.accent_foreground,
            surface_accent: theme.list_hover,
            danger: theme.danger,
            danger_ink: theme.danger_foreground,
            graphite: graphite.into(),
            graphite_soft: graphite_2.into(),
        }
    }
}

/// White-tinted reads used on the graphite code card (mode-independent).
pub fn card_title() -> Rgba {
    rgba(0xffffffd9)
}
pub fn card_muted() -> Rgba {
    rgba(0xffffff66)
}
pub fn card_dot() -> Rgba {
    rgba(0xffffff26)
}
pub fn card_rule() -> Rgba {
    rgba(0xffffff1a)
}
/// Success green on the graphite card (web `#4ade80`).
pub fn card_ok() -> Rgba {
    rgb(0x4ade80)
}
