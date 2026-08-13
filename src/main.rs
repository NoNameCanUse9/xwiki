#![cfg_attr(target_family = "wasm", no_main)]

use gpui::*;

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
        // SAFETY: single-threaded startup, before any other env reads.
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
    }

    // Crash visibility: a panic in the UI thread would otherwise vanish with
    // the window; log it to a file too.
    std::panic::set_hook(Box::new(|info| {
        let log = std::env::var("HOME")
            .map(|h| format!("{h}/.config/xwiki/panic.log"))
            .unwrap_or_else(|_| "/tmp/xwiki-panic.log".into());
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
        // With subcommands the binary acts as the xwiki CLI;
        // `gui` forces the desktop app, bare invocation starts the GUI.
        if args[0] != "gui" {
            std::process::exit(cli::run(args));
        }
    }
    gpui::Application::new().run(move |cx: &mut App| {
        // Install the Cobalt theme before any window opens: every guise
        // component reads it from the gpui global during render.
        // (theme = config::load_theme(), Cobalt built in ui::tokens)
        match config::load_theme() {
            config::ThemeMode::Light => ui::tokens::cobalt_light().init(cx),
            config::ThemeMode::Dark => ui::tokens::cobalt_dark().init(cx),
        }

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
            let start_bounds = cx
                .update(|cx| Bounds::centered(None, size(px(1024.0), px(680.0)), cx))
                .expect("app was released");
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("XWiki".into()),
                        ..Default::default()
                    }),
                    // Plan §0.3: desktop window contract — never smaller than
                    // 960×640 so panels + main content stay usable. The
                    // default gpui size is far larger, so pin a modest
                    // starting size.
                    // (gpui 0.2.2 WindowOptions has no `icon` field; Windows
                    // uses the resource embedded by build.rs, X11 falls back
                    // to the platform default.)
                    window_bounds: Some(WindowBounds::Windowed(start_bounds)),
                    window_min_size: Some(size(px(960.0), px(640.0))),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| XWikiApp::new(window, cx)),
            )
            .expect("Failed to open window")
            .update(cx, |_, window, _| window.activate_window())
            .ok();
        })
        .detach();
    });
}
