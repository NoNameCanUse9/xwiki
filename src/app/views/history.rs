//! History context panel: timeline, commit detail and diff recovery controls.
//! State and network operations stay in `crate::app` (mod.rs).

use gpui::*;
use gpui_component::{button::*, input::Input, scroll::ScrollableElement as _, *};

use crate::app::XWikiApp;
use crate::ui::{mono_label, tokens};

impl XWikiApp {
    pub(crate) fn render_history_view(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let has_query = !self.history_input.read(cx).value().trim().is_empty();
        let visible_indices: Vec<usize> = (0..self.commits.len()).collect();
        let added: u32 = self.diff_stats.iter().map(|stat| stat.added).sum();
        let deleted: u32 = self.diff_stats.iter().map(|stat| stat.deleted).sum();
        let selected_sha = self.selected_sha.as_deref();

        let timeline_content = if self.history_loading && self.commits.is_empty() {
            div()
                .v_flex()
                .gap_2()
                .p_3()
                .children((0..6).map(|i| {
                    div()
                        .id(format!("history-skeleton-{i}"))
                        .h(px(48.0))
                        .w_full()
                        .rounded(px(tokens::RADIUS_SMALL))
                        .bg(theme.skeleton)
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
                .child(Icon::new(IconName::CircleX).text_color(theme.danger))
                .child(mono_label("历史加载失败").text_color(theme.danger))
                .child(
                    div()
                        .text_center()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(error),
                )
                .child(
                    Button::new("retry-history")
                        .rounded(px(tokens::RADIUS))
                        .icon(IconName::Redo2)
                        .label("重试")
                        .on_click(cx.listener(|this, _, _, cx| this.load_history_page(true, cx))),
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
                .child(Icon::new(IconName::Undo2).text_color(theme.muted_foreground))
                .child(mono_label("暂无版本历史").text_color(theme.muted_foreground))
                .child(
                    div()
                        .text_center()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(if has_query {
                            "没有匹配的提交。"
                        } else {
                            "文档保存后，版本会出现在这里。"
                        }),
                )
                .into_any_element()
        } else if visible_indices.is_empty() {
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .p_4()
                .child(Icon::new(IconName::Search).text_color(theme.muted_foreground))
                .child(mono_label("没有匹配的版本").text_color(theme.muted_foreground))
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
                        .id(format!("rev-{sha}"))
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(theme.border)
                        .border_l_3()
                        .border_color(if selected || focused {
                            theme.accent
                        } else {
                            gpui::transparent_black()
                        })
                        .bg(if selected {
                            theme.list_active
                        } else if focused {
                            theme.list_hover
                        } else {
                            theme.sidebar
                        })
                        .hover(|s| s.bg(theme.list_hover))
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
                                    theme.list_hover
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_color(if is_current {
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
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .overflow_x_hidden()
                                                .text_ellipsis()
                                                .whitespace_nowrap()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.foreground)
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
                                                theme.muted_foreground
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
                                        .text_color(theme.muted_foreground)
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
                .v_flex()
                .children(rows)
                .child(if self.history_has_more || self.history_loading {
                    div().p_3().child(
                        Button::new("load-more-history")
                            .w_full()
                            .rounded(px(tokens::RADIUS))
                            .label(if self.history_loading {
                                "加载中…"
                            } else {
                                "加载更多"
                            })
                            .loading(self.history_loading)
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
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scrollbar()
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
                .border_color(theme.border)
                .child(mono_label("加载版本详情…").text_color(theme.muted_foreground))
        } else if let Some(commit) = &self.commit_detail {
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(theme.border)
                .v_flex()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(commit.message.clone()),
                )
                .child(
                    div()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "{} · {} · {}",
                            commit.author, commit.date, commit.sha
                        )),
                )
                .child(
                    div()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .text_color(theme.muted_foreground)
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
                        "A" => theme.success,
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
                                .text_color(theme.muted_foreground)
                                .child(file.path.clone()),
                        )
                        .into_any_element()
                })
                .collect()
        } else {
            Vec::new()
        };
        let stats = div()
            .max_h(px(160.0))
            .overflow_y_scrollbar()
            .border_t_1()
            .border_color(theme.border)
            .p_3()
            .v_flex()
            .gap_2()
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
                .max_h(px(220.0))
                .border_t_1()
                .border_color(theme.border)
                .overflow_y_scrollbar()
                .child(
                    div()
                        .w(px(760.0))
                        .overflow_x_scrollbar()
                        .font_family(tokens::FONT_MONO)
                        .text_xs()
                        .children(lines.into_iter().map(|line| {
                            let (color, bg) = if line.starts_with('+') && !line.starts_with("+++") {
                                (theme.success, theme.success.opacity(0.1))
                            } else if line.starts_with('-') && !line.starts_with("---") {
                                (theme.danger, theme.danger.opacity(0.1))
                            } else if line.starts_with('@') {
                                (theme.accent, theme.accent.opacity(0.08))
                            } else {
                                (theme.muted_foreground, theme.transparent)
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
            .bg(theme.sidebar)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .v_flex()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("close-history")
                                    .ghost()
                                    .compact()
                                    .icon(IconName::ArrowLeft)
                                    .label("返回文档")
                                    .tooltip("返回文档 (Esc)")
                                    .on_click(cx.listener(|this, _, _, cx| this.close_history(cx))),
                            )
                            .child(mono_label("REVISIONS").text_color(theme.accent))
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
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
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        mono_label("搜索版本").text_color(theme.muted_foreground),
                                    )
                                    .child(Input::new(&self.history_input).w_full()),
                            )
                            .child(
                                Button::new("compare-revisions")
                                    .compact()
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Search)
                                    .label(if self.history_compare_open {
                                        "收起 Diff"
                                    } else {
                                        "Compare"
                                    })
                                    .tooltip("对比版本差异")
                                    .disabled(
                                        self.commit_patch.is_none() || self.history_detail_loading,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.history_compare_open = !this.history_compare_open;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("restore-revision")
                                    .danger()
                                    .compact()
                                    .rounded(px(tokens::RADIUS))
                                    .icon(IconName::Redo2)
                                    .label(if self.restoring {
                                        "恢复中…"
                                    } else {
                                        "Restore"
                                    })
                                    .loading(self.restoring)
                                    .tooltip("恢复到此版本（生成新提交）")
                                    .disabled(
                                        self.selected_sha.is_none()
                                            || self.restoring
                                            || self.history_detail_loading,
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_revert_selected(window, cx);
                                    })),
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
                        .child(Icon::new(IconName::CircleX).text_color(theme.danger))
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(theme.danger)
                                .child(error.clone()),
                        )
                        .child(
                            Button::new("retry-history-detail")
                                .compact()
                                .rounded(px(tokens::RADIUS_SMALL))
                                .icon(IconName::Redo2)
                                .label("重试")
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
