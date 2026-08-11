//! Persistent authenticated desktop shell: topbar, content and status area.

use gpui::*;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{button::*, *};

use crate::app::{Screen, XWikiApp};
use crate::ui::{app_icon, mono_label, tokens};

#[derive(Clone)]
struct ApiOperationEntry {
    method: String,
    summary: String,
    tags: Vec<String>,
}

#[derive(Clone)]
struct ApiPathEntry {
    path: String,
    operations: Vec<ApiOperationEntry>,
}

/// Localize an RFC3339 UTC timestamp the way the web audit page does
/// (`toLocaleString`). Falls back to the raw string when parsing fails.
/// Localize an RFC3339 UTC timestamp the way the web audit page does
/// (`toLocaleString`). Falls back to the raw string when parsing fails.
fn format_audit_time(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|_| iso.to_string())
}

#[cfg(test)]
mod audit_time_tests {
    use super::format_audit_time;

    #[test]
    fn localizes_rfc3339_utc() {
        // TZ-independent check: the local rendering must be the UTC instant
        // shifted to the machine's local offset.
        let local = format_audit_time("2026-08-03T04:47:48Z");
        let expected = chrono::DateTime::parse_from_rfc3339("2026-08-03T04:47:48Z")
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        assert_eq!(local, expected);
        assert!(local.starts_with("2026-08-03"));
    }

    #[test]
    fn falls_back_to_raw_on_parse_failure() {
        assert_eq!(format_audit_time("not-a-timestamp"), "not-a-timestamp");
        assert_eq!(format_audit_time(""), "");
    }
}

fn api_fallback_tag(path: &str) -> &'static str {
    let route = path.to_ascii_lowercase();
    if route.contains("openapi") {
        "Developer"
    } else if route.contains("/auth/") {
        "Authentication"
    } else if route.contains("/meta") {
        "System"
    } else if route.contains("share") {
        "Sharing"
    } else if route.contains("backlink") || route.contains("search") {
        "Discovery"
    } else if route.contains("attachment") {
        "Attachments"
    } else if route.contains("import") || route.contains("export") {
        "Transfer"
    } else if route.contains("file/history")
        || route.contains("commits")
        || route.contains("revision")
    {
        "History"
    } else if route.contains("audit") || route.contains("token") || route.contains("user") {
        "Administration"
    } else if route.contains("lock") || route.contains("changeset") {
        "Editing"
    } else if route.contains("/docs") || route.contains("/pages") {
        "Documents"
    } else if route.contains("project") {
        "Projects"
    } else {
        "General"
    }
}

fn api_tag_label(tag: &str) -> String {
    match tag.to_ascii_lowercase().as_str() {
        "projects" | "project" => "项目管理".into(),
        "documents" | "document" | "docs" | "pages" => "文档".into(),
        "sharing" | "share" | "shares" => "分享".into(),
        "discovery" | "search" | "backlinks" | "backlink" => "搜索与关系".into(),
        "attachments" | "attachment" => "附件".into(),
        "transfer" | "import" | "export" => "导入与导出".into(),
        "history" | "commits" | "revision" => "历史与版本".into(),
        "editing" | "locks" | "changesets" => "编辑与同步".into(),
        "administration" | "audit" | "tokens" | "users" => "管理".into(),
        "developer" | "openapi" => "开发者".into(),
        "authentication" | "auth" => "认证".into(),
        "system" | "meta" => "系统".into(),
        "general" | "default" => "其他".into(),
        _ => tag.to_string(),
    }
}

impl XWikiApp {
    pub(crate) fn render_authenticated_shell(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let content = match self.screen {
            Screen::Workspace => {
                if self.selected_project.is_some() {
                    self.render_doc_view(window, cx).into_any_element()
                } else {
                    self.render_workspace(window, cx).into_any_element()
                }
            }
            Screen::Settings => self.render_settings(cx).into_any_element(),
            Screen::ApiReference => self.render_api_reference(cx).into_any_element(),
            Screen::Audit => self.render_audit(cx).into_any_element(),
            Screen::Login => self.render_login(cx).into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.render_shell_topbar(cx))
            .child(div().flex_1().min_w(px(0.0)).min_h(px(0.0)).child(content))
            .child(self.render_status_bar(cx))
    }

