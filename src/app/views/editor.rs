//! Editor + conflict recovery (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).
//!
//! Layout matches `docs/agentdocs-code/editor.html`:
//!   [toolbar | edit/preview tabs | commit msg | save/cancel]
//!   [editable document title]
//!   [editor content / markdown preview]
//!   [status bar: MARKDOWN · UTF-8 · LF · save status]

use gpui::*;
use gpui_component::{button::*, input::Input, scroll::ScrollableElement as _, *};

use crate::app::{ConflictInfo, XWikiApp};
use crate::ui::{markdown, mono_label, tokens};

impl XWikiApp {
    /// Main editor view — shown inside `render_doc_content` when editing.
    ///
    /// Structure mirrors editor.html exactly:
    ///   toolbar → title → content → status bar.
    pub(crate) fn render_editor_view(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let path = self
            .edit_path
            .clone()
            .unwrap_or_else(|| "untitled.md".into());
        let dirty = self.has_unsaved_edits(cx);
        let status_label = if self.saving {
            "● 保存中"
        } else if self.conflict.is_some() {
            "● 冲突待处理"
        } else if dirty {
            "● 未保存"
        } else if self.lock_held {
            "● 已保存"
        } else {
            "● 锁丢失"
        };
        let status_color = if self.saving || dirty {
            theme.accent
        } else if self.conflict.is_some() || !self.lock_held {
            theme.danger
        } else {
            theme.success
        };
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            // ── Toolbar ──────────────────────────────────────────
            // Matches: [EDIT] [path] | flex | [Edit] [Preview] [History] [commit msg] [Save] [Cancel]
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
                    .child(mono_label("EDIT").text_color(theme.accent))
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_x_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(tokens::truncate(&path, 64)),
                    )
                    .child(div().flex_1())
                    .child(self.editor_tab_button(cx, "编辑", !self.editor_preview))
                    .child(self.editor_tab_button(cx, "预览", self.editor_preview))
                    .child(
                        Button::new("editor-history")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Undo2)
                            .label("历史")
                            .tooltip("打开版本历史")
                            .on_click(cx.listener(|this, _, _, cx| this.open_history(cx))),
                    )
                    .child(
                        Button::new("rename-doc")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Replace)
                            .label("重命名")
                            .tooltip("移动 / 重命名当前文档")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_rename_dialog(window, cx)
                            })),
                    )
                    .child(
                        div()
                            .w(px(280.0))
                            .v_flex()
                            .gap_1()
                            .child(mono_label("提交消息").text_color(theme.muted_foreground))
                            .child(Input::new(&self.commit_msg).w_full()),
                    )
                    .child(
                        Button::new("save-edit")
                            .primary()
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Check)
                            .label(if self.saving {
                                "保存中…"
                            } else {
                                "保存"
                            })
                            .loading(self.saving)
                            .tooltip("提交修改 (Ctrl/Cmd+S)")
                            .disabled(!self.lock_held || self.saving || self.conflict.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                    )
                    .child(
                        Button::new("cancel-edit")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Close)
                            .label("取消")
                            .tooltip("放弃修改并释放锁 (Esc)")
                            .disabled(self.saving)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.request_cancel_edit(window, cx)
                            })),
                    ),
            )
            // ── Status message bar (error) ──────────────────────
            .child(if let Some(msg) = &self.save_error {
                div()
                    .px_4()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.danger)
                    .bg(theme.danger.opacity(0.1))
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .child(Icon::new(IconName::CircleX).text_color(theme.danger))
                    .child(
                        div()
                            .flex_1()
                            .text_color(theme.danger)
                            .child(format!("保存失败 · {msg}")),
                    )
                    .child(
                        Button::new("retry-save")
                            .compact()
                            .rounded(px(tokens::RADIUS_SMALL))
                            .icon(IconName::Redo2)
                            .label("重试")
                            .disabled(self.saving || self.conflict.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                    )
            } else if let Some(msg) = &self.status_msg {
                div()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.danger)
                    .child(msg.clone())
            } else {
                div()
            })
            // ── Conflict banner ──────────────────────────────────
            .child(if let Some(conflict) = &self.conflict {
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.danger)
                    .bg(theme.danger.opacity(0.1))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(Icon::new(IconName::TriangleAlert).text_color(theme.danger))
                    .child(
                        div()
                            .flex_1()
                            .v_flex()
                            .gap_1()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.danger)
                            .child("远端版本冲突")
                            .child(tokens::truncate(&conflict.message, 180)),
                    )
                    .child(
                        Button::new("review-conflict-inline")
                            .secondary()
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Search)
                            .label("查看历史")
                            .on_click(cx.listener(|this, _, _, cx| this.open_history(cx))),
                    )
            } else {
                div()
            })
            // ── Document title ───────────────────────────────────
            // Matches: [editable input: "AgentDocs Core Documentation"]
            .child(
                div()
                    .px_6()
                    .pt_5()
                    .pb_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .v_flex()
                    .gap_2()
                    .child(mono_label("文档路径").text_color(theme.muted_foreground))
                    .child(Input::new(&self.editor_title_input).w_full()),
            )
            // ── Editor content / Preview ─────────────────────────
            .child(if self.editor_preview {
                div().flex_1().overflow_y_scrollbar().p_6().child(
                    div().w(px(tokens::MEASURE)).max_w_full().child(markdown(
                        "editor-preview",
                        self.editor_input.read(cx).value().to_string(),
                    )),
                )
            } else {
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_6()
                    .v_flex()
                    .gap_2()
                    .child(mono_label("MARKDOWN CONTENT").text_color(theme.muted_foreground))
                    .child(Input::new(&self.editor_input).h_full().w_full())
            })
            // ── Status bar ───────────────────────────────────────
            // Matches: [Markdown] [UTF-8] [LF] | [Ln 1, Col 1] [Spaces: 2] [● Saved]
            .child(
                div()
                    .h(px(tokens::STATUS_H))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(mono_label("MARKDOWN").text_color(theme.muted_foreground))
                    .child(mono_label("UTF-8").text_color(theme.muted_foreground))
                    .child(mono_label("LF").text_color(theme.muted_foreground))
                    .child(div().flex_1())
                    .child(mono_label(status_label).text_color(status_color)),
            )
    }

    /// Tab-style button for Edit/Preview toggle.
    /// Matches demo: active tab has accent color + bottom border; inactive is muted.
    fn editor_tab_button(
        &self,
        cx: &mut Context<Self>,
        label: &'static str,
        active: bool,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let id = if label == "编辑" {
            "tab-edit"
        } else {
            "tab-preview"
        };
        let is_preview = label == "预览";
        div()
            .id(id)
            .px_4()
            .py_3()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .font_family(tokens::FONT_BODY)
            .text_color(if active {
                theme.accent
            } else {
                theme.muted_foreground
            })
            .border_b_2()
            .border_color(if active {
                theme.accent
            } else {
                gpui::transparent_black()
            })
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.editor_preview = is_preview;
                cx.notify();
            }))
            .into_any_element()
    }

    /// Conflict recovery panel — shown when a save-time revision conflict occurs.
    /// Matches demo: centered card with danger border, description, and action buttons.
    pub(crate) fn render_conflict_panel(&self, c: &ConflictInfo, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(520.0))
                    .p_6()
                    .rounded(px(tokens::RADIUS))
                    .border_1()
                    .border_color(theme.danger)
                    .bg(theme.popover)
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(Icon::new(IconName::TriangleAlert).text_color(theme.danger))
                            .child(mono_label("保存冲突").text_color(theme.danger))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_size(px(tokens::FONT_SIZE_LABEL))
                                    .text_color(theme.muted_foreground)
                                    .overflow_x_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(tokens::truncate(&c.path, 64)),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.foreground)
                            .child(c.message.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("服务器上的文档已被修改。选择如何处理你的本地编辑："),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(
                                Button::new("conflict-abandon")
                                    .danger()
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Delete)
                                    .label("放弃编辑")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_abandon(cx)
                                    })),
                            )
                            .child(
                                Button::new("conflict-reload")
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Redo2)
                                    .label("重新加载")
                                    .tooltip("丢弃本地修改，加载服务器版本")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_reload(cx)
                                    })),
                            )
                            .child(
                                Button::new("conflict-force")
                                    .primary()
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Check)
                                    .label("覆盖重试")
                                    .tooltip("以最新 revision 重新提交（后写者胜）")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_force(cx)
                                    })),
                            ),
                    ),
            )
    }

    // ----- Settings view -----
}
