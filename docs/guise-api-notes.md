# guise 迁移 API 笔记（基建层产出，供视图层 worker 参考）

> 来源：guise-ui 0.10.0 源码（`/d/Rust/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/guise-ui-0.10.0/`）+ gpui 0.2.2 源码。所有签名均从源码确认，标注位置供复核。

## 1. App 入口（main.rs 已改好）

```rust
gpui::Application::new().run(move |cx: &mut App| {
    ui::tokens::cobalt_light().init(cx); // 或 cobalt_dark()，先于任何窗口
    cx.bind_keys([...]);
    cx.spawn(async move |cx| {
        let bounds = cx.update(|cx| Bounds::centered(None, size(px(1024.0), px(680.0)), cx)).expect("app was released");
        cx.open_window(WindowOptions { ... }, |window, cx| cx.new(|cx| XWikiApp::new(window, cx)))
            .expect("Failed to open window")
            .activate_window(); // AsyncApp 无 activate，用 WindowHandle::activate_window()
    }).detach();
});
```

**坑**：`AsyncApp::update` 返回 `Result<R>`（async_context.rs:142）；`AsyncApp` 没有 `activate`，用 `WindowHandle::activate_window()`（window.rs:4112）。

## 2. Theme（theme/mod.rs）

- 构建：`guise::theme::Theme::light()/dark()` + `with_primary/with_body/with_surface/with_surface_hover/with_text/with_dimmed/with_border/with_success/with_warning/with_danger/with_info`，接受 `impl Into<Hsla>`（如 `guise::theme::rgb(r,g,b)` 或 `Color::hex("#rrggbb")`）。
- 安装：`Theme::init(cx)` = `cx.set_global(theme)`（Theme: Global）。
- **读取（视图用）**：`guise::theme::theme(cx) -> &Theme`（等价 `cx.global::<Theme>()`）。
- 语义色 getter：`body()/surface()/surface_hover()/text()/dimmed()/border()/primary()/success()/warning()/danger()/info()`，返回 `Color`，转 Hsla 用 `.hsla()`，带 alpha 用 `.alpha(a)`，反色用 `.contrasting()`（color.rs:66-90）。
- `ColorScheme::{Light,Dark}` + `scheme.is_dark()`。
- **切换主题**：`let t = tokens::cobalt_dark(); cx.set_global(t); cx.notify();`（视图 toggle_theme 用这个替代 `Theme::change`）。
- 已有映射：`ui::tokens::cobalt_light()/cobalt_dark()`、`Cobalt::from_theme(&Theme)`（保留旧字段名：paper/paper_2/rule/ink/ink_2/ink_3/accent/accent_ink/surface_accent/danger/danger_ink/graphite）。
- **惯例（architecture.md）**：在 `cx.listener` 之前先把主题值解析成局部变量（`theme(cx)` 不可变借用 cx）。

## 3. 关键组件构造签名（全部源码确认）

### Button（button.rs）

```rust
Button::new(id: impl Into<ElementId>, label: impl Into<SharedString>)
    .variant(Variant::{Filled, Light, Outline, Subtle, Default})
    .color(ColorName | Hsla | color!宏)   // ColorValue，style.rs:105
    .size(Size::{Xs, Sm, Md, Lg, Xl})      // 默认 Sm（36px 高）
    .radius(Size)
    .full_width(bool)
    .disabled(bool)
    .left_section(impl IntoElement)   // 图标放这里，替代旧 .icon()
    .right_section(impl IntoElement)
    .on_click(impl Fn(&ClickEvent, &mut Window, &mut App) + 'static)
```

注意：`guise::button` 模块是私有的，从 crate 根导入 `use guise::Button;`。旧 `.compact()` 无对应——用 `.size(Size::Xs)` 缩小。

### Icon / IconName（icon/lucide.rs、icon/mod.rs）

```rust
use guise::{Icon, IconName, Glyph};
Icon::new(IconName::Search).size(Size).color(ColorName)
```

