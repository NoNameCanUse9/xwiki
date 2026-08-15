//! Document workspace (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::InteractiveElement;
use gpui::*;
use guise::theme::{theme, ColorName, Size};
use guise::{tooltip, ActionIcon, Button, ContextMenu, Icon, IconName, Variant};

use crate::app::XWikiApp;
use crate::config;
use crate::ui::{mono_label, split_pane, tokens};

/// Semantic colors consumed by the document views, mirroring the old
/// `theme.*` slot reads against the Cobalt palette.
#[derive(Clone)]
struct DocTheme {
    background: Hsla,
    sidebar: Hsla,
    foreground: Hsla,
    muted_foreground: Hsla,
    border: Hsla,
    accent: Hsla,
    danger: Hsla,
    list_hover: Hsla,
    list_active: Hsla,
    skeleton: Hsla,
}

impl DocTheme {
    fn from_cx(cx: &mut Context<XWikiApp>) -> Self {
        let t = theme(cx);
        let cobalt = tokens::Cobalt::from_theme(t);
        Self {
            background: cobalt.paper,
            sidebar: cobalt.paper_2,
            foreground: cobalt.ink,
            muted_foreground: cobalt.ink_3,
            border: cobalt.rule,
            accent: cobalt.accent,
            danger: cobalt.danger,
            list_hover: cobalt.surface_accent,
            list_active: t.primary().alpha(0.08),
            skeleton: cobalt.rule,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DocumentViewMode {
    Browser,
    Reading,
    Editing,
}

fn document_view_mode(has_document: bool, editing: bool) -> DocumentViewMode {
    if editing {
        DocumentViewMode::Editing
    } else if has_document {
        DocumentViewMode::Reading
    } else {
        DocumentViewMode::Browser
    }
}

impl XWikiApp {
    pub(crate) fn render_tree(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = DocTheme::from_cx(cx);
        // Owned (path, is_dir) pairs: the keyboard listener below is 'static
        // and must not borrow from `self`.
        let items: Vec<(String, bool)> = self
            .tree_entries
            .iter()
            .filter(|e| e.path != "_sidebar.md")
            .map(|e| (e.path.clone(), e.r#type == "tree"))
            .collect();
        let mut list = div().flex().flex_col().w_full();
        let count = items.len();
        let location_label = if self.tree_path.is_empty() {
            "root".to_string()
        } else {
            tokens::truncate(&self.tree_path, 48)
        };
        // Header: path + count badge, plus an up-one-level button whenever
        // the browser is inside a directory (keyboard ← also does this).
        let mut header_left = div().flex().items_center().gap_2();
        if !self.tree_path.is_empty() {
            header_left = header_left.child(
                div()
                    .id("tree-go-up-tooltip")
                    .tooltip(tooltip("返回上一级目录 (←)"))
                    .child(
                        ActionIcon::new("tree-go-up", IconName::ArrowLeft)
                            .size(Size::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let parent =
                                    this.tree_path.rsplit_once('/').map(|(p, _)| p.to_string());
                                this.load_tree(parent.as_deref().unwrap_or(""), cx);
                            })),
                    ),
            );
        }
        header_left = header_left
            .child(
                div()
                    .text_color(theme.accent)
                    .child(Icon::new(IconName::Folder).size(Size::Xs)),
            )
            .child(
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.foreground)
                    .child(location_label),
            );
        list = list.child(
            div()
                .px_4()
                .py_3()
                .flex()
                .items_center()
                .justify_between()
                .child(header_left)
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(tokens::RADIUS_SMALL))
                        .bg(theme.border.opacity(0.18))
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{count}")),
                ),
        );
        if !items.is_empty() {
            list = list.child(div().h(px(1.0)).w_full().bg(theme.border));
        }
        if items.is_empty() {
            list = list.child(
                div()
                    .mx_4()
                    .my_6()
                    .px_4()
                    .py_8()
                    .rounded(px(tokens::RADIUS))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_3()
                    .text_color(theme.muted_foreground)
                    .child(
                        div()
                            .size(px(48.0))
                            .rounded(px(tokens::RADIUS))
                            .bg(theme.border.opacity(0.12))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_color(theme.muted_foreground)
                                    .child(Icon::new(IconName::FolderOpen).size(Size::Md)),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.foreground.opacity(0.7))
                            .child("此目录没有文档"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("创建文档或导入文件来开始"),
                    )
                    .child(
                        Button::new("empty-tree-back", "返回项目")
                            .variant(Variant::Subtle)
                            .size(Size::Xs)
                            .radius(Size::Sm)
                            .left_section(Icon::new(IconName::ArrowLeft).size(Size::Xs))
                            .on_click(cx.listener(|this, _, _, cx| this.back_to_projects(cx))),
                    ),
            );
            return list.into_any_element();
        }
        let focus_bar = theme.accent;
        let app_handle = cx.entity();
        for (i, (path, is_dir)) in items.iter().enumerate() {
            let is_selected = !is_dir && self.doc_path.as_deref() == Some(path.as_str());
            let is_focused = self.tree_focus == Some(i);
            let path_owned = path.clone();
            let is_dir_owned = *is_dir;
            let file_name = path_owned.split('/').next_back().unwrap_or("").to_string();
            // Short extension badge for non-directory items.
            let badge = if is_dir_owned {
                None
            } else {
                file_name
                    .rsplit_once('.')
                    .map(|(_, ext)| ext.to_ascii_uppercase())
            };
            // Per-row context menus (guise entities, keyed by path so they
            // survive re-renders): the ellipsis button opens the full menu,
            // right-click opens the shorter one (no export).
            let btn_path = path_owned.clone();
            let btn_dir = is_dir_owned;
            let btn_app = app_handle.clone();
            let btn_menu = window.use_keyed_state(
                SharedString::from(format!("tree-btn-menu-{path_owned}")),
                cx,
                move |_, cx| {
                    let app = btn_app.clone();
                    let path = btn_path.clone();
                    let is_dir = btn_dir;
                    let mut m =
                        ContextMenu::new(cx).item(if is_dir { "进入目录" } else { "打开" }, {
                            let app = app.clone();
                            let path = path.clone();
                            move |_window, cx| {
                                app.update(cx, |app, cx| {
                                    if is_dir {
                                        app.load_tree(&path, cx);
                                    } else {
                                        app.open_doc(&path, cx);
                                    }
                                });
                            }
                        });
                    if !is_dir {
                        m = m
                            .item("编辑", {
                                let app = app.clone();
                                let path = path.clone();
                                move |_window, cx| {
                                    app.update(cx, |app, cx| {
                                        app.pending_edit = Some(path.clone());
                                        app.open_doc(&path, cx);
                                    });
                                }
                            })
                            .item("导出项目", {
                                let app = app.clone();
                                move |window, cx| {
                                    app.update(cx, |app, cx| app.open_export_dialog(window, cx));
                                }
                            });
                    }
                    m = m
                        .item("移动", {
                            let app = app.clone();
                            let path = path.clone();
                            let is_dir = is_dir;
                            move |window, cx| {
                                app.update(cx, |app, cx| {
                                    app.confirm_move_doc(window, cx, path.clone(), is_dir)
                                });
                            }
                        })
                        .item("重命名", {
                            let app = app.clone();
                            let path = path.clone();
                            let is_dir = is_dir;
                            move |window, cx| {
                                app.update(cx, |app, cx| {
                                    app.confirm_rename_doc(window, cx, path.clone(), is_dir)
                                });
                            }
                        })
                        .danger_item("删除", {
                            let app = app.clone();
                            let path = path.clone();
                            let is_dir = is_dir;
                            move |window, cx| {
                                app.update(cx, |app, cx| {
                                    app.confirm_delete_doc(window, cx, path.clone(), is_dir)
                                });
                            }
                        });
                    m
                },
            );
            let ctx_path = path_owned.clone();
            let ctx_dir = is_dir_owned;
            let ctx_app = app_handle.clone();
            let ctx_menu = window.use_keyed_state(
                SharedString::from(format!("tree-ctx-menu-{path_owned}")),
                cx,
                move |_, cx| {
                    let app = ctx_app.clone();
                    let path = ctx_path.clone();
                    let is_dir = ctx_dir;
                    let mut m =
                        ContextMenu::new(cx).item(if is_dir { "进入目录" } else { "打开" }, {
                            let app = app.clone();
                            let path = path.clone();
                            move |_window, cx| {
                                app.update(cx, |app, cx| {
                                    if is_dir {
                                        app.load_tree(&path, cx);
                                    } else {
                                        app.open_doc(&path, cx);
                                    }
                                });
                            }
                        });
                    if !is_dir {
                        m = m.item("编辑", {
                            let app = app.clone();
                            let path = path.clone();
                            move |_window, cx| {
                                app.update(cx, |app, cx| {
                                    app.pending_edit = Some(path.clone());
                                    app.open_doc(&path, cx);
                                });
                            }
                        });
                    }
                    m = m
                        .item("移动", {
                            let app = app.clone();
                            let path = path.clone();
                            let is_dir = is_dir;
                            move |window, cx| {
                                app.update(cx, |app, cx| {
                                    app.confirm_move_doc(window, cx, path.clone(), is_dir)
                                });
                            }
                        })
                        .item("重命名", {
                            let app = app.clone();
                            let path = path.clone();
                            let is_dir = is_dir;
                            move |window, cx| {
                                app.update(cx, |app, cx| {
                                    app.confirm_rename_doc(window, cx, path.clone(), is_dir)
                                });
                            }
                        })
                        .danger_item("删除", {
                            let app = app.clone();
                            let path = path.clone();
                            let is_dir = is_dir;
                            move |window, cx| {
                                app.update(cx, |app, cx| {
                                    app.confirm_delete_doc(window, cx, path.clone(), is_dir)
                                });
                            }
                        });
                    m
                },
            );
            let row = div()
                .id(SharedString::from(path.clone()))
                .w_full()
                .flex()
                .items_center()
                .px_4()
                .py_2()
                .gap_3()
                .border_b_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .child(
                    div()
                        .w(px(2.0))
                        .h(px(18.0))
                        .flex_none()
                        .rounded(px(1.0))
                        .bg(if is_selected || is_focused {
                            focus_bar
                        } else {
                            gpui::transparent_black()
                        }),
                )
                .child(
                    div()
                        .flex_none()
                        .size(px(28.0))
                        .rounded(px(tokens::RADIUS_SMALL))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_color(if is_dir_owned {
                                    theme.accent
                                } else {
                                    theme.muted_foreground
                                })
                                .child(
                                    Icon::new(if is_dir_owned {
                                        IconName::Folder
                                    } else {
                                        IconName::File
                                    })
                                    .size(Size::Sm),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .font_family(tokens::FONT_MONO)
                                .text_sm()
                                .text_color(if is_selected || is_focused || is_dir_owned {
                                    theme.foreground
                                } else {
                                    theme.muted_foreground
                                })
                                .child(file_name.clone()),
                        )
                        .children(badge.map(|ext| {
                            div()
                                .flex_none()
                                .px_1()
                                .py_1()
                                .rounded(px(3.0))
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.muted_foreground)
                                .bg(theme.border.opacity(0.12))
                                .child(ext)
                        })),
                )
                .child({
                    let menu = btn_menu.clone();
                    div()
                        .flex_none()
                        .size(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(tokens::RADIUS_SMALL))
                        .hover(|s| s.bg(theme.border.opacity(0.15)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_, ev: &MouseDownEvent, window, cx| {
                                let position = ev.position;
                                cx.stop_propagation();
                                menu.update(cx, |menu, cx| menu.show(position, window, cx));
                            }),
                        )
                        .child(Icon::new(IconName::EllipsisVertical).size(Size::Xs))
                })
                .on_click({
                    let click_path = path_owned.clone();
                    cx.listener(move |this, _, _, cx| {
                        if this.editing {
                            return;
                        }
                        this.tree_focus = Some(i);
                        if is_dir_owned {
                            this.load_tree(&click_path, cx);
                        } else {
                            this.open_doc(&click_path, cx);
                        }
                    })
                });
            let row = if is_selected || is_focused {
                row.bg(theme.list_active)
            } else {
                row
            };
            // Plan §3.2: right-click on a tree row opens the shorter menu.
            let row = row.on_mouse_down(MouseButton::Right, {
                let menu_handle = ctx_menu.clone();
                cx.listener(move |_, ev: &MouseDownEvent, window, cx| {
                    let position = ev.position;
                    let menu = menu_handle.clone();
                    menu.update(cx, |menu, cx| menu.show(position, window, cx));
                })
            });
            list = list.child(row.child(btn_menu.clone()).child(ctx_menu.clone()));
        }
        // Keyboard: wrap the list in a focusable container.
        let dirs: Vec<String> = items
            .iter()
            .filter(|(_, d)| *d)
            .map(|(p, _)| p.clone())
            .collect();
        let items_clone = items.clone();
        div()
            .id("tree-keyboard")
            .w_full()
            .focusable()
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _w, cx| {
                if this.editing {
                    return;
                }
                let n = items_clone.len();
                if n == 0 {
                    return;
                }
                let cur = this.tree_focus.unwrap_or(0).min(n - 1);
                match event.keystroke.key.as_str() {
                    "up" => this.tree_focus = Some((cur + n - 1) % n),
                    "down" => this.tree_focus = Some((cur + 1) % n),
                    "right" => {
                        if let Some(dir) = dirs.iter().find(|p| {
                            items_clone
                                .iter()
                                .position(|(path, _)| path == *p)
                                .map(|idx| idx == cur)
                                .unwrap_or(false)
                        }) {
                            this.load_tree(dir, cx);
                            return;
                        }
                    }
                    "left" => {
                        let parent = this.tree_path.rsplit_once('/').map(|(p, _)| p.to_string());
                        this.load_tree(parent.as_deref().unwrap_or(""), cx);
                        return;
                    }
                    "enter" => {
                        let (path, is_dir) = &items_clone[cur];
                        if *is_dir {
                            this.load_tree(path, cx);
                        } else {
                            this.open_doc(path, cx);
                        }
                    }
                    _ => return,
                }
                cx.notify();
            }))
            .child(list)
            .into_any_element()
    }

    fn render_project_changes(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = DocTheme::from_cx(cx);
        let mut timeline = div().size_full().flex().flex_col().child(
            div()
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .child(mono_label("CHANGES · ROADMAP").text_color(theme.accent)),
        );

        if self.project_commits.is_empty() && self.project_changes_error.is_some() {
            timeline = timeline.child(
                div()
                    .px_2()
                    .text_xs()
                    .text_color(theme.danger)
                    .child(self.project_changes_error.clone().unwrap_or_default()),
            );
        }

        if !self.project_commits.is_empty() {
            const NODE_WIDTH: f32 = 168.0;
            const NODE_HEIGHT: f32 = 60.0;
            const NODE_SLOT_HEIGHT: f32 = 18.0;

            let commit_count = self.project_commits.len();
            let mut nodes = div().flex().flex_none().h(px(NODE_HEIGHT));

            for (index, commit) in self.project_commits.iter().enumerate() {
                let sha = commit.sha.clone();
                let is_top = index % 2 == 0;
                let line_color = if index == 0 {
                    theme.accent.opacity(0.7)
                } else {
                    theme.border
                };
                let node_color = if index == 0 {
                    theme.accent
                } else {
                    theme.muted_foreground
                };
                let message = if commit.message.is_empty() {
                    "（无提交信息）".to_string()
                } else {
                    commit.message.clone()
                };
                let title = tokens::truncate(
                    message.lines().next().unwrap_or("（无提交信息）").trim(),
                    24,
                );
                let author = commit.author.clone();
                let date = commit.date.clone();
                let tooltip_sha = sha.clone();

                let mut track = div().w_full().h(px(24.0)).flex().items_center();
                if index > 0 {
                    track = track.child(div().flex_1().h(px(1.0)).bg(line_color));
                }
                track = track.child(
                    div()
                        .size(px(14.0))
                        .flex_none()
                        .rounded(px(7.0))
                        .border_2()
                        .border_color(node_color)
                        .bg(if index == 0 {
                            node_color
                        } else {
                            theme.sidebar
                        }),
                );
                if index + 1 < commit_count {
                    track = track.child(div().flex_1().h(px(1.0)).bg(theme.border));
                }

                let make_title = || {
                    div()
                        .w_full()
                        .h(px(NODE_SLOT_HEIGHT))
                        .px_2()
                        .text_center()
                        .text_xs()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(if index == 0 {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(title.clone())
                };
                let top_slot = if is_top {
                    div()
                        .h(px(NODE_SLOT_HEIGHT))
                        .flex()
                        .items_end()
                        .justify_center()
                        .child(make_title())
                } else {
                    div().h(px(NODE_SLOT_HEIGHT))
                };
                let bottom_slot = if is_top {
                    div().h(px(NODE_SLOT_HEIGHT))
                } else {
                    div()
                        .h(px(NODE_SLOT_HEIGHT))
                        .flex()
                        .items_start()
                        .justify_center()
                        .child(make_title())
                };

                nodes = nodes.child(
                    div()
                        .id(ElementId::named_usize("project-roadmap-node", index))
                        .w(px(NODE_WIDTH))
                        .flex_none()
                        .h(px(NODE_HEIGHT))
                        .flex()
                        .flex_col()
                        .child(top_slot)
                        .child(track)
                        .child(bottom_slot)
                        .tooltip(tooltip(format!(
                            "{message}\n{} · {}\n{}",
                            author, date, tooltip_sha
                        ))),
                );
            }

            let scroll_handle = self.project_changes_scroll.clone();
            let scroll_strip = div()
                .relative()
                .flex_1()
                .min_w(px(0.0))
                .h(px(NODE_HEIGHT))
                .child(
                    div()
                        .id("project-roadmap-scroll-area")
                        .size_full()
                        .track_scroll(&scroll_handle)
                        .overflow_x_scroll()
                        .child(nodes),
                );

            let scroll_left_handle = self.project_changes_scroll.clone();
            let scroll_right_handle = self.project_changes_scroll.clone();
            let scroll_left = div()
                .id("project-roadmap-scroll-left")
                .flex_none()
                .size(px(24.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(Icon::new(IconName::ArrowLeft).size(Size::Sm)),
                )
                .on_click(cx.listener(move |_, _, _, cx| {
                    let offset = scroll_left_handle.offset();
                    let next_x = offset.x + px(240.0);
                    scroll_left_handle.set_offset(point(
                        if next_x > px(0.0) { px(0.0) } else { next_x },
                        offset.y,
                    ));
                    cx.notify();
                }));
            let scroll_right = div()
                .id("project-roadmap-scroll-right")
                .flex_none()
                .size(px(24.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(Icon::new(IconName::ArrowRight).size(Size::Sm)),
                )
                .on_click(cx.listener(move |_, _, _, cx| {
                    let offset = scroll_right_handle.offset();
                    let max_x = scroll_right_handle.max_offset().width;
                    let next_x = offset.x - px(240.0);
                    scroll_right_handle.set_offset(point(
                        if next_x < -max_x { -max_x } else { next_x },
                        offset.y,
                    ));
                    cx.notify();
                }));

            timeline = timeline.child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .child(scroll_left)
                    .child(scroll_strip)
                    .child(scroll_right),
            );
        }

        timeline.into_any_element()
    }

    /// Project card grid (web home.tsx style): hairline panels, hover lift,

    pub(crate) fn render_doc_view(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = DocTheme::from_cx(cx);
        let view_mode = document_view_mode(self.doc_path.is_some(), self.editing);
        if view_mode == DocumentViewMode::Browser {
            return self.render_doc_browser(window, cx);
        }
        let app_handle = cx.entity();
        // Build the content side first so `window` isn't borrowed twice in the
        // splitter call (it may itself open the history split).
        let content = self.render_doc_content(window, cx);
        if view_mode == DocumentViewMode::Editing {
            return div().flex().flex_col().size_full().child(content);
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(split_pane::horizontal(
                "doc-rail-split",
                self.layout.doc_rail,
                tokens::DOC_RAIL_MIN,
                tokens::DOC_RAIL_MAX,
                tokens::DOC_RAIL,
                window,
                theme.border,
                theme.list_hover,
                self.render_doc_rail(cx),
                content,
                move |w, _window, cx| {
                    app_handle.update(cx, |app, cx| {
                        app.layout.doc_rail = w;
                        config::save_layout(&app.layout);
                        cx.notify();
                    });
                },
            ))
    }

    /// Project root and directory browser. It deliberately has no document
    /// rail: the file list owns the center of the workspace, like the web app.
    fn render_doc_browser(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = DocTheme::from_cx(cx);
        let count = self
            .tree_entries
            .iter()
            .filter(|entry| entry.path != "_sidebar.md")
            .count();
        let path_label = if self.tree_path.is_empty() {
            "docs".to_string()
        } else {
            format!("docs / {}", self.tree_path)
        };
        let browser_content = div()
            .w(px(tokens::MEASURE))
            .max_w_full()
            .mx_auto()
            .child(self.render_tree(window, cx));
        let file_list = if self.tree_loading {
            div()
                .flex_1()
                .min_h(px(0.0))
                .p_6()
                .flex()
                .flex_col()
                .gap_3()
                .children((0..6).map(|i| {
                    div()
                        .id(ElementId::named_usize("browser-skeleton", i))
                        .h(px(34.0))
                        .w_full()
                        .rounded(px(tokens::RADIUS_SMALL))
                        .bg(theme.skeleton)
                }))
                .into_any_element()
        } else if let Some(err) = &self.tree_error {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(mono_label("目录加载失败").text_color(theme.danger))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(err.clone()),
                )
                .child(
                    Button::new("retry-browser-tree", "重试")
                        .radius(Size::Sm)
                        .left_section(Icon::new(IconName::Redo2).size(Size::Sm))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let path = this.tree_path.clone();
                            this.load_tree(&path, cx);
                        })),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h(px(0.0))
                .id("file-list-scroll")
                .overflow_y_scroll()
                .p_6()
                .child(browser_content)
                .into_any_element()
        };
        let roadmap = if self.tree_path.is_empty() {
            div()
                .flex_none()
                .h(relative(0.2))
                .w_full()
                .px_6()
                .child(
                    div()
                        .w(px(tokens::MEASURE))
                        .max_w_full()
                        .h_full()
                        .mx_auto()
                        .child(self.render_project_changes(cx)),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };
        let content = div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(file_list)
            .child(roadmap);

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(tokens::TOOLBAR_H))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(
                        Button::new("back-projects-browser", "项目")
                            .variant(Variant::Subtle)
                            .size(Size::Xs)
                            .radius(Size::Sm)
                            .left_section(Icon::new(IconName::ArrowLeft).size(Size::Sm))
                            .on_click(cx.listener(|this, _, _, cx| this.back_to_projects(cx))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(path_label),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded(px(tokens::RADIUS_SMALL))
                            .bg(theme.border.opacity(0.18))
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{count}")),
                    ),
            )
            .child(
                div().px_4().py_2().bg(theme.background).child(
                    div()
                        .w(px(tokens::MEASURE))
                        .max_w_full()
                        .mx_auto()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("doc-import-folder", "导入文件夹")
                                .variant(Variant::Outline)
                                .size(Size::Xs)
                                .radius(Size::Sm)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_document_folder_import_dialog(window, cx)
                                })),
                        )
                        .child(
                            Button::new("doc-import-markdown", "导入 Markdown")
                                .variant(Variant::Outline)
                                .size(Size::Xs)
                                .radius(Size::Sm)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_document_markdown_import_dialog(window, cx)
                                })),
                        ),
                ),
            )
            .child(content)
    }

    pub(crate) fn render_doc_rail(&self, cx: &mut Context<Self>) -> Div {
        let theme = DocTheme::from_cx(cx);
        div()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar)
            .child(
                div()
                    .px_3()
                    .py_3()
                    .w_full()
                    .h(px(tokens::TOOLBAR_H))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("back-document-browser", "返回文件列表")
                            .variant(Variant::Subtle)
                            .size(Size::Xs)
                            .radius(Size::Sm)
                            .left_section(Icon::new(IconName::ArrowLeft).size(Size::Sm))
                            .disabled(self.editing)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.back_to_document_browser(cx)),
                            ),
                    )
                    .child(mono_label("OUTLINE").text_color(theme.muted_foreground)),
            )
            .child(
                div()
                    .flex_1()
                    .id("outline-scroll")
                    .overflow_y_scroll()
                    .p_2()
                    .child(if self.doc_outline.entries.is_empty() {
                        div()
                            .p_3()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("暂无大纲")
                            .into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(self.doc_outline.entries.iter().enumerate().map(
                                |(index, entry)| {
                                    let active = self.active_outline == Some(index);
                                    let section = entry.section;
                                    let indent =
                                        8.0 + (entry.level.saturating_sub(1) as f32) * 12.0;
                                    div()
                                        .id(ElementId::named_usize("outline", index))
                                        .w_full()
                                        .pl(px(indent))
                                        .pr_2()
                                        .py_1()
                                        .rounded(px(tokens::RADIUS_SMALL))
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(if active {
                                            theme.accent
                                        } else {
                                            theme.muted_foreground
                                        })
                                        .bg(if active {
                                            theme.list_active
                                        } else {
                                            theme.sidebar
                                        })
                                        .hover(|s| {
                                            s.bg(theme.list_hover).text_color(theme.foreground)
                                        })
                                        .child(tokens::truncate(&entry.text, 64))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.doc_scroll.scroll_to_top_of_item(section);
                                            this.active_outline = Some(index);

                                            // GPUI applies ScrollHandle's active item during
                                            // prepaint. Keep the window rendering long enough
                                            // for the item bounds and resulting offset to settle.
                                            cx.on_next_frame(window, |_, window, cx| {
                                                cx.on_next_frame(window, |_, _, cx| cx.notify());
                                                cx.notify();
                                            });
                                            cx.notify();
                                        }))
                                },
                            ))
                            .into_any_element()
                    }),
            )
    }

    fn render_reading_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = DocTheme::from_cx(cx);
        if self.doc_content.trim().is_empty() {
            return div()
                .id("doc-content-scroll")
                .flex_1()
                .min_h(px(0.0))
                .track_scroll(&self.doc_scroll)
                .overflow_y_scroll()
                .child(
                    div()
                        .min_h(px(320.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .text_center()
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .child(Icon::new(IconName::File).size(Size::Lg)),
                        )
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child("文档内容为空"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("可以直接开始编辑，保存后会创建一个新的版本。"),
                        )
                        .child(
                            Button::new("empty-doc-edit", "开始编辑")
                                .radius(Size::Sm)
                                .left_section(Icon::new(IconName::File).size(Size::Sm))
                                .on_click(cx.listener(|this, _, _, cx| this.start_edit(cx))),
                        ),
                )
                .child(self.render_doc_panel(cx))
                .into_any_element();
        }

        let sections: Vec<AnyElement> = self
            .doc_outline
            .sections
            .iter()
            .enumerate()
            .map(|(index, section)| {
                div()
                    .id(ElementId::named_usize("doc-section", index))
                    .w_full()
                    .px_6()
                    .py_4()
                    .child(
                        div()
                            .w(px(tokens::MEASURE))
                            .max_w_full()
                            .mx_auto()
                            .child(crate::ui::markdown(cx, section.source.clone())),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .id("doc-content-scroll")
            .flex_1()
            .min_h(px(0.0))
            .track_scroll(&self.doc_scroll)
            .overflow_y_scroll()
            .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                let top = this.doc_scroll.top_item();
                let active = this
                    .doc_outline
                    .entries
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, entry)| entry.section <= top)
                    .map(|(index, _)| index);
                if this.active_outline != active {
                    this.active_outline = active;
                    cx.notify();
                }
            }))
            .children(sections)
            .child(self.render_doc_panel(cx))
            .into_any_element()
    }

    /// Content area: reading/editor, plus the history context panel on the

    pub(crate) fn render_doc_content(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = DocTheme::from_cx(cx);
        let main = div()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .child(self.render_main_pane(window, cx));
        if !self.history_open {
            return main;
        }
        let app_handle = cx.entity();
        split_pane::horizontal_right(
            "history-split",
            self.layout.history,
            tokens::HISTORY_W_MIN,
            tokens::HISTORY_W_MAX,
            tokens::HISTORY_W,
            window,
            theme.border,
            theme.list_hover,
            main,
            self.render_history_view(cx),
            move |w, _window, cx| {
                app_handle.update(cx, |app, cx| {
                    app.layout.history = w;
                    config::save_layout(&app.layout);
                    cx.notify();
                });
            },
        )
    }

    pub(crate) fn render_main_pane(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = DocTheme::from_cx(cx);
        if self.editing {
            return self.render_editor_view(cx).into_any_element();
        }
        if let Some(c) = &self.conflict {
            return self.render_conflict_panel(c, cx).into_any_element();
        }
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(if self.doc_loading {
                // Plan §4: skeleton while the page loads.
                div()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w_full()
                    .child(
                        div()
                            .w(px(320.0))
                            .h(px(20.0))
                            .rounded(px(tokens::RADIUS_SMALL))
                            .bg(theme.skeleton),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(12.0))
                            .rounded(px(tokens::RADIUS_SMALL))
                            .bg(theme.skeleton),
                    )
                    .child(
                        div()
                            .w(px(480.0))
                            .h(px(12.0))
                            .rounded(px(tokens::RADIUS_SMALL))
                            .bg(theme.skeleton),
                    )
                    .child(
                        div()
                            .w(px(360.0))
                            .h(px(12.0))
                            .rounded(px(tokens::RADIUS_SMALL))
                            .bg(theme.skeleton),
                    )
            } else if let Some(err) = &self.doc_error {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(mono_label("加载失败").text_color(theme.danger))
                    .child(
                        div()
                            .px_4()
                            .text_center()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(err.clone()),
                    )
                    .child(
                        Button::new("retry-doc", "重试")
                            .radius(Size::Sm)
                            .left_section(Icon::new(IconName::Redo2).size(Size::Sm))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let path = this.doc_path.clone().unwrap_or_default();
                                this.open_doc(&path, cx);
                            })),
                    )
            } else if let Some(path) = &self.doc_path {
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_6()
                            .pb_4()
                            .border_b_1()
                            .border_color(theme.border)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .overflow_x_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(path.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("open-search", "搜索")
                                            .radius(Size::Sm)
                                            .left_section(
                                                Icon::new(IconName::Search).size(Size::Sm),
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_project_search(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("open-file-history", "历史")
                                            .radius(Size::Sm)
                                            .left_section(Icon::new(IconName::Undo2).size(Size::Sm))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_file_history_panel(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("open-share", "分享")
                                            .variant(Variant::Filled)
                                            .radius(Size::Sm)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_share_panel(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("start-edit", "编辑")
                                            .radius(Size::Sm)
                                            .left_section(Icon::new(IconName::File).size(Size::Sm))
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.start_edit(cx)),
                                            ),
                                    )
                                    .child({
                                        let app_handle = cx.entity();
                                        let more_menu = window.use_keyed_state(
                                            "doc-more-menu",
                                            cx,
                                            move |_, cx| {
                                                let app = app_handle.clone();
                                                ContextMenu::new(cx)
                                                    .item("文档分析", {
                                                        let app = app.clone();
                                                        move |_window, cx| {
                                                            app.update(cx, |app, cx| {
                                                                app.open_backlinks_panel(cx)
                                                            });
                                                        }
                                                    })
                                                    .item("附件", {
                                                        let app = app.clone();
                                                        move |_window, cx| {
                                                            app.update(cx, |app, cx| {
                                                                app.open_attachments_panel(cx)
                                                            });
                                                        }
                                                    })
                                                    .item("移动", {
                                                        let app = app.clone();
                                                        move |window, cx| {
                                                            app.update(cx, |app, cx| {
                                                                if let Some(path) =
                                                                    app.doc_path.clone()
                                                                {
                                                                    app.confirm_move_doc(
                                                                        window, cx, path, false,
                                                                    );
                                                                }
                                                            });
                                                        }
                                                    })
                                            },
                                        );
                                        div()
                                            .id("doc-more-button")
                                            .size(px(28.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded(px(tokens::RADIUS_SMALL))
                                            .hover(|s| s.bg(theme.list_hover))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(
                                                    move |_, ev: &MouseDownEvent, window, cx| {
                                                        let position = ev.position;
                                                        cx.stop_propagation();
                                                        more_menu.update(cx, |menu, cx| {
                                                            menu.show(position, window, cx)
                                                        });
                                                    },
                                                ),
                                            )
                                            .child(
                                                Icon::new(IconName::EllipsisVertical)
                                                    .size(Size::Sm),
                                            )
                                    }),
                            ),
                    )
                    .child(self.render_reading_content(cx).into_any_element())
            } else {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("选择左侧文档")
            })
            .into_any_element()
    }

    fn render_doc_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = DocTheme::from_cx(cx);
        if self.doc_panel == crate::app::DocPanel::None {
            return div().into_any_element();
        }
        let title = "分享页面";
        let header = div()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(theme.border)
            .px_6()
            .py_3()
            .child(
                Button::new("close-doc-panel", "返回文档")
                    .variant(Variant::Subtle)
                    .size(Size::Xs)
                    .left_section(Icon::new(IconName::ArrowLeft).size(Size::Sm))
                    .on_click(cx.listener(|this, _, _, cx| this.close_doc_panel(cx))),
            )
            .child(
                div()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(title),
            );
        let body = self.render_share_panel(cx);
        div()
            .flex_none()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(header)
            .child(body)
            .into_any_element()
    }

    fn render_share_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = DocTheme::from_cx(cx);
        let body = if self.share_loading {
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在创建分享链接…")
        } else if let Some(error) = &self.share_error {
            div()
                .text_sm()
                .text_color(theme.danger)
                .child(error.clone())
        } else if let Some(url) = &self.share_url {
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .px_3()
                        .py_2()
                        .border_1()
                        .border_color(theme.border)
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(theme.foreground)
                        .overflow_x_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(url.clone()),
                )
                .child(
                    Button::new("copy-share-url", "复制完整 URL")
                        .variant(Variant::Filled)
                        .radius(Size::Sm)
                        .on_click(cx.listener(|this, _, _, cx| this.copy_share_url(cx))),
                )
        } else {
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("点击“分享”创建一个只读页面链接。")
        };
        div().px_6().py_4().child(body).into_any_element()
    }

    /// 文档分析独立页（文档页“更多”菜单进入）：全页反向链接列表。
    pub(crate) fn render_backlinks_page(&self, cx: &mut Context<Self>) -> Div {
        let theme = DocTheme::from_cx(cx);
        let page = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .w_full()
                    .max_w(px(1200.0))
                    .mx_auto()
                    .px_6()
                    .py_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Button::new("close-backlinks-page", "返回文档")
                                    .variant(Variant::Subtle)
                                    .size(Size::Xs)
                                    .left_section(Icon::new(IconName::ArrowLeft).size(Size::Sm))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.screen = crate::app::Screen::Workspace;
                                        cx.notify();
                                    })),
                            )
                            .child(div().w(px(1.0)).h(px(20.0)).bg(theme.border))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        mono_label("DOCUMENT / ANALYSIS").text_color(theme.accent),
                                    )
                                    .child(
                                        crate::ui::display("文档分析")
                                            .text_lg()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground),
                                    ),
                            ),
                    ),
            );
        let body: AnyElement = if self.backlinks_loading {
            div()
                .py_4()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在加载反向链接…")
                .into_any_element()
        } else if let Some(error) = &self.backlinks_error {
            div()
                .py_4()
                .text_sm()
                .text_color(theme.danger)
                .child(error.clone())
                .into_any_element()
        } else if self.backlinks.is_empty() {
            div()
                .py_4()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("暂无其他页面引用本文档。")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .children(self.backlinks.iter().enumerate().map(|(index, item)| {
                    let path = item.source.clone();
                    div()
                        .id(ElementId::named_usize("backlink", index))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .py_3()
                        .border_b_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.list_hover))
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.accent)
                                .child(path.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(item.snippet.clone()),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.open_doc(&path, cx)))
                        .into_any_element()
                }))
                .into_any_element()
        };
        page.child(
            div()
                .id("backlinks-page-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .child(
                    div()
                        .w_full()
                        .max_w(px(1200.0))
                        .mx_auto()
                        .px_6()
                        .py_6()
                        .child(body),
                ),
        )
    }

    /// 附件管理独立页（文档页“更多”菜单进入）：上传 + 下载/删除列表。
    pub(crate) fn render_attachments_page(&self, cx: &mut Context<Self>) -> Div {
        let theme = DocTheme::from_cx(cx);
        let page = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .w_full()
                    .max_w(px(1200.0))
                    .mx_auto()
                    .px_6()
                    .py_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Button::new("close-attachments-page", "返回文档")
                                    .variant(Variant::Subtle)
                                    .size(Size::Xs)
                                    .left_section(Icon::new(IconName::ArrowLeft).size(Size::Sm))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.screen = crate::app::Screen::Workspace;
                                        cx.notify();
                                    })),
                            )
                            .child(div().w(px(1.0)).h(px(20.0)).bg(theme.border))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        mono_label("DOCUMENT / ATTACHMENTS")
                                            .text_color(theme.accent),
                                    )
                                    .child(
                                        crate::ui::display("附件")
                                            .text_lg()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground),
                                    ),
                            ),
                    ),
            );
        let mut body = div().flex().flex_col().gap_3();
        body = body
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("上传路径"),
            )
            .child(self.attachment_source_input.clone())
            .child(
                Button::new(
                    "upload-attachment",
                    if self.attachments_loading {
                        "处理中…"
                    } else {
                        "读取并上传附件"
                    },
                )
                .variant(Variant::Filled)
                .radius(Size::Sm)
                .disabled(self.attachments_loading)
                .on_click(cx.listener(|this, _, _, cx| this.upload_attachment(cx))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("下载目标（留空则保存到当前目录）"),
            )
            .child(self.attachment_destination_input.clone());
        if let Some(error) = &self.attachments_error {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(theme.danger)
                    .child(error.clone()),
            );
        }
        if self.attachments_loading {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("正在处理附件…"),
            );
        }
        if self.attachments.is_empty() && !self.attachments_loading {
            body = body.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("暂无附件。"),
            );
        }
        let rows: Vec<AnyElement> = self
            .attachments
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let path = item.path.clone();
                let filename = item.name.clone();
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .py_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.foreground)
                            .overflow_x_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(filename),
                    )
                    .child(
                        Button::new(ElementId::named_usize("download-attachment", index), "下载")
                            .size(Size::Xs)
                            .on_click(cx.listener({
                                let path = path.clone();
                                move |this, _, _, cx| this.download_attachment(&path, cx)
                            })),
                    )
                    .child(
                        Button::new(ElementId::named_usize("delete-attachment", index), "删除")
                            .variant(Variant::Filled)
                            .color(ColorName::Red)
                            .size(Size::Xs)
                            .on_click(cx.listener({
                                let path = path.clone();
                                move |this, _, window, cx| {
                                    this.confirm_delete_attachment(window, cx, path.clone())
                                }
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        body = body.children(rows);
        page.child(
            div()
                .id("attachments-page-scroll")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .child(
                    div()
                        .w_full()
                        .max_w(px(1200.0))
                        .mx_auto()
                        .px_6()
                        .py_6()
                        .child(body),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{document_view_mode, DocumentViewMode};

    #[test]
    fn document_layout_keeps_browser_reading_and_editing_separate() {
        assert_eq!(document_view_mode(false, false), DocumentViewMode::Browser);
        assert_eq!(document_view_mode(true, false), DocumentViewMode::Reading);
        assert_eq!(document_view_mode(true, true), DocumentViewMode::Editing);
        // An empty document can still be edited without bringing back the TOC rail.
        assert_eq!(document_view_mode(false, true), DocumentViewMode::Editing);
    }
}
