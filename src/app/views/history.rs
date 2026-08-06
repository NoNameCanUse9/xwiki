//! History context panel (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).

use gpui::*;
use gpui_component::{
    button::*,
    scroll::ScrollableElement as _,
    *,
};

use crate::app::XWikiApp;
use crate::ui::{mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_history_view(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        div()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.sidebar)
            .child(
                // Panel header: mono label + close (context panel, not a screen).
                div()
                    .h(px(tokens::TOOLBAR_H))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(mono_label("HISTORY").text_color(theme.muted_foreground))
                    .child(div().flex_1())
                    .child(
                        Button::new("close-history")
                            .rounded(px(tokens::RADIUS))
                            .label("关闭")
                            .tooltip("关闭历史面板 (Esc)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_history(cx)
                            })),
                    ),
            )
            .child(
                // Commit list (top): message first, sha/author/date meta.
                div()
                    .flex_1()
                    .min_h(px(120.0))
                    .overflow_y_scrollbar()
                    .children(self.commits.iter().map(|c| {
                        let sha = c.sha.clone();
                        let short: String = c.sha.chars().take(7).collect();
                        let selected = self.selected_sha.as_deref() == Some(c.sha.as_str());
                        let row = div()
                            .id(format!("commit-{short}"))
                            .px_3()
                            .py_2_5()
                            .border_b_1()
                            .border_color(theme.border)
                            .cursor_pointer()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(c.message.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(short)
                                    .child(format!("{} · {}", c.author, c.date)),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_commit(&sha, cx);
                            }));
                        if selected {
                            row.bg(theme.list_active)
                        } else {
                            row.hover(|s| s.bg(theme.list_hover))
                        }
                    })),
            )
            .child(
                // Commit detail (bottom): message, files, numstat — stays in
                // sync with the selected commit above.
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_4()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(if let Some(d) = &self.commit_detail {
                        div()
                            .v_flex()
                            .gap_3()
                            .w_full()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(d.message.clone()),
                            )
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!(
                                        "{} · {} · {}",
                                        d.sha, d.author, d.date
                                    )),
                            )
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("FILES").text_color(theme.muted_foreground))
                            .children(d.files.iter().map(|f| {
                                let color = if f.status.as_str() == "D" {
                                    theme.danger
                                } else {
                                    theme.foreground
                                };
                                div()
                                    .flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.foreground)
                                            .child(f.status.clone()),
                                    )
                                    .child(
                                        div()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(color)
                                            .child(f.path.clone()),
                                    )
                            }))
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
                            .child(mono_label("NUMSTAT").text_color(theme.muted_foreground))
                            .children(self.diff_stats.iter().map(|s| {
                                div()
                                    .flex()
                                    .gap_3()
                                    .items_center()
                                    .child(
                                        div()
                                            .w(px(tokens::NUMSTAT_W))
                                            .text_right()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.foreground)
                                            .child(format!("+{}", s.added)),
                                    )
                                    .child(
                                        div()
                                            .w(px(tokens::NUMSTAT_W))
                                            .text_right()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.danger)
                                            .child(format!("-{}", s.deleted)),
                                    )
                                    .child(
                                        div()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(s.path.clone()),
                                    )
                            }))
                    } else {
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("选择上方提交查看详情")
                    }),
            )
    }

    // ----- Command palette (⌘K), theme toggle, notifications -----

}