- 内置 Lucide 1991 图标，**无 asset pipeline**。
- `Glyph::{Lucide(IconName), Text(...)}`，`From<IconName>/&str/String`。
- 旧 gpui-component 的 IconName::File/Inbox/Search/Plus/Close/ArrowLeft/ArrowRight/ChevronRight/EllipsisVertical/Undo2/Redo2/Sun/Moon/Settings/Network/Check/Delete/CircleX/CircleCheck/Folder/Eye 等均为 Lucide 名，基本一一对应（驼峰命名，如 `IconName::ArrowLeft`、`IconName::ChevronRight`）。
- **旧 `.text_color(...)` 无对应**——Icon 颜色用 `.color(ColorName)`（主题色）或继承父元素文本色。要精确色值：用 `div().text_color(hsla).child(Icon::new(...))` 包裹，或自定义 Glyph。

### TextInput（input/text.rs）—— 替代 InputState + Input

```rust
// 创建（entity-based）：
let input: Entity<TextInput> = cx.new(|cx| TextInput::new(cx).placeholder("...").label("..."));
// 渲染：.child(input.clone())     // impl<V: Render> IntoElement for Entity<V> (gpui view.rs:296)
// 事件：cx.subscribe(&input, |this, _input, event: &TextInputEvent, cx| match event {
//     TextInputEvent::Change(text) | TextInputEvent::Submit(text) => ...
// })
// 读写：input.read(cx).text() / input.update(cx, |s, cx| s.set_text("", cx))
// 绑定：TextInput::bind(&entity, &signal, cx)
```

**重大变化**：旧代码 `Entity<InputState>` + `Input::new(&entity)` 全部改为 `Entity<TextInput>` + `.child(entity)`。视图层每个输入框字段（filter_input/search_input/history_input/editor_input/editor_title_input/attachment_*/reset_*/login 字段等）都要改类型并替换 `Input::new(&x)` 为 `.child(x.clone())`。事件订阅签名：`cx.subscribe(&input, |_, _, event: &TextInputEvent, _| ...)`。

### Modal（overlay/modal.rs）—— 替代 window.open_dialog

guise 没有全局 dialog API。**必须在根视图持有 `Entity<OverlayHost>`**（overlay/host.rs）：

```rust
// 根视图创建（app/mod.rs 的 render 里，最后一个 child）：
let overlays = cx.new(OverlayHost::new);   // 存到 XWikiApp 字段
div().child(page_content).child(overlays.clone())
// 任何 handler：
overlays.update(cx, |host, cx| {
    host.open_modal(window, cx, |close, _window, _cx| {
        Modal::new().title("...").on_close(move |_ev, window, cx| close(window, cx))
            .child(...).into_any_element()
    });
    // host.close_top(window, cx) / host.close_modal(id, ...)
});
```

**TODO（视图层）**：XWikiApp 需新增 `overlay_host: Entity<OverlayHost>` 字段；所有 `window.open_dialog(...)` 改成 `host.open_modal(...)`；`ConfirmModal::new().title().message().confirm_label().cancel_label().danger().on_confirm(|_, w, c| ...).on_cancel(...)`（overlay/confirm.rs）。

### Notification / Toast（feedback/notification.rs、toast.rs）—— 替代 cx.push_notification

```rust
host.toast("消息", cx);                    // OverlayHost::toast
host.toast_titled("标题", "消息", ColorName::Green, cx);
// 或 ToastStack 实体：host.toast_stack()
```

**TODO（视图层）**：`XWikiApp::notify()` 方法改为 `overlay_host.update(cx, |host, cx| host.toast(msg, cx))`。`Notification::new(msg).title(...).color(ColorName).icon(Glyph)` 是 builder（feedback/notification.rs:22），由 ToastStack 渲染。

### Tooltip（overlay/tooltip.rs）

```rust
.tooltip(Text::new("提示"))   // 自由函数 tooltip(...)，overlay/tooltip.rs:49；注意与 gpui-component 的 .tooltip("str") 不同
```

具体挂载方式见 overlay/tooltip.rs 顶部注释（返回 `Tooltip::new(label)` builder）。**待视图层核验挂载 API**。

### ScrollArea（scrollarea.rs）—— 替代 ScrollableElement

```rust
ScrollArea::new(id).max_height(f32).horizontal(bool)   // 内部滚动
```

旧 `.overflow_y_scrollbar()/.overflow_x_scrollbar()/.track_scroll(&handle)` 无直接对应：`ScrollArea` 实体实现 `Scrollable`，可用 `.scroll_handle()`。**待视图层核验**。

### Markdown 只读渲染（markdown/editor.rs）

```rust
let editor = cx.new(|cx| MarkdownEditor::new(cx).value(text).read_only(true));
div().child(editor)
```

