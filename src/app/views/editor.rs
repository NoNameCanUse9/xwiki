//! Editor + conflict recovery (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::*;
use gpui_component::{
    button::*,
    input::Input,
    *,
};

use crate::app::{ConflictInfo, XWikiApp};
use crate::ui::{mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_editor_view(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(
                // Edit toolbar: path, commit message, save/cancel.
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
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(self.edit_path.clone().unwrap_or_default()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .w(px(320.0))
                            .child(Input::new(&self.commit_msg)),
                    )
                    .child(
                        Button::new("save-edit")
                            .primary()
                            .rounded(px(tokens::RADIUS))
                            .label("保存")
                            .tooltip("提交修改 (⌘S)")
                            .disabled(!self.lock_held)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_edit(cx)
                            })),
                    )
                    .child(
                        Button::new("cancel-edit")
                            .rounded(px(tokens::RADIUS))
                            .label("取消")
                            .tooltip("放弃修改并释放锁 (Esc)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cancel_edit(cx)
                            })),
                    ),
            )
            .child(if let Some(msg) = &self.status_msg {
                div()
                    .px_4()
                    .py_1()
                    .font_family(tokens::FONT_MONO)
                    .text_xs()
                    .text_color(theme.danger)
                    .child(msg.clone())
            } else {
                div()
            })
            .child(
                div()
                    .flex_1()
                    .p_4()
                    .child(Input::new(&self.editor_input).h_full().w_full()),
            )
    }

    /// Breadcrumb trail for the current tree directory; clicking a crumb
    /// navigates back up. Keyboard: ↑/↓ move the focus cursor, → enters a

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
                            .child(
                                mono_label("保存冲突")
                                    .text_color(theme.danger),
                            )
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
                            .child(
                                "服务器上的文档已被修改。选择如何处理你的本地编辑：",
                            ),
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
                                    .label("放弃编辑")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.resolve_conflict_abandon(cx)
                                    })),
                            )
                            .child(
                                Button::new("conflict-reload")
                                    .rounded(px(tokens::RADIUS))
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
