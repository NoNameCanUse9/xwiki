#![cfg_attr(target_family = "wasm", no_main)]

use gpui::*;
use gpui_component::*;

mod app;

use app::XWikiApp;

fn main() {
    gpui_platform::application().run(move |cx| {
        // Must be called before any GPUI Component features are used.
        gpui_component::init(cx);

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