已封装为 `ui::markdown(cx: &mut App, content: String) -> Div`（新建 entity 每帧重建，可后续缓存到视图字段）。旧调用 `ui::markdown(format!("doc-content-{index}"), src)` → `ui::markdown(cx, src)`。链接点击事件：`cx.subscribe(&editor, |_, _, e: &MarkdownEditorEvent, _| ...)`（LinkClick）。

## 4. gpui 0.2.2 陷阱（视图层必读）

1. **`ElementId` 无 `From<String>`**（window.rs:4864-4963，只有 `&'static str`/`usize`/`SharedString`/`(&str, EntityId)`）。所有 `format!(...)` 传给 `.id()`/`Button::new()` 的地方要用 `ElementId::named_usize(name, i)` 或 `(&'static str, EntityId)` 元组。
2. **`cx.theme()` 不存在**——用 `guise::theme::theme(cx)`（见 §2）。
3. **`BoxShadow::new(...)` 改版**：gpui 0.2.2 的 BoxShadow 构造不同（app/mod.rs:3703 报 E0599）。用 `gpui::BoxShadow { color, offset, blur_radius, spread_radius }` 字面量或查 gpui 0.2.2 shadow 文档。**待核验**。
4. **`v_flex/overflow_y_scrollbar/context_menu/...` 消失**——这些是 gpui-component 的扩展 trait（StyledExt/ScrollableElement/ContextMenuExt）。全部改为 guise 组件或裸 gpui Div + flex。
5. **`WindowOptions` 无 `icon` 字段**（platform.rs:1089）。`app_icon_rgba()` 保留但已无调用点（#[allow(dead_code)]）。
6. **`Size<Pixels>` 字段**：`size.x` 报 E0609（document.rs:640）——`Size` 的字段访问方式变化，用 `.width` 或 `.into()`。**待核验**。
7. **布局助手**：guise 提供 `layout::{Stack, Group, Center, Container, SimpleGrid, Space, Align, Justify}`、`flex::{Row, Column, Expanded, ...}`（需 `use guise::flex::*;`，prelude 不含）、宏 `row!/col!/vstack!/hstack!`。旧 `.flex()/.flex_col()/.items_center()/.justify_between()/.gap_2()/.w_full()/.px_3()/.py_3()/.rounded()/.border_1()/.bg()/.text_sm()` 等裸 gpui Div 方法大多还在（gpui 核心 Styled），可继续用。
8. **主题色访问**：旧 `theme.background/sidebar/foreground/muted_foreground/border/accent/danger/list_hover/list_active/skeleton/input` 全部换成 guise 语义色（body/surface/text/dimmed/border/primary/danger/surface_hover...），或 `tokens::Cobalt::from_theme(&t)` 字段。

## 5. 已完成（基建层）

- `Cargo.toml`：gpui 0.2.2 (crates.io) + guise-ui 0.10，移除 gpui_platform/gpui-component/gpui-component-assets
- `src/main.rs`：Application::new()、Theme::init、WindowOptions 无 icon、activate_window
- `src/ui/mod.rs`：app_icon/refresh_icon（gpui Image::from_bytes + img(Arc<Image>)）、mono_label/body/display（不变）、markdown(cx, content)、clear_search_button(id, Entity<TextInput>, app_id)
- `src/ui/tokens.rs`：cobalt_light()/cobalt_dark()、Cobalt::from_theme(&guise Theme)
- `src/config.rs`：自建 ThemeMode 枚举（原 gpui_component::ThemeMode）
- `src/ui/split_pane.rs`：**未改动**（gpui 0.2.2 核心 API 兼容，已确认 on_drag/on_drag_move/mouse_position/click_count/CursorStyle 均存在）

## 6. 剩余错误分布（视图层，220 个）

- src/app/mod.rs ≈ 67（最大：theme 读取、对话框、通知、BoxShadow、滚动、InputState 字段）
- document.rs ≈ 41、shell.rs ≈ 24、settings.rs ≈ 22、workspace.rs ≈ 19、history.rs ≈ 15、login.rs ≈ 14、editor.rs ≈ 10
- 通用修复模式：`use gpui_component::*` → `use guise::prelude::*`；`cx.theme()` → `theme(cx)`；Input/InputState → TextInput entity；Button 构造器；`window.open_dialog` → OverlayHost；`push_notification` → host.toast；IconName 名基本不变。
