#![cfg_attr(target_family = "wasm", no_main)]

use gpui::*;
use gpui_component::*;

mod api;
mod app;
mod cli;
mod config;
mod ui;

use app::XWikiApp;

actions!(
    app_actions,
    [TogglePalette, ToggleTheme, QuickOpen, SaveEditor]
);

fn main() {
    // WSLg: the native-Wayland + swiftshader Vulkan path renders a blank
    // window for this app, while the X11 (Xwayland) path is verified to
    // paint correctly. Prefer X11 whenever a DISPLAY exists; on Wayland-
    // only desktops (no DISPLAY) nothing changes. ponytail: heuristic, not
    // a config knob — flip it if a real Wayland desktop regresses.
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty()) {
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    // Crash visibility: a panic in the UI thread would otherwise vanish with
    // the window; log it to a file too.
    std::panic::set_hook(Box::new(|info| {
        let log = std::env::var("HOME")
            .map(|h| format!("{h}/.config/agentdocs-client/panic.log"))
            .unwrap_or_else(|_| "/tmp/agentdocs-client-panic.log".into());
        let _ = std::fs::create_dir_all(
            log.rsplit_once('/')
                .map(|(d, _)| d.to_string())
                .unwrap_or_default(),
        );
        let _ = std::fs::write(&log, format!("panic: {info}\n"));
        eprintln!("PANIC: {info}");
    }));

    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        // With subcommands the binary acts as the agentdocs-client CLI;
        // `gui` forces the desktop app, bare invocation starts the GUI.
        if args[0] != "gui" {
            std::process::exit(cli::run(args));
        }
    }
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);
    app.run(move |cx| {
        // Must be called before any GPUI Component features are used.
        gpui_component::init(cx);

        // Load the Cobalt theme (cool engineered paper, one electric-blue
        // signal accent, hairlines over shadows) for both modes.
        let (light, dark) = {
            let registry = ThemeRegistry::global_mut(cx);
            if let Err(err) = registry.load_themes_from_str(include_str!("themes/cobalt.json")) {
                eprintln!("cobalt theme failed to load: {err}");
            }
            let light = registry.themes().get("Cobalt Light").cloned();
            let dark = registry.themes().get("Cobalt Dark").cloned();
            (light, dark)
        };
        let global = Theme::global_mut(cx);
        if let Some(cobalt) = light {
            global.light_theme = cobalt;
        }
        if let Some(cobalt) = dark {
            global.dark_theme = cobalt;
        }
        Theme::change(config::load_theme(), None, cx);

        cx.bind_keys([
            KeyBinding::new("cmd-k", TogglePalette, None),
            KeyBinding::new("ctrl-k", TogglePalette, None),
            KeyBinding::new("cmd-p", QuickOpen, None),
            KeyBinding::new("ctrl-p", QuickOpen, None),
            KeyBinding::new("cmd-shift-t", ToggleTheme, None),
            KeyBinding::new("ctrl-shift-t", ToggleTheme, None),
            KeyBinding::new("cmd-s", SaveEditor, None),
            KeyBinding::new("ctrl-s", SaveEditor, None),
        ]);

        cx.spawn(async move |cx| {
            let start_bounds = cx.update(|cx| {
                Bounds::centered(None, size(px(1024.0), px(680.0)), cx)
            });
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("AgentDocs".into()),
                        ..Default::default()
                    }),
                    // Plan §0.3: desktop window contract — never smaller than
                    // 960×640 so panels + main content stay usable. The
                    // default gpui size is far larger, so pin a modest
                    // starting size.
                    icon: Some(crate::ui::app_icon_rgba()),
                    window_bounds: Some(WindowBounds::Windowed(start_bounds)),
                    window_min_size: Some(size(px(960.0), px(640.0))),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| XWikiApp::new(window, cx));
                    // The first level on the window must be a Root.
                    // Root paints the active theme background dynamically. Do
                    // not capture a one-time color here, otherwise switching
                    // between light and dark modes leaves the surface stale.
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("Failed to open window");
        })
        .detach();
    });
}
