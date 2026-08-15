//! Editor + conflict recovery (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).
//!
//! Layout matches `docs/xwiki-code/editor.html`:
//!   [toolbar | edit/preview tabs | commit msg | save/cancel]
//!   [editable document title]
//!   [editor content / markdown preview]
//!   [status bar: MARKDOWN · UTF-8 · LF · save status]

use gpui::*;
use guise::{
    style::Variant,
    theme::{theme, ColorName, Size as GuiseSize},
    Button, Icon, IconName,
};

use crate::app::{ConflictInfo, XWikiApp};
use crate::ui::{markdown, mono_label, tokens};

impl XWikiApp {
    /// Main editor view — shown inside `render_doc_content` when editing.
    ///
    /// Structure mirrors editor.html exactly:
    ///   toolbar → title → content → status bar.
    pub(crate) fn render_editor_view(&self, cx: &mut Context<Self>) -> Div {
        let t = theme(cx);
        let cobalt = tokens::Cobalt::from_theme(t);
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
            cobalt.accent
        } else if self.conflict.is_some() || !self.lock_held {
            cobalt.danger
        } else {
            t.success().hsla()
        };
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            // ── Toolbar ──────────────────────────────────────────
            // Matches: [EDIT] [path] | [Edit] [Preview] [History] [Rename]
            // [commit msg] [Save] [Cancel]
            .child(
                div()
                    .h(px(tokens::TOOLBAR_H))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(cobalt.rule)
                    .bg(cobalt.paper_2)
                    .child(mono_label("EDIT").text_color(cobalt.accent))
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(cobalt.ink_3)
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_x_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(tokens::truncate(&path, 64)),
                    )
                    .child(self.editor_tab_button(cx, "编辑", !self.editor_preview))
                    .child(self.editor_tab_button(cx, "预览", self.editor_preview))
                    .child(
                        Button::new("editor-history", "历史")
                            .variant(Variant::Outline)
                            .size(GuiseSize::Xs)
                            .left_section(Icon::new(IconName::Undo2).size(GuiseSize::Xs))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.open_file_history_panel(cx)),
                            ),
                    )
                    .child(
                        Button::new("rename-doc", "重命名")
                            .variant(Variant::Outline)
                            .size(GuiseSize::Xs)
                            .left_section(Icon::new(IconName::Replace).size(GuiseSize::Xs))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_rename_dialog(window, cx)
                            })),
                    )
                    .child(div().w(px(280.0)).child(self.commit_msg.clone()))
                    .child(
                        Button::new(
                            "save-edit",
                            if self.saving {
                                "保存中…"
                            } else {
                                "保存"
                            },
                        )
                        .variant(Variant::Filled)
                        .size(GuiseSize::Xs)
                        .left_section(Icon::new(IconName::Check).size(GuiseSize::Xs))
                        .disabled(!self.lock_held || self.saving || self.conflict.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                    )
                    .child(
                        Button::new("cancel-edit", "取消")
                            .variant(Variant::Outline)
                            .size(GuiseSize::Xs)
                            .left_section(Icon::new(IconName::Close).size(GuiseSize::Xs))
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
                    .border_color(cobalt.danger)
                    .bg(cobalt.danger.opacity(0.1))
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .child(Icon::new(IconName::CircleX).color(ColorName::Red))
                    .child(
                        div()
                            .flex_1()
                            .text_color(cobalt.danger)
                            .child(format!("保存失败 · {msg}")),
                    )
                    .child(
                        Button::new("retry-save", "重试")
                            .variant(Variant::Outline)
                            .size(GuiseSize::Xs)
                            .left_section(Icon::new(IconName::Redo2).size(GuiseSize::Xs))
                            .disabled(self.saving || self.conflict.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                    )
            } else if let Some(msg) = &self.status_msg {
                div()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(cobalt.rule)
                    .bg(cobalt.paper_2)
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(cobalt.danger)
                    .child(msg.clone())
                    .child(if self.editing && !self.lock_held {
                        Button::new("reacquire-lock", "重新获取锁")
                            .variant(Variant::Outline)
                            .size(GuiseSize::Xs)
                            .left_section(Icon::new(IconName::Redo2).size(GuiseSize::Xs))
                            .on_click(cx.listener(|this, _, _, cx| this.reacquire_lock(cx)))
                            .into_any_element()
                    } else {
                        div().hidden().into_any_element()
                    })
            } else {
                div()
            })
            // ── Conflict banner ──────────────────────────────────
            .child(if let Some(conflict) = &self.conflict {
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(cobalt.danger)
                    .bg(cobalt.danger.opacity(0.1))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(Icon::new(IconName::TriangleAlert).color(ColorName::Red))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(cobalt.danger)
                            .child("远端版本冲突")
                            .child(tokens::truncate(&conflict.message, 180)),
                    )
                    .child(
                        Button::new("review-conflict-inline", "查看历史")
                            .variant(Variant::Outline)
                            .size(GuiseSize::Xs)
                            .left_section(Icon::new(IconName::Search).size(GuiseSize::Xs))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.open_file_history_panel(cx)),
                            ),
                    )
            } else {
                div()
            })
            // ── Editor content / Preview ─────────────────────────
            .child(if self.editor_preview {
                let content = self.editor_input.read(cx).text();
                div()
                    .id("editor-preview-scroll")
                    .flex_1()
                    .p_6()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w(px(tokens::MEASURE))
                            .max_w_full()
                            .child(markdown(&mut *cx, content)),
                    )
            } else {
                div()
                    .id("editor-input-scroll")
                    .flex_1()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(mono_label("MARKDOWN CONTENT").text_color(cobalt.ink_3))
                    .child(div().h_full().w_full().child(self.editor_input.clone()))
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
                    .border_color(cobalt.rule)
                    .bg(cobalt.paper_2)
                    .child(mono_label("MARKDOWN").text_color(cobalt.ink_3))
                    .child(mono_label("UTF-8").text_color(cobalt.ink_3))
                    .child(mono_label("LF").text_color(cobalt.ink_3))
                    .child(div().flex_1())
                    .child(mono_label(status_label).text_color(status_color)),
            )
    }

    fn editor_tab_button(
        &self,
        cx: &mut Context<Self>,
        label: &'static str,
        active: bool,
    ) -> AnyElement {
        let cobalt = tokens::Cobalt::from_theme(theme(cx));
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
            .text_color(if active { cobalt.accent } else { cobalt.ink_3 })
            .border_b(px(2.0))
            .border_color(if active {
                cobalt.accent
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
        let cobalt = tokens::Cobalt::from_theme(theme(cx));
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
                    .border_color(cobalt.danger)
                    .bg(cobalt.paper_2)
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(Icon::new(IconName::TriangleAlert).color(ColorName::Red))
                            .child(mono_label("保存冲突").text_color(cobalt.danger))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_size(px(tokens::FONT_SIZE_LABEL))
                                    .text_color(cobalt.ink_3)
                                    .overflow_x_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(tokens::truncate(&c.path, 64)),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cobalt.ink)
                            .child(c.message.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cobalt.ink_3)
                            .child("服务器上的文档已被修改。选择如何处理你的本地编辑："),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .justify_end()
                            .w_full()
                            .child(
                                Button::new("conflict-abandon", "放弃编辑")
                                    .variant(Variant::Filled)
                                    .color(ColorName::Red)
                                    .size(GuiseSize::Xs)
                                    .left_section(Icon::new(IconName::Delete).size(GuiseSize::Xs))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_abandon(cx)
                                    })),
                            )
                            .child(
                                Button::new("conflict-reload", "重新加载")
                                    .variant(Variant::Outline)
                                    .size(GuiseSize::Xs)
                                    .left_section(Icon::new(IconName::Redo2).size(GuiseSize::Xs))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_reload(cx)
                                    })),
                            )
                            .child(
                                Button::new("conflict-force", "覆盖重试")
                                    .variant(Variant::Filled)
                                    .size(GuiseSize::Xs)
                                    .left_section(Icon::new(IconName::Check).size(GuiseSize::Xs))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_force(cx)
                                    })),
                            ),
                    ),
            )
    }

    // ----- Settings view -----
}