    pub(crate) fn render_api_reference(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let spec = self
            .api_reference
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
        let schema_version = spec
            .as_ref()
            .and_then(|value| value.get("openapi"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("OpenAPI")
            .to_string();
        let schema_title = spec
            .as_ref()
            .and_then(|value| value.get("info"))
            .and_then(|value| value.get("title"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("AgentDocs API")
            .to_string();
        let schema_description = spec
            .as_ref()
            .and_then(|value| value.get("info"))
            .and_then(|value| value.get("description"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("服务端 API 的只读参考文档。Try it 默认关闭。")
            .to_string();
        let mut path_entries: Vec<ApiPathEntry> = Vec::new();
        if let Some(paths) = spec
            .as_ref()
            .and_then(|value| value.get("paths"))
            .and_then(serde_json::Value::as_object)
        {
            for (path, operations) in paths {
                let Some(operations) = operations.as_object() else {
                    continue;
                };
                let mut items = Vec::new();
                for (method, operation) in operations {
                    if !matches!(
                        method.as_str(),
                        "get" | "post" | "put" | "patch" | "delete" | "head" | "options"
                    ) {
                        continue;
                    }
                    let summary = operation
                        .get("summary")
                        .or_else(|| operation.get("description"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("未提供接口说明")
                        .to_string();
                    let mut tags = operation
                        .get("tags")
                        .and_then(serde_json::Value::as_array)
                        .map(|tags| {
                            tags.iter()
                                .filter_map(serde_json::Value::as_str)
                                .map(str::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if tags.is_empty()
                        || tags.iter().all(|tag| {
                            matches!(tag.to_ascii_lowercase().as_str(), "general" | "default")
                        })
                    {
                        tags = vec![api_fallback_tag(path).into()];
                    }
                    items.push(ApiOperationEntry {
                        method: method.to_uppercase(),
                        summary,
                        tags,
                    });
                }
                if !items.is_empty() {
                    path_entries.push(ApiPathEntry {
                        path: path.clone(),
                        operations: items,
                    });
                }
            }
        }
        let endpoint_count: usize = path_entries
            .iter()
            .map(|entry| entry.operations.len())
            .sum();
        let selected_path = self
            .api_reference_selected_path
            .clone()
            .filter(|path| path_entries.iter().any(|entry| &entry.path == path))
            .or_else(|| path_entries.first().map(|entry| entry.path.clone()));
        let selected_operations = selected_path
            .as_ref()
            .and_then(|path| {
                path_entries
                    .iter()
                    .find(|entry| &entry.path == path)
                    .map(|entry| entry.operations.clone())
            })
            .unwrap_or_default();
        let selected_method = self
            .api_reference_selected_method
            .clone()
            .filter(|method| {
                selected_operations
                    .iter()
                    .any(|operation| &operation.method == method)
            })
            .or_else(|| {
                selected_operations
                    .first()
                    .map(|operation| operation.method.clone())
            });
        let selected_summary = selected_operations
            .iter()
            .find(|operation| Some(&operation.method) == selected_method.as_ref())
            .map(|operation| operation.summary.clone())
            .unwrap_or_else(|| "选择左侧接口查看详情".into());

        let page = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .w_full()
                    .max_w(px(1200.0))
                    .mx_auto()
                    .px_6()
                    .py_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        Button::new("close-openapi")
                                            .ghost()
                                            .compact()
                                            .icon(IconName::ArrowLeft)
                                            .label("返回工作区")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.api_reference_open = false;
                                                this.screen = Screen::Workspace;
                                                cx.notify();
                                            })),
                                    )
                                    .child(div().w(px(1.0)).h(px(20.0)).bg(theme.border))
                                    .child(
                                        div()
                                            .v_flex()
                                            .gap_1()
                                            .child(
                                                mono_label("DEVELOPER / REFERENCE")
                                                    .text_color(theme.accent),
                                            )
                                            .child(
                                                crate::ui::display("OpenAPI Reference")
                                                    .text_lg()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.foreground),
                                            ),
                                    ),
                            )
                            .child(
                                Button::new("copy-openapi-json")
                                    .secondary()
                                    .outline()
                                    .compact()
                                    .label("复制 JSON")
                                    .disabled(self.api_reference.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(spec) = this.api_reference.clone() {
                                            cx.write_to_clipboard(ClipboardItem::new_string(spec));
                                            this.notify("OpenAPI JSON 已复制".into(), cx);
                                        }
                                    })),
                            ),
                    ),
            );
        if self.api_reference_loading {
            return page.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("正在加载 OpenAPI schema…"),
            );
        }
        if let Some(error) = &self.api_reference_error {
            return page.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .text_sm()
                    .text_color(theme.danger)
                    .child(error.clone())
                    .child(
                        Button::new("retry-openapi")
                            .rounded(px(tokens::RADIUS))
                            .label("重试")
                            .on_click(cx.listener(|this, _, _, cx| this.open_api_reference(cx))),
                    ),
            );
        }

