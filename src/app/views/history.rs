//! History context panel: timeline, commit detail and diff recovery controls.
//! State and network operations stay in `crate::app` (mod.rs).

use gpui::*;
use guise::theme::theme;
use guise::{Button, Icon, IconName, Size, Variant};

use crate::app::XWikiApp;
use crate::ui::{mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_history_view(&self, cx: &mut Context<Self>) -> Div {
        let t = theme(cx);
        let theme = tokens::Cobalt::from_theme(t);
        let skeleton = t.border().hsla();
        let success = t.success().hsla();
        let list_active = t.primary().alpha(0.08);
        let has_query = !self.history_input.read(cx).text().trim().is_empty();
        let visible_indices: Vec<usize> = (0..self.commits.len()).collect();
        let added: u32 = self.diff_stats.iter().map(|stat| stat.added).sum();
        let deleted: u32 = self.diff_stats.iter().map(|stat| stat.deleted).sum();
        let selected_sha = self.selected_sha.as_deref();

        let timeline_content =
            if self.history_loading && self.commits.is_empty() {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .children((0..6).map(|i| {
                        div()
                            .id(ElementId::named_usize("history-skeleton", i))
                            .h(px(48.0))
                            .w_full()
                            .rounded(px(tokens::RADIUS_SMALL))
                            .bg(skeleton)
                    }))
                    .into_any_element()
            } else if self.commits.is_empty() && self.history_error.is_some() {
                let error = self.history_error.clone().unwrap_or_default();
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .p_4()
                    .child(
                        div()
                            .text_color(theme.danger)
                            .child(Icon::new(IconName::CircleX)),
                    )
                    .child(mono_label("历史加载失败").text_color(theme.danger))
                    .child(
                        div()
                            .text_center()
                            .text_xs()
                            .text_color(theme.ink_3)
                            .child(error),
                    )
                    .child(
                        Button::new("retry-history", "重试")
                            .size(Size::Sm)
                            .left_section(Icon::new(IconName::Redo2).size(Size::Sm))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.load_history_page(true, cx)),
                            ),
                    )
                    .into_any_element()
            } else if self.commits.is_empty() {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .p_4()
                    .child(
                        div()
                            .text_color(theme.ink_3)
                            .child(Icon::new(IconName::Undo2)),
                    )
                    .child(mono_label("暂无版本历史").text_color(theme.ink_3))
                    .child(div().text_center().text_xs().text_color(theme.ink_3).child(
                        if has_query {
                            "没有匹配的提交。"
                        } else {
                            "文档保存后，版本会出现在这里。"
                        },
                    ))
                    .into_any_element()
            } else if visible_indices.is_empty() {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .p_4()
                    .child(
                        div()
                            .text_color(theme.ink_3)
                            .child(Icon::new(IconName::Search)),
                    )
                    .child(mono_label("没有匹配的版本").text_color(theme.ink_3))
                    .child(crate::ui::clear_search_button(
                        "clear-history-search",
                        self.history_input.clone(),
                        cx.entity().entity_id(),
                    ))
                    .into_any_element()
            } else {
                let rows: Vec<AnyElement> = visible_indices
                    .iter()
                    .map(|index| {
                        let commit = &self.commits[*index];
                        let commit_index = *index;
                        let sha = commit.sha.clone();
                        let short: String = commit.sha.chars().take(7).collect();
                        let initials: String = commit
                            .author
                            .chars()
                            .take(2)
                            .collect::<String>()
                            .to_uppercase();
                        let selected = selected_sha == Some(commit.sha.as_str());
                        let focused = self.history_focus == Some(commit_index);
                        let is_current = commit_index == 0;
                        div()
                            .id(ElementId::from((
                                SharedString::from(format!("rev-{sha}")),
                                commit_index,
                            )))
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(theme.rule)
                            .border_l_3()
                            .border_color(if selected || focused {
                                theme.accent
                            } else {
                                gpui::transparent_black()
                            })
                            .bg(if selected {
                                list_active
                            } else if focused {
                                theme.surface_accent
                            } else {
                                theme.paper_2
                            })
                            .hover(|s| s.bg(theme.surface_accent))
                            .cursor_pointer()
                            .flex()
                            .items_start()
                            .gap_2()
                            .child(
                                div()
                                    .size(px(28.0))
                                    .rounded_full()
                                    .bg(if is_current {
                                        theme.accent
                                    } else {
                                        theme.surface_accent
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(if is_current {
                                        theme.accent_ink
                                    } else {
                                        theme.ink
                                    })
                                    .child(initials),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .overflow_x_hidden()
                                                    .text_ellipsis()
                                                    .whitespace_nowrap()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.ink)
                                                    .child(commit.message.clone()),
                                            )
                                            .child(
                                                mono_label(if is_current {
                                                    "CURRENT"
                                                } else {
                                                    "REVISION"
                                                })
                                                .text_color(if is_current {
                                                    theme.accent
                                                } else {
                                                    theme.ink_3
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .font_family(tokens::FONT_MONO)
                                            .text_xs()
                                            .text_color(theme.ink_3)
                                            .child(short)
                                            .child(format!("{} · {}", commit.author, commit.date)),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.history_focus = Some(commit_index);
                                this.select_commit(&sha, cx);
                            }))
                            .into_any_element()
                    })
                    .collect();
                div()
                    .flex()
                    .flex_col()
                    .children(rows)
                    .child(if self.history_has_more || self.history_loading {
                        div().p_3().child(
                            Button::new(
                                "load-more-history",
                                if self.history_loading {
                                    "加载中…"
                                } else {
                                    "加载更多"
                                },
                            )
                            .full_width(true)
                            .size(Size::Sm)
                            .disabled(self.history_loading)
                            .on_click(cx.listener(|this, _, _, cx| this.load_more_history(cx))),
                        )
                    } else {
                        div()
                    })
                    .into_any_element()
            };

        let keyboard_indices = visible_indices.clone();
        let timeline_scroll = div()
            .id("history-timeline-scroll")
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .child(timeline_content);
        let timeline = div()
            .id("history-keyboard")
            .flex_1()
            .min_h(px(0.0))
            .focusable()
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _window, cx| {
                if keyboard_indices.is_empty() {
                    return;
                }
                let current_position = this
                    .history_focus
                    .and_then(|focus| keyboard_indices.iter().position(|index| *index == focus))
                    .unwrap_or(0);
                let next_position = match event.keystroke.key.as_str() {
                    "up" => {
                        (current_position + keyboard_indices.len() - 1) % keyboard_indices.len()
                    }
                    "down" => (current_position + 1) % keyboard_indices.len(),
                    "enter" => current_position,
                    _ => return,
                };
                let index = keyboard_indices[next_position];
                this.history_focus = Some(index);
                if event.keystroke.key == "enter"
                    && let Some(commit) = this.commits.get(index)
                {
                    let sha = commit.sha.clone();
                    this.select_commit(&sha, cx);
                }
                cx.notify();
            }))
            .child(timeline_scroll);

        let detail = if self.history_detail_loading {
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme.rule)
                .child(mono_label("加载版本详情…").text_color(theme.ink_3))
        } else if let Some(commit) = &self.commit_detail {
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme.rule)
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.ink)
                        .child(commit.message.clone()),
                )
                .child(
                    div()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(theme.ink_3)
                        .child(format!(
                            "{} · {} · {}",
                            commit.author, commit.date, commit.sha
                        )),
                )
                .child(
                    div()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(theme.ink_3)
                        .child(format!("{} 个文件变更", commit.files.len())),
                )
        } else {
            div()
        };

        let stat_rows: Vec<AnyElement> = if let Some(commit) = &self.commit_detail {
            commit
                .files
                .iter()
                .map(|file| {
                    let status_color = match file.status.as_str() {
                        "A" => success,
                        "D" => theme.danger,
                        _ => theme.accent,
                    };
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(mono_label(file.status.clone()).text_color(status_color))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .overflow_x_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(theme.ink_3)
                                .child(file.path.clone()),
                        )
                        .into_any_element()
                })
                .collect()
        } else {
            Vec::new()
        };
        let stats = div()
            .id("history-stats")
            .max_h(px(160.0))
            .overflow_y_scroll()
            .border_t_1()
            .border_color(theme.rule)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(mono_label("DIFF").text_color(theme.ink_3))
                    .child(div().flex_1())
                    .child(
                        mono_label(format!("+{} / -{}", added, deleted)).text_color(theme.ink_3),
                    ),
            )
            .children(stat_rows);

        let patch = if self.history_compare_open {
            let lines: Vec<String> = self
                .commit_patch
                .as_deref()
                .unwrap_or("")
                .lines()
                .map(ToOwned::to_owned)
                .collect();
            div()
                .id("history-patch")
                .max_h(px(220.0))
                .border_t_1()
                .border_color(theme.rule)
                .overflow_y_scroll()
                .child(
                    div()
                        .id("history-patch-x")
                        .w(px(760.0))
                        .overflow_x_scroll()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .children(lines.into_iter().map(|line| {
                            let (color, bg) = if line.starts_with('+') && !line.starts_with("+++") {
                                (success, success.opacity(0.1))
                            } else if line.starts_with('-') && !line.starts_with("---") {
                                (theme.danger, theme.danger.opacity(0.1))
                            } else if line.starts_with('@') {
                                (theme.accent, theme.accent.opacity(0.08))
                            } else {
                                (theme.ink_3, gpui::transparent_black())
                            };
                            div().px_3().py_1().bg(bg).text_color(color).child(line)
                        })),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        div()
            .h_full()
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .bg(theme.paper_2)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.rule)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("close-history", "返回文档")
                                    .variant(Variant::Subtle)
                                    .size(Size::Xs)
                                    .left_section(Icon::new(IconName::ArrowLeft).size(Size::Sm))
                                    .on_click(cx.listener(|this, _, _, cx| this.close_history(cx))),
                            )
                            .child(mono_label("REVISIONS").text_color(theme.accent))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.ink_3)
                                    .child(format!("{} revisions", self.commits.len())),
                            )
                            .child(div().flex_1()),
                    )
                    .child(
                        div()
                            .flex()
                            // The search field has a label above it. Align the
                            // action buttons to the field's bottom edge instead
                            // of centering them against the taller two-line block.
                            .items_end()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(mono_label("搜索版本").text_color(theme.ink_3))
                                    .child(div().w_full().child(self.history_input.clone())),
                            )
                            .child(
                                Button::new(
                                    "compare-revisions",
                                    if self.history_compare_open {
                                        "收起 Diff"
                                    } else {
                                        "Compare"
                                    },
                                )
                                .size(Size::Xs)
                                .left_section(Icon::new(IconName::Search).size(Size::Sm))
                                .disabled(
                                    self.commit_patch.is_none() || self.history_detail_loading,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.history_compare_open = !this.history_compare_open;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                Button::new(
                                    "restore-revision",
                                    if self.restoring {
                                        "恢复中…"
                                    } else {
                                        "Restore"
                                    },
                                )
                                .variant(Variant::Filled)
                                .color(theme.danger)
                                .size(Size::Xs)
                                .left_section(Icon::new(IconName::Redo2).size(Size::Sm))
                                .disabled(
                                    self.selected_sha.is_none()
                                        || self.restoring
                                        || self.history_detail_loading,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.confirm_revert_selected(window, cx);
                                    },
                                )),
                            ),
                    ),
            )
            .child(detail)
            .child(if !self.commits.is_empty() {
                if let Some(error) = &self.history_error {
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(theme.danger)
                        .bg(theme.danger.opacity(0.1))
                        .child(
                            div()
                                .text_color(theme.danger)
                                .child(Icon::new(IconName::CircleX)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(theme.danger)
                                .child(error.clone()),
                        )
                        .child(
                            Button::new("retry-history-detail", "重试")
                                .size(Size::Xs)
                                .left_section(Icon::new(IconName::Redo2).size(Size::Sm))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(sha) = this.selected_sha.clone() {
                                        this.select_commit(&sha, cx);
                                    } else {
                                        this.load_history_page(true, cx);
                                    }
                                })),
                        )
                } else {
                    div()
                }
            } else {
                div()
            })
            .child(timeline)
            .child(stats)
            .child(patch)
    }
}
