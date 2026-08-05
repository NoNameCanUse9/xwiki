#![cfg_attr(target_family = "wasm", no_main)]

use gpui::*;
use gpui_component::*;

mod app;

use app::XWikiApp;

fn main() {
    gpui_platform::application().run(move |cx| {
        // Must be called before any GPUI Component features are used.
        gpui_component::init(cx);

        // Load the Cobalt theme (cool engineered paper, one electric-blue
        // signal accent, hairlines over shadows) as the active light theme.
        let registry = ThemeRegistry::global_mut(cx);
        if let Err(err) = registry.load_themes_from_str(include_str!("themes/cobalt.json")) {
            eprintln!("cobalt theme failed to load: {err}");
        }
        if let Some(cobalt) = registry.themes().get("Cobalt Light").cloned() {
            Theme::global_mut(cx).light_theme = cobalt;
        }
        Theme::change(ThemeMode::Light, None, cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| XWikiApp::new(window, cx));
                // The first level on the window must be a Root.
                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}
