//! History context panel (plan §5): render methods for this screen/region; state and logic
//! stay in `crate::app` (mod.rs).
//!
//! Layout matches `docs/agentdocs-code/history.html`:
//!   Left 50%: document content (icon + title + subtitle + rendered markdown)
//!   Right 50%: revision list with avatars + diff stats panel

use gpui::*;
use gpui_component::{button::*, input::Input, scroll::ScrollableElement as _, *};

use crate::app::XWikiApp;
use crate::ui::{markdown, mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_history_view(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let added: u32 = self.diff_stats.iter().map(|stat| stat.added).sum();
        let deleted: u32 = self.diff_stats.iter().map(|stat| stat.deleted).sum();
        let query = self.history_input.read(cx).value().to_lowercase();
        div()
            .h_full()
            .flex()
            .size_full()
            // ── Left 50%: Document content ──────────────────────
            // Matches demo: [icon + title + subtitle] [markdown content]
            .child(
                div()
                    .w_1_2()
                    .flex()
                    .flex_col()
                    .bg(theme.background)
                    .border_r_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .p_6()
                            .border_b_1()
                            .border_color(theme.border)
                            .v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .rounded(px(tokens::RADIUS))
                                            .bg(theme.accent)
                                            .child(
                                                Icon::new(IconName::File)
                                                    .with_size(px(18.0))
                                                    .text_color(theme.accent_foreground),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .font_family(tokens::FONT_DISPLAY)
                                            .text_xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child(
                                                self.doc_path
                                                    .clone()
                                                    .unwrap_or_else(|| {
                                                        "System Architecture V2".into()
                                                    })
                                                    .rsplit('/')
                                                    .next()
                                                    .unwrap_or("")
                                                    .to_string(),
                                            ),
                                    ),
                            )
                            .child(div().text_sm().text_color(theme.muted_foreground).child(
                                if let Some(p) = &self.doc_path {
                                    format!("Last updated · {p}")
                                } else {
                                    "Last updated · 2 hours ago by John Doe".into()
                                },
                            )),
                    )
                    .child(
                        div().flex_1().overflow_y_scrollbar().p_6().child(
                            div()
                                .max_w(px(tokens::MEASURE))
                                .child(markdown("history-doc-preview", self.doc_content.clone())),
                        ),
                    ),
            )
            // ── Right 50%: Revision history ─────────────────────
            // Matches demo: [toolbar] [revision list] [diff stats]
            .child(
                div()
                    .w_1_2()
                    .flex()
                    .flex_col()
                    .bg(theme.sidebar)
                    // ── Toolbar ──────────────────────────────────
                    .child(
                        div()
                            .h(px(tokens::TOOLBAR_H))
                            .px_4()
                            .flex()
                            .items_center()
                            .gap_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(mono_label("REVISIONS").text_color(theme.accent))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{} revisions", self.commits.len())),
                            )
                            .child(div().flex_1())
                            .child(div().w(px(132.0)).child(Input::new(&self.history_input)))
                            .child(
                                Button::new("compare-revisions")
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Search)
                                    .label(if self.history_compare_open {
                                        "收起"
                                    } else {
                                        "Compare"
                                    })
                                    .tooltip("对比版本差异")
                                    .disabled(self.commit_patch.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.history_compare_open = !this.history_compare_open;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("restore-revision")
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Redo2)
                                    .label("Restore")
                                    .tooltip("恢复到此版本（生成新提交）")
                                    .disabled(self.selected_sha.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.revert_selected(cx);
                                    })),
                            ),
                    )
                    // ── Revision list ────────────────────────────
                    .child(
                        div().flex_1().overflow_y_scrollbar().children(
                            self.commits
                                .iter()
                                .enumerate()
                                .filter(|(_, c)| {
                                    query.is_empty()
                                        || c.message.to_lowercase().contains(&query)
                                        || c.author.to_lowercase().contains(&query)
                                        || c.sha.to_lowercase().starts_with(&query)
                                })
                                .map(|(index, c)| {
                                    let sha = c.sha.clone();
                                    let short: String = c.sha.chars().take(7).collect();
                                    let initials: String =
                                        c.author.chars().take(2).collect::<String>().to_uppercase();
                                    let selected =
                                        self.selected_sha.as_deref() == Some(c.sha.as_str());
                                    let is_first = index == 0;

                                    let row = div()
                                        .id(format!("rev-{short}"))
                                        .px_4()
                                        .py_3()
                                        .border_b_1()
                                        .border_color(theme.border)
                                        .border_l_3()
                                        .border_color(if selected {
                                            theme.accent
                                        } else {
                                            gpui::transparent_black()
                                        })
                                        .bg(if selected {
                                            theme.list_active
                                        } else {
                                            theme.background
                                        })
                                        .hover(|s| s.bg(theme.list_hover))
                                        .cursor_pointer()
                                        .flex()
                                        .items_start()
                                        .gap_3()
                                        .child(
                                            // Avatar circle
                                            div()
                                                .size(px(32.0))
                                                .rounded_full()
                                                .bg(if is_first {
                                                    theme.accent
                                                } else {
                                                    theme.list_hover
                                                })
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .font_family(tokens::FONT_MONO)
                                                .text_xs()
                                                .text_color(if is_first {
                                                    theme.accent_foreground
                                                } else {
                                                    theme.foreground
                                                })
                                                .child(initials),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .v_flex()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(theme.foreground)
                                                                .child(c.message.clone()),
                                                        )
                                                        .child(if is_first {
                                                            mono_label("CURRENT")
                                                                .text_color(theme.accent)
                                                        } else {
                                                            mono_label(format!(
                                                                "v{}",
                                                                self.commits
                                                                    .len()
                                                                    .saturating_sub(index)
                                                            ))
                                                            .text_color(theme.muted_foreground)
                                                        }),
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
                                                        .child(format!(
                                                            "{} · {}",
                                                            c.author, c.date
                                                        )),
                                                ),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.select_commit(&sha, cx);
                                        }));
                                    row
                                }),
                        ),
                    )
                    // ── Diff stats panel ─────────────────────────
                    .child(
                        div()
                            .border_t_1()
                            .border_color(theme.border)
                            .p_4()
                            .v_flex()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(mono_label("DIFF").text_color(theme.muted_foreground))
                                    .child(div().flex_1())
                                    .child(
                                        mono_label(format!("+{} / -{}", added, deleted))
                                            .text_color(theme.muted_foreground),
                                    ),
                            )
                            .child(div().w_full().h(px(1.0)).bg(theme.border))
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
                            })),
                    )
                    // ── Patch view (Compare) ─────────────────────
                    // Expanded via the Compare button; renders the unified
                    // diff with +/− color coding, matching the demo diff.
                    .child(if self.history_compare_open {
                        let patch = self.commit_patch.clone().unwrap_or_default();
                        let lines: Vec<&str> = patch.lines().collect();
                        div()
                            .border_t_1()
                            .border_color(theme.border)
                            .max_h(px(220.0))
                            .overflow_y_scrollbar()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .children(lines.into_iter().map(|line| {
                                let (color, bg) =
                                    if line.starts_with('+') && !line.starts_with("+++") {
                                        (theme.success_foreground, gpui::rgba(0x16a34a14).into())
                                    } else if line.starts_with('-') && !line.starts_with("---") {
                                        (theme.danger, gpui::rgba(0xdc262614).into())
                                    } else if line.starts_with('@') {
                                        (theme.accent, gpui::transparent_black())
                                    } else {
                                        (theme.muted_foreground, gpui::transparent_black())
                                    };
                                div()
                                    .px_3()
                                    .py_1()
                                    .bg(bg)
                                    .text_color(color)
                                    .child(line.to_string())
                            }))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
    }

    // ----- Command palette (⌘K), theme toggle, notifications -----
}
