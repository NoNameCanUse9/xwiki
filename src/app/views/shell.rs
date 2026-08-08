//! Persistent authenticated desktop shell: topbar, content and status area.

use gpui::*;
use gpui_component::{button::*, *};

use crate::app::{Screen, XWikiApp};
use crate::ui::{mono_label, tokens};

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

    fn render_shell_topbar(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let project = match &self.screen {
            Screen::Settings => "设置".to_string(),
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
                    .child(mono_label("AgentDocs").text_color(theme.accent))
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
