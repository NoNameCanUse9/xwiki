//! Document workspace (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::*;
use gpui_component::{button::*, menu::ContextMenuExt, scroll::ScrollableElement as _, *};

use crate::app::{DocDeleteAction, DocRenameAction, EditDocAction, TreeContextAction, XWikiApp};
use crate::config;
use crate::ui::{mono_label, split_pane, tokens};

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
        list = list.child(
            div()
                .px_3()
                .py_2()
                .font_family(tokens::FONT_MONO)
                .text_size(px(tokens::FONT_SIZE_LABEL))
                .text_color(theme.muted_foreground)
                .child(format!(
                    "root · {count} {}",
                    if count == 1 { "item" } else { "items" }
                )),
        );
        if items.is_empty() {
            list = list.child(
                div()
                    .mx_3()
                    .my_4()
                    .px_4()
                    .py_5()
                    .rounded(px(tokens::RADIUS))
                    .border_1()
                    .border_color(theme.border)
                    .v_flex()
                    .items_center()
                    .gap_3()
                    .text_center()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(Icon::new(IconName::FolderOpen).text_color(theme.muted_foreground))
                    .child(mono_label("此目录没有文档"))
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
            let row = div()
                .id(path.clone())
                .flex()
                .items_center()
                .gap_2_5()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme.border)
                .cursor_pointer()
                .child(
                    // Selection bar: cobalt signal on the focused/active row.
                    div()
                        .w(px(2.0))
                        .self_stretch()
                        .bg(if is_selected || is_focused {
                            focus_bar
                        } else {
                            gpui::transparent_black()
                        }),
                )
                .child(
                    Icon::new(if is_dir_owned {
                        IconName::Folder
                    } else {
                        IconName::File
                    })
                    .with_size(px(16.0))
                    .text_color(if is_dir_owned {
                        theme.accent
                    } else {
                        theme.muted_foreground
                    }),
                )
                .child(
                    div()
                        .flex_1()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(if is_selected || is_focused || is_dir_owned {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(tokens::truncate(
                            path_owned.split('/').next_back().unwrap_or(""),
                            48,
                        )),
                )
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
                row.hover(|s| s.bg(theme.list_hover))
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
        let app_handle = cx.entity();
        // Build the content side first so `window` isn't borrowed twice in the
        // splitter call (it may itself open the history split).
        let content = self.render_doc_content(window, cx);
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
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("DOCS"),
                    )
                    .child(
                        Button::new("back-projects")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::ArrowLeft)
                            .label("项目")
                            .disabled(self.editing)
                            .on_click(cx.listener(|this, _, _, cx| this.back_to_projects(cx))),
                    ),
            )
            .child(
                // Breadcrumb: docs › dir1 › dir2 (web style).
                div()
                    .px_3()
                    .pb_2()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id("crumb-root")
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .hover(|s| s.text_color(theme.accent))
                            .cursor_pointer()
                            .child("docs")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.editing {
                                    this.load_tree("", cx);
                                }
                            })),
                    )
                    .children({
                        let mut acc = String::new();
                        let mut parts: Vec<String> = Vec::new();
                        for part in self.tree_path.split('/') {
                            if part.is_empty() {
                                continue;
                            }
                            acc = if acc.is_empty() {
                                part.to_string()
                            } else {
                                format!("{acc}/{part}")
                            };
                            parts.push(acc.clone());
                        }
                        let mut out: Vec<AnyElement> = Vec::new();
                        for (i, full) in parts.iter().enumerate() {
                            let is_last = i == parts.len() - 1;
                            let target = full.clone();
                            let name: String =
                                full.split('/').next_back().unwrap_or("").to_string();
                            out.push(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("›")
                                    .into_any_element(),
                            );
                            if is_last {
                                out.push(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.foreground)
                                        .child(tokens::truncate(&name, 32))
                                        .into_any_element(),
                                );
                            } else {
                                out.push(
                                    div()
                                        .id(format!("crumb-{target}"))
                                        .font_family(tokens::FONT_MONO)
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .hover(|s| s.text_color(theme.accent))
                                        .cursor_pointer()
                                        .child(tokens::truncate(&name, 32))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if !this.editing {
                                                this.load_tree(&target, cx);
                                            }
                                        }))
                                        .into_any_element(),
                                );
                            }
                        }
                        out
                    }),
            )
            .child(if self.tree_loading {
                div()
                    .flex_1()
                    .p_3()
                    .v_flex()
                    .gap_3()
                    .children((0..6).map(|i| {
                        div()
                            .id(format!("tree-skeleton-{i}"))
                            .h(px(28.0))
                            .w_full()
                            .rounded(px(tokens::RADIUS_SMALL))
                            .bg(theme.skeleton)
                    }))
                    .into_any_element()
            } else if let Some(err) = &self.tree_error {
                div()
                    .flex_1()
                    .p_3()
                    .v_flex()
                    .gap_3()
                    .child(mono_label("目录加载失败").text_color(theme.danger))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(err.clone()),
                    )
                    .child(
                        Button::new("retry-tree")
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
                    .child(self.render_tree(cx))
                    .into_any_element()
            })
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
            .overflow_y_scrollbar()
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
                    .p_6()
                    .flex_1()
                    .child(
                        div()
                            .pb_4()
                            .mb_4()
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
                    .child(if self.doc_content.trim().is_empty() {
                        div()
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
                            .into_any_element()
                    } else {
                        div()
                            .w_full()
                            .flex()
                            .justify_center()
                            .child(div().w(px(tokens::MEASURE)).child(crate::ui::markdown(
                                "doc-content",
                                self.doc_content.clone(),
                            )))
                            .into_any_element()
                    })
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
