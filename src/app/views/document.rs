//! Document workspace (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::InteractiveElement;
use gpui::*;
use gpui_component::{
    button::*,
    menu::{ContextMenuExt, DropdownMenu},
    scroll::ScrollableElement as _,
    *,
};

use crate::app::{DocDeleteAction, DocRenameAction, EditDocAction, TreeContextAction, XWikiApp};
use crate::config;
use crate::ui::{mono_label, split_pane, tokens};

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
    pub(crate) fn render_tree(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // Owned (path, is_dir) pairs: the keyboard listener below is 'static
        // and must not borrow from `self`.
        let items: Vec<(String, bool)> = self
            .tree_entries
            .iter()
            .filter(|e| e.path != "_sidebar.md")
            .map(|e| (e.path.clone(), e.r#type == "tree"))
            .collect();
        let mut list = div().v_flex().w_full();
        let count = items.len();
        let location_label = if self.tree_path.is_empty() {
            "root".to_string()
        } else {
            tokens::truncate(&self.tree_path, 48)
        };
        // Header: path + count badge.
        list = list.child(
            div()
                .px_4()
                .py_3()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::Folder)
                                .with_size(px(14.0))
                                .text_color(theme.accent),
                        )
                        .child(
                            div()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.foreground)
                                .child(location_label),
                        ),
                )
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
                    .v_flex()
                    .items_center()
                    .gap_3()
                    .text_center()
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
                                Icon::new(IconName::FolderOpen)
                                    .with_size(px(22.0))
                                    .text_color(theme.muted_foreground),
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
                        Button::new("empty-tree-back")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::ArrowLeft)
                            .label("返回项目")
                            .on_click(cx.listener(|this, _, _, cx| this.back_to_projects(cx))),
                    ),
            );
            return list.into_any_element();
        }
        let focus_bar = theme.accent;
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
            let row = div()
                .id(path.clone())
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
                            Icon::new(if is_dir_owned {
                                IconName::Folder
                            } else {
                                IconName::File
                            })
                            .with_size(px(15.0))
                            .text_color(if is_dir_owned {
                                theme.accent
                            } else {
                                theme.muted_foreground
                            }),
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
                    let menu_path = path_owned.clone();
                    let menu_dir = is_dir_owned;
                    div()
                        .flex_none()
                        .size(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(tokens::RADIUS_SMALL))
                        .hover(|s| s.bg(theme.border.opacity(0.15)))
                        .child(
                            Button::new(format!("doc-menu-{}", menu_path))
                                .ghost()
                                .compact()
                                .icon(IconName::EllipsisVertical)
                                .tooltip("文档操作")
                                .dropdown_menu(move |menu, _window, _cx| {
                                    let mut m = menu.menu(
                                        if menu_dir { "进入目录" } else { "打开" },
                                        Box::new(TreeContextAction {
                                            path: menu_path.clone(),
                                            is_dir: menu_dir,
                                        }),
                                    );
                                    if !menu_dir {
                                        m = m.menu(
                                            "编辑",
                                            Box::new(EditDocAction {
                                                path: menu_path.clone(),
                                            }),
                                        );
                                    }
                                    m = m.menu(
                                        "重命名",
                                        Box::new(DocRenameAction {
                                            path: menu_path.clone(),
                                            is_dir: menu_dir,
                                        }),
                                    );
                                    m.menu(
                                        "删除",
                                        Box::new(DocDeleteAction {
                                            path: menu_path.clone(),
                                            is_dir: menu_dir,
                                        }),
                                    )
                                }),
                        )
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
            // Plan §3.2: right-click on a tree row.
            let ctx_path = path_owned.clone();
            let ctx_dir = is_dir_owned;
            let row = row.context_menu(move |menu, _window, _cx| {
                let mut m = menu.menu(
                    if ctx_dir { "进入目录" } else { "打开" },
                    Box::new(TreeContextAction {
                        path: ctx_path.clone(),
                        is_dir: ctx_dir,
                    }),
                );
                if !ctx_dir {
                    m = m.menu(
                        "编辑",
                        Box::new(EditDocAction {
                            path: ctx_path.clone(),
                        }),
                    );
                }
                m = m.menu(
                    "重命名",
                    Box::new(DocRenameAction {
                        path: ctx_path.clone(),
                        is_dir: ctx_dir,
                    }),
                );
                m = m.menu(
                    "删除",
                    Box::new(DocDeleteAction {
                        path: ctx_path.clone(),
                        is_dir: ctx_dir,
                    }),
                );
                m
            });
            list = list.child(row);
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

    /// Project card grid (web home.tsx style): hairline panels, hover lift,

    pub(crate) fn render_doc_view(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let view_mode = document_view_mode(self.doc_path.is_some(), self.editing);
        if view_mode == DocumentViewMode::Browser {
            return self.render_doc_browser(cx);
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
    fn render_doc_browser(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
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
                        Button::new("back-projects-browser")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::ArrowLeft)
                            .label("项目")
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
            .child(if self.tree_loading {
                div()
                    .flex_1()
                    .p_6()
                    .v_flex()
                    .gap_3()
                    .children((0..6).map(|i| {
                        div()
                            .id(format!("browser-skeleton-{i}"))
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
                        Button::new("retry-browser-tree")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Redo2)
                            .label("重试")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let path = this.tree_path.clone();
                                this.load_tree(&path, cx);
                            })),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_6()
                    .child(
                        div()
                            .w(px(tokens::MEASURE))
                            .max_w_full()
                            .mx_auto()
                            .child(self.render_tree(cx)),
                    )
                    .into_any_element()
            })
    }

    pub(crate) fn render_doc_rail(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar)
            .child(
                div()
                    .px_3()
                    .py_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(mono_label("OUTLINE").text_color(theme.muted_foreground))
                    .child(
                        Button::new("back-projects")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::ArrowLeft)
                            .label("项目")
                            .disabled(self.editing)
                            .on_click(cx.listener(|this, _, _, cx| this.back_to_projects(cx))),
                    ),
            )
            .child(div().flex_1().overflow_y_scrollbar().p_2().child(
                if self.doc_outline.entries.is_empty() {
                    div()
                        .p_3()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("暂无大纲")
                        .into_any_element()
                } else {
                    div()
                        .v_flex()
                        .gap_1()
                        .children(self.doc_outline.entries.iter().enumerate().map(
                            |(index, entry)| {
                                let active = self.active_outline == Some(index);
                                let section = entry.section;
                                let indent = 8.0 + (entry.level.saturating_sub(1) as f32) * 12.0;
                                div()
                                    .id(format!("outline-{index}"))
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
                                    .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                                    .child(tokens::truncate(&entry.text, 64))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.doc_scroll.scroll_to_top_of_item(section);
                                        this.active_outline = Some(index);
                                        cx.notify();
                                    }))
                            },
                        ))
                        .into_any_element()
                },
            ))
    }

    fn render_reading_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        if self.doc_content.trim().is_empty() {
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .text_center()
                .child(
                    Icon::new(IconName::File)
                        .with_size(px(24.0))
                        .text_color(theme.muted_foreground),
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
                    Button::new("empty-doc-edit")
                        .rounded(px(tokens::RADIUS))
                        .icon(IconName::File)
                        .label("开始编辑")
                        .on_click(cx.listener(|this, _, _, cx| this.start_edit(cx))),
                )
                .into_any_element();
        }

        let sections =
            self.doc_outline
                .sections
                .iter()
                .enumerate()
                .map(|(index, section)| {
                    div()
                        .id(format!("doc-section-{index}"))
                        .w_full()
                        .px_6()
                        .py_4()
                        .child(div().w(px(tokens::MEASURE)).max_w_full().child(
                            crate::ui::markdown(
                                format!("doc-content-{index}"),
                                section.source.clone(),
                            ),
                        ))
                });

        div()
            .id("doc-content-scroll")
            .flex_1()
            .min_h(px(0.0))
            .track_scroll(&self.doc_scroll)
            .overflow_y_scrollbar()
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
            .into_any_element()
    }

    /// Content area: reading/editor, plus the history context panel on the

    pub(crate) fn render_doc_content(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let main = div()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .child(self.render_main_pane(cx));
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

    pub(crate) fn render_main_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
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
                    .v_flex()
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
                        Button::new("retry-doc")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Redo2)
                            .label("重试")
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
                                    .gap_2()
                                    .child(
                                        Button::new("open-search")
                                            .rounded(px(tokens::RADIUS))
                                            .icon(IconName::Search)
                                            .label("搜索")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_project_search(cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("open-history")
                                            .rounded(px(tokens::RADIUS))
                                            .icon(IconName::Undo2)
                                            .label("历史")
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.open_history(cx)),
                                            ),
                                    )
                                    .child(
                                        Button::new("start-edit")
                                            .rounded(px(tokens::RADIUS))
                                            .icon(IconName::File)
                                            .label("编辑")
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.start_edit(cx)),
                                            ),
                                    ),
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
