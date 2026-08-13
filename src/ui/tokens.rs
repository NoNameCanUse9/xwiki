//! Cobalt design tokens for the desktop app.
//!
//! Source of truth: `web/src/index.css` (Hallmark · Cobalt). Primitive oklch
//! values from the web are converted to sRGB hex and exposed through
//! [`guise::theme::Theme`] semantic colors (see [`cobalt_light`] /
//! [`cobalt_dark`]) plus the semantic accessors below for the few tokens
//! that have no slot.

use gpui::{rgb, rgba, Hsla, Rgba};
use guise::theme::{rgb as theme_rgb, Theme};

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
/// Reading measure (web `max-w` for prose).
pub const MEASURE: f32 = 720.0;
/// Keep paths and user-provided labels from expanding fixed desktop regions.
pub fn truncate(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let value: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{value}…")
    } else {
        value
    }
}

// ---- Login geometry ----
pub const LOGIN_PANEL: f32 = 380.0;

/// The Cobalt light theme (web `paper` / `ink` / electric cobalt accent).
/// Installed once at startup with `Theme::init`; views read it via
/// `guise::theme::theme(cx)`.
pub fn cobalt_light() -> Theme {
    Theme::light()
        .with_primary(theme_rgb(0x00, 0x76, 0xed))
        .with_body(theme_rgb(0xf8, 0xfa, 0xfd))
        .with_surface(theme_rgb(0xef, 0xf3, 0xf6))
        .with_surface_hover(theme_rgb(0xed, 0xf4, 0xfd))
        .with_text(theme_rgb(0x19, 0x20, 0x29))
        .with_dimmed(theme_rgb(0x70, 0x75, 0x7c))
        .with_border(theme_rgb(0xdc, 0xe0, 0xe5))
        .with_danger(theme_rgb(0xcc, 0x27, 0x2e))
        .with_success(theme_rgb(0x2d, 0xa4, 0x4e))
}

/// The Cobalt dark theme (web graphite register).
pub fn cobalt_dark() -> Theme {
    Theme::dark()
        .with_primary(theme_rgb(0x49, 0x92, 0xf2))
        .with_body(theme_rgb(0x06, 0x09, 0x0e))
        .with_surface(theme_rgb(0x0d, 0x11, 0x17))
        .with_surface_hover(theme_rgb(0x12, 0x19, 0x22))
        .with_text(theme_rgb(0xe8, 0xeb, 0xf1))
        .with_dimmed(theme_rgb(0x8d, 0x93, 0x9a))
        .with_border(theme_rgb(0x20, 0x24, 0x2b))
        .with_danger(theme_rgb(0xe5, 0x55, 0x51))
        .with_success(theme_rgb(0x3f, 0xb9, 0x50))
}

/// Semantic Cobalt palette mapped from the active guise theme.
///
/// `graphite` (the code-card surface) and friends have no theme slot and
/// live here.
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
    /// Success / connected.
    pub success: Hsla,
    /// Floating surface (popover / overlay).
    pub popover: Hsla,
}

#[allow(dead_code)]
impl Cobalt {
    pub fn from_theme(theme: &Theme) -> Self {
        let dark = theme.scheme.is_dark();
        // Web graphite: light oklch(22% 0.016 260) / dark oklch(18% 0.016 258).
        let (graphite, graphite_2) = if dark {
            (rgb(0x0d1219), rgb(0x151b23))
        } else {
            (rgb(0x161b22), rgb(0x21272f))
        };
        Self {
            paper: theme.body().hsla(),
            paper_2: theme.surface().hsla(),
            rule: theme.border().hsla(),
            ink: theme.text().hsla(),
            ink_2: theme.text().hsla(),
            ink_3: theme.dimmed().hsla(),
            accent: theme.primary().hsla(),
            accent_ink: theme.primary().contrasting().hsla(),
            surface_accent: theme.surface_hover().hsla(),
            danger: theme.danger().hsla(),
            danger_ink: theme.danger().contrasting().hsla(),
            success: theme.success().hsla(),
            popover: theme.surface().hsla(),
            graphite: graphite.into(),
            graphite_soft: graphite_2.into(),
        }
    }
}

/// White-tinted reads used on the graphite code card (mode-independent).
#[allow(dead_code)]
pub fn card_title() -> Rgba {
    rgba(0xffffffd9)
}
#[allow(dead_code)]
pub fn card_muted() -> Rgba {
    rgba(0xffffff66)
}
