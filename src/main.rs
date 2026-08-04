#![cfg_attr(target_family = "wasm", no_main)]

use gpui::{App, Bounds, Menu, WindowBounds, WindowOptions, prelude::*, px, size};

mod app;
mod components;

use app::XWikiApp;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.set_menus(vec![Menu {
            name: "XWiki".into(),
            disabled: false,
            items: vec![],
        }]);

        cx.init_colors();

        let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);

        let window = cx
            .open_window(
                WindowOptions {
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("XWiki GPUI App".into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_window, cx| cx.new(|_cx| XWikiApp {}),
            )
            .unwrap();

        window
            .update(cx, |_view, _window, cx| {
                cx.activate(true);
            })
            .unwrap();
    });
}
