use gpui::*;
use gpui_component::{
    button::*,
    input::{Input, InputEvent, InputState},
    *,
};

/// Application shell: login screen and the project workspace skeleton.
/// Theme discipline (Hallmark · Cobalt): cool engineered paper, hairlines
/// over shadows, exactly one electric-blue signal (primary button, focus,
/// hover underlines), mono UPPERCASE labels, 6px radii.
pub struct XWikiApp {
    screen: Screen,
    server_input: Entity<InputState>,
    user_input: Entity<InputState>,
    password_input: Entity<InputState>,
    /// Keep input subscriptions alive with the app entity.
    _subscriptions: Vec<Subscription>,
}

enum Screen {
    Login,
    Workspace,
}

impl XWikiApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let server_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("http://127.0.0.1:9090")
        });
        let user_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("用户名"));
        let password_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("密码").masked(true));

        let mut subs = Vec::new();
        for state in [&server_input, &user_input, &password_input] {
            subs.push(cx.subscribe_in(state, window, |_, _, _: &InputEvent, _, cx| {
                cx.notify()
            }));
        }

        Self {
            screen: Screen::Login,
            server_input,
            user_input,
            password_input,
            _subscriptions: subs,
        }
    }

    fn login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // ponytail: UI skeleton only — wire the real /api/v1/auth/login call
        // when the api layer lands.
        self.screen = Screen::Workspace;
        cx.notify();
    }

    fn logout(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Login;
        cx.notify();
    }

    fn eyebrow(&self, label: &'static str, cx: &Context<Self>) -> Div {
        div()
            .font_family("JetBrains Mono")
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(label)
    }

    fn render_login(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .w(px(360.0))
                    .p_6()
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(self.eyebrow("AgentDocs", cx))
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("文档工作台"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Git-backed 文档系统 · 团队协作"),
                    )
                    .child(Input::new(&self.server_input).w_full())
                    .child(Input::new(&self.user_input).w_full())
                    .child(Input::new(&self.password_input).w_full())
                    .child(
                        Button::new("login")
                            .primary()
                            .w_full()
                            .rounded(px(6.0))
                            .label("登录")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.login(window, cx)
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .font_family("JetBrains Mono")
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("v0.8.0")
                            .child("/api/v1"),
                    ),
            )
    }

    fn render_workspace(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Top bar: flush, hairline bottom border, mono labels.
                div()
                    .h(px(44.0))
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
                            .child(self.eyebrow("AgentDocs", cx))
                            .child(
                                div()
                                    .w(px(1.0))
                                    .h(px(16.0))
                                    .bg(theme.border),
                            )
                            .child(
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("docs-site"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                // Bordered ⌘K affordance — the command palette
                                // wires in with the desktop feature set.
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .px_2()
                                    .py_1()
                                    .rounded(px(6.0))
                                    .border_1()
                                    .border_color(theme.border)
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("⌘K"),
                            )
                            .child(
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("admin"),
                            )
                            .child(
                                Button::new("logout")
                                    .rounded(px(6.0))
                                    .label("退出")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.logout(window, cx)
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .size_full()
                    .child(
                        // Project rail: mono section label, hairline divider.
                        div()
                            .w(px(240.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(theme.border)
                            .bg(theme.sidebar)
                            .child(
                                div()
                                    .px_3()
                                    .py_3()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("PROJECTS"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px_3()
                                    .py_1_5()
                                    .hover(|s| s.bg(theme.list_hover))
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.foreground)
                                            .child("docs-site"),
                                    )
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("—"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px_3()
                                    .py_1_5()
                                    .hover(|s| s.bg(theme.list_hover))
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.foreground)
                                            .child("handbook"),
                                    )
                                    .child(
                                        div()
                                            .font_family("JetBrains Mono")
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("—"),
                                    ),
                            ),
                    )
                    .child(
                        // Content area: empty state.
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("选择一个项目"),
                            )
                            .child(
                                div()
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("左侧列表 · 或 ⌘K 快速跳转"),
                            ),
                    ),
            )
    }
}

impl Render for XWikiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.screen {
            Screen::Login => self.render_login(cx),
            Screen::Workspace => self.render_workspace(cx),
        }
    }
}
