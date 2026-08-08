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
        let _filename = path.rsplit('/').next().unwrap_or(path.as_str()).to_string();
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
                            .child(path.clone()),
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
                    .child(div().w(px(280.0)).child(Input::new(&self.commit_msg)))
                    .child(
                        Button::new("save-edit")
                            .primary()
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Check)
                            .label("保存")
                            .tooltip("提交修改 (⌘S)")
                            .disabled(!self.lock_held)
                            .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                    )
                    .child(
                        Button::new("cancel-edit")
                            .rounded(px(tokens::RADIUS))
                            .icon(IconName::Close)
                            .label("取消")
                            .tooltip("放弃修改并释放锁 (Esc)")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_edit(cx))),
                    ),
            )
            // ── Status message bar (error) ──────────────────────
            .child(if let Some(msg) = &self.status_msg {
                div()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.danger)
                    .child(format!("保存状态 · {msg}"))
            } else {
                div()
            })
            // ── Conflict banner ──────────────────────────────────
            // Matches: [⚠ warning] [Remote Conflict] [Review Changes]
            .child(if self.conflict.is_some() {
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.danger)
                    .bg(theme.danger)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(gpui::white())
                            .child("⚠ Remote Conflict"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(gpui::white())
                            .child("A newer revision exists on the server."),
                    )
                    .child(
                        Button::new("review-conflict-inline")
                            .rounded(px(tokens::RADIUS))
                            .label("Review Changes")
                            .on_click(cx.listener(|_this, _, _, cx| {
                                // conflict panel is already shown via render_main_pane
                                cx.notify();
                            })),
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
                    .child(
                        mono_label(if self.lock_held {
                            "● 已保存"
                        } else {
                            "● 锁丢失"
                        })
                        .text_color(if self.lock_held {
                            theme.success_foreground
                        } else {
                            theme.danger
                        }),
                    ),
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
                            .child(mono_label("保存冲突").text_color(theme.danger))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_size(px(tokens::FONT_SIZE_LABEL))
                                    .text_color(theme.muted_foreground)
                                    .child(c.path.clone()),
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