        let mut tag_groups: Vec<(String, Vec<(String, Vec<String>)>)> = Vec::new();
        for entry in &path_entries {
            for operation in &entry.operations {
                for tag in &operation.tags {
                    let Some(group_index) = tag_groups.iter().position(|(name, _)| name == tag)
                    else {
                        tag_groups.push((tag.clone(), Vec::new()));
                        let group_index = tag_groups.len() - 1;
                        tag_groups[group_index]
                            .1
                            .push((entry.path.clone(), vec![operation.method.clone()]));
                        continue;
                    };
                    let paths = &mut tag_groups[group_index].1;
                    if let Some((_, methods)) =
                        paths.iter_mut().find(|(path, _)| path == &entry.path)
                    {
                        if !methods.contains(&operation.method) {
                            methods.push(operation.method.clone());
                        }
                    } else {
                        paths.push((entry.path.clone(), vec![operation.method.clone()]));
                    }
                }
            }
        }

        let endpoint_nav: Vec<AnyElement> = tag_groups
            .iter()
            .map(|(tag, paths)| {
                let rows: Vec<AnyElement> = paths
                    .iter()
                    .map(|(path, methods)| {
                        let path_owned = path.clone();
                        let is_selected = selected_path.as_ref() == Some(path);
                        let mut row = div()
                            .id(format!("api-nav-{}", path.replace('/', "-")))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .px_3()
                            .py_3()
                            .cursor_pointer()
                            .child(
                                div()
                                    .font_family(tokens::FONT_MONO)
                                    .text_xs()
                                    .text_color(if is_selected {
                                        theme.foreground
                                    } else {
                                        theme.muted_foreground
                                    })
                                    .child(path.clone()),
                            )
                            .child(div().flex().gap_1().children(methods.iter().map(|method| {
                                mono_label(method.clone())
                                    .text_color(if is_selected {
                                        theme.accent
                                    } else {
                                        theme.muted_foreground
                                    })
                                    .into_any_element()
                            })))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.api_reference_selected_path = Some(path_owned.clone());
                                this.api_reference_selected_method = None;
                                cx.notify();
                            }));
                        row = if is_selected {
                            row.bg(theme.list_active)
                        } else {
                            row.hover(|style| style.bg(theme.list_hover))
                        };
                        row.into_any_element()
                    })
                    .collect();
                div()
                    .v_flex()
                    .child(
                        div()
                            .px_3()
                            .py_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(mono_label(api_tag_label(tag)).text_color(theme.accent)),
                    )
                    .children(rows)
                    .into_any_element()
            })
            .collect();

        let detail = if let (Some(path), Some(method)) = (selected_path.clone(), selected_method) {
            let method_color = match method.as_str() {
                "GET" => theme.accent,
                "POST" | "PUT" | "PATCH" => theme.success,
                "DELETE" => theme.danger,
                _ => theme.muted_foreground,
            };
            let copy_path = path.clone();
            let copy_method = method.clone();
            let method_rows: Vec<AnyElement> = selected_operations
                .iter()
                .map(|operation| {
                    let active = operation.method == method;
                    let row_path = path.clone();
                    let row_method = operation.method.clone();
                    let row_color = match operation.method.as_str() {
                        "GET" => theme.accent,
                        "POST" | "PUT" | "PATCH" => theme.success,
                        "DELETE" => theme.danger,
                        _ => theme.muted_foreground,
                    };
                    let mut row = div()
                        .id(format!("api-method-{}", operation.method))
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .cursor_pointer()
                        .child(
                            div()
                                .w(px(68.0))
                                .flex_none()
                                .px_2()
                                .py_1()
                                .rounded(px(tokens::RADIUS_SMALL))
                                .bg(row_color.opacity(0.14))
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .text_center()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(row_color)
                                .child(operation.method.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .text_sm()
                                .text_color(theme.foreground)
                                .child(operation.summary.clone()),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.api_reference_selected_path = Some(row_path.clone());
                            this.api_reference_selected_method = Some(row_method.clone());
                            cx.notify();
                        }));
                    row = if active {
                        row.bg(theme.list_active)
                    } else {
                        row.hover(|style| style.bg(theme.list_hover))
                    };
                    row.into_any_element()
                })
                .collect();
            div()
                .flex_1()
                .min_w(px(0.0))
                .h_full()
                .overflow_y_scrollbar()
                .border_1()
                .border_color(theme.border)
                .rounded(px(tokens::RADIUS))
                .bg(theme.sidebar)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_5()
                        .py_4()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded(px(tokens::RADIUS_SMALL))
                                .bg(method_color.opacity(0.14))
                                .font_family(tokens::FONT_MONO)
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(method_color)
                                .child(method.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .font_family(tokens::FONT_MONO)
                                .text_sm()
                                .text_color(theme.foreground)
                                .child(path.clone()),
                        )
                        .child(mono_label("READ ONLY").text_color(theme.muted_foreground)),
                )
                .child(
                    div()
                        .v_flex()
                        .gap_4()
                        .p_5()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child(selected_summary),
                        )
                        .child(
                            div()
                                .v_flex()
                                .gap_2()
                                .child(mono_label("REQUEST").text_color(theme.muted_foreground))
                                .child(
                                    div()
                                        .w_full()
                                        .px_3()
                                        .py_3()
                                        .rounded(px(tokens::RADIUS_SMALL))
                                        .bg(theme.background)
                                        .font_family(tokens::FONT_MONO)
                                        .text_sm()
                                        .text_color(theme.foreground)
                                        .child(format!("{method} {path}")),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Try it 已关闭，仅用于查看 schema 和接口说明。"),
                                )
                                .child(
                                    Button::new("copy-api-path")
                                        .secondary()
                                        .outline()
                                        .compact()
                                        .label("复制路径")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                format!("{copy_method} {copy_path}"),
                                            ));
                                            this.notify("接口路径已复制".into(), cx);
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .border_t_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .px_5()
                                .py_3()
                                .child(mono_label("OPERATIONS").text_color(theme.muted_foreground)),
                        )
                        .children(method_rows),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .h_full()
                .border_1()
                .border_color(theme.border)
                .rounded(px(tokens::RADIUS))
                .bg(theme.sidebar)
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("OpenAPI schema 中暂无可展示的接口")
                .into_any_element()
        };

        let content = div().flex_1().min_h(px(0.0)).child(
            div()
                .w_full()
                .max_w(px(1200.0))
                .mx_auto()
                .h_full()
                .px_6()
                .py_6()
                .v_flex()
                .gap_5()
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(mono_label("SCHEMA OVERVIEW").text_color(theme.accent))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child(schema_title),
                        )
                        .child(
                            div()
                                .max_w(px(760.0))
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(schema_description),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .children([
                            div()
                                .flex_none()
                                .min_w(px(150.0))
                                .px_3()
                                .py_3()
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.sidebar)
                                .v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_lg()
                                        .text_color(theme.foreground)
                                        .child(schema_version),
                                )
                                .child(mono_label("VERSION").text_color(theme.muted_foreground)),
                            div()
                                .flex_none()
                                .min_w(px(150.0))
                                .px_3()
                                .py_3()
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.sidebar)
                                .v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .font_family(tokens::FONT_MONO)
                                        .text_lg()
                                        .text_color(theme.foreground)
                                        .child(format!("{endpoint_count}")),
                                )
                                .child(mono_label("ENDPOINTS").text_color(theme.muted_foreground)),
                        ])
                        .into_any_element(),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.0))
                        .flex()
                        .items_start()
                        .gap_5()
                        .child(
                            div()
                                .w(px(280.0))
                                .flex_none()
                                .h_full()
                                .overflow_y_scrollbar()
                                .border_1()
                                .border_color(theme.border)
                                .rounded(px(tokens::RADIUS))
                                .bg(theme.sidebar)
                                .child(
                                    div()
                                        .px_3()
                                        .py_3()
                                        .border_b_1()
                                        .border_color(theme.border)
                                        .child(
                                            mono_label("ENDPOINTS")
                                                .text_color(theme.muted_foreground),
                                        ),
                                )
                                .children(endpoint_nav),
                        )
                        .child(detail),
                ),
        );
        page.child(content)
    }

    /// Audit log page, mirroring the web audit page (`/audit`): a project
    /// picker plus per-entry rows of `action`, `actor_type:actor_id`, `path`
    /// and `created_at`. Entry point lives next to API Reference in the
    /// sidebar DEVELOPER TOOLS block.
    pub(crate) fn render_audit(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let page = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .w_full()
                    .max_w(px(1200.0))
                    .mx_auto()
                    .px_6()
                    .py_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        Button::new("close-audit")
                                            .ghost()
                                            .compact()
                                            .icon(IconName::ArrowLeft)
                                            .label("返回工作区")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.screen = Screen::Workspace;
                                                cx.notify();
                                            })),
                                    )
                                    .child(div().w(px(1.0)).h(px(20.0)).bg(theme.border))
                                    .child(
                                        div()
                                            .v_flex()
                                            .gap_1()
                                            .child(
                                                mono_label("DEVELOPER / AUDIT")
                                                    .text_color(theme.accent),
                                            )
                                            .child(
                                                crate::ui::display("审计日志")
                                                    .text_lg()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme.foreground),
                                            ),
                                    ),
                            )
                            .child(
                                Button::new("refresh-audit")
                                    .secondary()
                                    .outline()
                                    .compact()
                                    .icon(IconName::Redo2)
                                    .label(if self.audit_loading {
                                        "加载中…"
                                    } else {
                                        "刷新"
                                    })
                                    .loading(self.audit_loading)
                                    .disabled(self.audit_selected_project.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| this.load_audit(cx))),
                            ),
                    ),
            );
        let picker: AnyElement = if self.audit_projects.is_empty() {
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("暂无项目，无法查看审计日志。")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(self.audit_projects.iter().map(|p| {
                    let id = p.id.clone();
                    let selected = self.audit_selected_project.as_deref() == Some(p.id.as_str());
                    Button::new(format!("audit-project-{}", p.id))
                        .secondary()
                        .outline()
                        .compact()
                        .label(p.name.clone())
                        .selected(selected)
                        .toggled(selected)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.audit_selected_project = Some(id.clone());
                            this.load_audit(cx);
                        }))
                        .into_any_element()
                }))
                .into_any_element()
        };
        let entries: AnyElement = if let Some(error) = &self.audit_error {
            div()
                .py_4()
                .text_sm()
                .text_color(theme.danger)
                .child(error.clone())
                .into_any_element()
        } else if self.audit_projects.is_empty() {
            // No projects on the server: the web page renders nothing here.
            div().into_any_element()
        } else if self.audit_loading {
            div()
                .py_4()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("正在加载审计日志…")
                .into_any_element()
        } else if self.audit_entries.is_empty() {
            div()
                .py_4()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("暂无审计记录。")
                .into_any_element()
        } else {
            let mut rows = div().v_flex();
            for e in &self.audit_entries {
                rows = rows.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .py_3()
                        .border_b_1()
                        .border_color(theme.border)
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
                                        .child(mono_label(&e.action).text_color(theme.accent))
                                        .child(
                                            mono_label(format!("{}:{}", e.actor_type, e.actor_id))
                                                .text_color(theme.muted_foreground),
                                        ),
                                )
                                .child(
                                    mono_label(if e.path.is_empty() {
                                        "—".into()
                                    } else {
                                        e.path.clone()
                                    })
                                    .text_color(theme.muted_foreground),
                                ),
                        )
                        .child(
                            mono_label(format_audit_time(&e.created_at))
                                .flex_shrink_0()
                                .text_color(theme.muted_foreground),
                        ),
                );
            }
            rows.into_any_element()
        };
        page.child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scrollbar()
                .flex()
                .items_start()
                .justify_center()
                .child(
                    div()
                        .w_full()
                        .flex_none()
                        .max_w(px(920.0))
                        .p_6()
                        .v_flex()
                        .gap_4()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("按项目查看操作记录：谁在什么时间对哪个文档执行了什么操作。"),
                        )
                        .child(
                            div()
                                .v_flex()
                                .gap_2()
                                .child(mono_label("PROJECT").text_color(theme.muted_foreground))
                                .child(picker),
                        )
                        .child(
                            div()
                                .v_flex()
                                .gap_2()
                                .child(
                                    mono_label(format!("ENTRIES · {}", self.audit_entries.len()))
                                        .text_color(theme.muted_foreground),
                                )
                                .child(div().border_t_1().border_color(theme.border).child(entries)),
                        ),
                ),
        )
    }

    fn render_shell_topbar(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let project = match &self.screen {
            Screen::Settings => "设置".to_string(),
            Screen::ApiReference => "API Reference".to_string(),
            Screen::Audit => "审计日志".to_string(),
            _ => self
                .selected_project
                .as_deref()
                .unwrap_or("workspace")
                .to_string(),
        };
        let document = self
            .doc_path
            .as_deref()
            .map(|path| format!(" / {}", tokens::truncate(path, 72)))
            .unwrap_or_default();
        div()
            .h(px(tokens::TOOLBAR_H))
            .px_4()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(app_icon().size(px(24.0)))
                            .child(mono_label("AgentDocs").text_color(theme.accent)),
                    )
                    .child(div().w(px(1.0)).h(px(16.0)).bg(theme.border))
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{}{}", tokens::truncate(&project, 40), document)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_1()
                            .rounded(px(tokens::RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} K", tokens::MOD_KEY)),
                    )
                    .child(
                        Button::new("shell-quick-open")
                            .ghost()
                            .compact()
                            .icon(IconName::Search)
                            .tooltip(format!("快速打开 ({} P)", tokens::MOD_KEY))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_quick_open(window, cx)
                            })),
                    )
                    .child(
                        Button::new("shell-theme")
                            .ghost()
                            .compact()
                            .icon(if cx.theme().is_dark() {
                                IconName::Sun
                            } else {
                                IconName::Moon
                            })
                            .tooltip(format!("切换主题 ({} Shift T)", tokens::MOD_KEY))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
                    )
                    .child(
                        Button::new("shell-settings")
                            .ghost()
                            .compact()
                            .selected(matches!(&self.screen, &Screen::Settings))
                            .toggled(matches!(&self.screen, &Screen::Settings))
                            .icon(IconName::Settings)
                            .tooltip("打开设置")
                            .disabled(self.editing)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.screen = Screen::Settings;
                                this.load_settings_access(cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .font_family(tokens::FONT_MONO)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(self.username.clone()),
                    ),
            )
    }
}
