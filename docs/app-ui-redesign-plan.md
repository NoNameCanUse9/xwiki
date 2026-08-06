# AgentDocs App UI 重设计计划 — Cobalt Desktop Workspace

> 目标:让 Rust/GPUI App 与网页端共享 Hallmark Cobalt 设计系统,但布局和交互按桌面生产力工具重新设计。
>
> 方法论来源:
>
> - **baoyu-design**:先读取现有 UI/设计系统,将其作为绑定视觉约束;复用令牌、组件语汇、密度和状态,再通过可运行结果验证。
> - **ui-ux-pro-max**:先确定设计系统,再审查键盘、焦点、加载、空状态、错误恢复、对比度和 reduced motion;排除移动端专属规则。
>
> 设计源:`web/src/index.css`、`web/src/components/`、`web/src/routes/`。
> 技术目标:Rust + GPUI + gpui-component 桌面应用,不是 WebView,不复制 Web 路由模型。

## 1. 平台定位与设计原则

### 1.1 产品定位

AgentDocs App 是面向长时间阅读、编辑和版本管理的**桌面文档工作台**。主要输入方式是键盘、鼠标和触控板;主要结构是持续存在的 workspace shell、可调整分栏和命令系统。

### 1.2 与网页端的关系

- **共享**:Cobalt 色彩、字体语气、6px 圆角、hairline、组件状态、内容排版和品牌表达。
- **适配**:桌面窗口、侧栏、树导航、分栏、快捷键、右键菜单、高 DPI 和持久化工作区。
- **不复制**:Web routes、移动端底部导航、safe area、haptics、44/48dp 触摸目标、浏览器 z-index/ARIA/skip-link 方案。
- “视觉一致”不等于“页面 1:1”;桌面端优先信息密度、键盘效率和多面板并行。

### 1.3 视觉方向

UI/UX Pro Max 检索结果支持 **Minimalism & Swiss Style**:高可读性、网格、少量颜色、完整 Light/Dark、清晰层级;与现有 Cobalt 方向一致。

- genre:`modern-minimal`
- shell:`desktop side-rail workspace`
- 浅色:cool engineered near-white
- 深色:graphite register
- 唯一信号色:electric cobalt
- 阴影只表达浮层;常规层级优先使用背景明度和 hairline
- 不使用 emoji 作为结构图标;使用 gpui-component/矢量图标

---

## Phase 0 · 桌面平台契约与现状审计

### 0.1 当前基础

- `src/app.rs` 约 1984 行,包含 Login、Workspace、文档树、阅读、编辑、History、Dialog 和 Command Palette。
- 已有 `src/themes/cobalt.json`、Light/Dark 切换、`Ctrl/Cmd+K`、`Ctrl/Cmd+Shift+T`。
- 已使用 gpui-component 的 Button、Input、Dialog、Notification、TextView。

### 0.2 Gap 清单

| 维度 | 当前状态 | 需要补齐 |
| --- | --- | --- |
| 设计令牌 | `cobalt.json` 有近似色值 | 精确映射 web oklch + 自定义语义令牌 |
| 字体 | 默认字体,局部 JetBrains Mono | 跨平台 Latin/CJK/Mono 字体回退矩阵 |
| 密度 | 尺寸零散硬编码 | 4px 网格 + compact/cozy 两级密度 |
| 窗口 | 单一布局 | 最小尺寸、缩放规则、窄窗降级、多 DPI |
| 面板 | 固定侧栏/内容区 | 可调整宽度、折叠、最小宽度、状态持久化 |
| 键盘 | 少量全局快捷键 | 焦点顺序、树导航、编辑、关闭和快速打开 |
| 鼠标 | click/hover 为主 | tooltip、右键菜单、拖动分隔线、双击规则 |
| 异步反馈 | loading/status 零散 | loading/disabled/success/error/retry/conflict 统一状态 |
| 可访问性 | 部分 focus/contrast | 键盘全流程、focus 恢复、Light/Dark 对比度 |
| 代码结构 | `app.rs` 单文件 | 状态编排、UI 组件、workspace 布局和业务视图分离 |

### 0.3 桌面窗口契约

初始建议值,实现前用现有内容验证:

- 默认窗口:`1280 × 800`
- 最小窗口:`960 × 640`
- 左侧 workspace panel:默认 260px,范围 220–360px
- History panel:默认 360px,范围 300–520px
- 主阅读区:最小 480px;正文 measure 建议 720–820px
- 窗口不足时按顺序关闭/折叠:History → 次级信息区 → 项目导航,主内容不被压扁
- 分隔线支持拖动;双击恢复默认;宽度、折叠状态和最后工作区持久化
- 在 Windows 100%/125%/150% DPI 下验证 hairline、图标和文本清晰度

---

## Phase 1 · Cobalt 设计系统映射

### 1.1 令牌架构

采用三层令牌,避免视图直接写颜色和尺寸:

1. **Primitive**:网页端 oklch → sRGB hex 的浅/深基础值。
2. **Semantic**:`paper`、`paper-2`、`ink`、`ink-2`、`ink-3`、`rule`、`rule-2`、`accent`、`surface-accent`、`graphite`、`danger`。
3. **Component**:`button-primary-bg`、`tree-selected-bg`、`panel-border`、`input-focus-ring` 等。

映射策略:

- gpui-component schema 有槽位的写入 `src/themes/cobalt.json`。
- 无槽位的写入 `src/ui/tokens.rs`。
- 视图层禁止新增未经设计系统定义的颜色。
- Light/Dark 分别定义 hover/active/focus/disabled,不能只反转背景。

### 1.2 字体与中文回退

不打包字体,使用系统字体优先 + 明确回退:

| 场景 | Windows | macOS | Linux |
| --- | --- | --- | --- |
| 正文 | Segoe UI → Microsoft YaHei UI | SF Pro Text → PingFang SC | Inter → Noto Sans CJK SC |
| Display | Space Grotesk → Segoe UI | Space Grotesk → SF Pro Display | Space Grotesk → Inter |
| Mono | JetBrains Mono → Cascadia Mono | JetBrains Mono → SF Mono | JetBrains Mono → Noto Sans Mono |

实现时先确认 GPUI 的字体族 API 是否支持字体列表;如果只支持单字体族,按平台选择首个已安装字体并回退到 GPUI 默认字体。中文标题、正文、路径和中英混排必须单独验收。

### 1.3 字号、间距和密度

- Mono label:11px / Medium / tracked / uppercase 仅用于英文机器标签;中文标签不强制 uppercase/letter-spacing。
- Body:15px / line-height 1.55–1.65。
- Display:24px 页面标题、32px 项目标题;登录品牌不超过 40px。
- 4px spacing grid:4/8/12/16/20/24/32/40。
- radius:4px 小控件、6px 常规、10px 代码/浮层;禁止无理由 pill。
- 桌面行高建议:compact 28–30px,cozy 34–36px;不会采用移动端 44/48dp 规则。

### 1.4 状态与动效

- focus:2px cobalt ring + 清晰 offset,所有键盘可达元素一致。
- hover:surface-accent,不做大面积 cobalt flood。
- active:相对 hover 再下降一档明度。
- disabled:同时降低对比度并禁止操作,不能只改颜色。
- selection:accent 背景/边线 + 合法前景对比。
- 动效:120–220ms;进入 ease-out、退出 ease-in;只动关键元素。
- reduced motion:关闭位移/缩放,保留必要的颜色和显隐反馈。

---

## Phase 2 · Desktop Workspace Shell

### 2.1 持续工作区结构

桌面端不按 Web routes 逐页切换,使用持续存在的 shell:

1. **Titlebar/Topbar**:应用名、当前项目/文档、全局命令、主题、账户。
2. **Primary side rail**:Workspace、最近项目、设置等高层入口。
3. **Project panel**:项目列表或当前项目文档树,根据上下文切换。
4. **Main content**:项目网格、文档阅读或编辑器。
5. **Context panel**:History、版本详情、diff;按需打开。
6. **Status area**:服务器连接、锁状态、revision、保存状态。

### 2.2 窄窗口降级

- ≥1280px:完整三栏/四区布局。
- 960–1279px:History 作为可开关右侧面板;项目 panel 保留。
- 接近最小宽度:primary rail 压缩为图标;文档树允许临时 overlay/切换,主内容优先。
- 任何尺寸下不得出现无法访问的操作或被遮挡的保存/关闭按钮。

### 2.3 原生窗口行为

实现前确认并记录:

- 保留系统 titlebar 或自定义 titlebar 的选择;若自定义,必须保留 Windows 拖动、最小化、最大化、关闭区域。
- 窗口标题格式:`AgentDocs — 项目 / 文档`。
- 系统主题变化与手动主题的优先级。
- 多显示器/DPI 切换时重新布局,不缓存错误像素尺寸。

---

## Phase 3 · 桌面交互与命令模型

### 3.1 键盘模型

所有核心任务必须不依赖鼠标完成:

- `Tab / Shift+Tab`:按视觉顺序跨区域移动焦点。
- 文档树:`↑/↓` 移动、`←/→` 折叠展开、`Enter` 打开。
- `Esc`:依优先级关闭 Dialog → Command Palette → History/context panel → 编辑临时状态。
- `Ctrl/Cmd+K`:命令面板。
- `Ctrl/Cmd+P`:快速打开项目/文档。
- `Ctrl/Cmd+S`:保存;编辑外不触发无意义操作。
- `Ctrl/Cmd+Shift+T`:主题切换。
- 快捷键在按钮 tooltip 和 Command Palette 中可发现。
- Dialog 关闭后焦点返回触发控件;打开编辑器后焦点进入正文,不得形成焦点陷阱。

GPUI 中优先使用 action/key binding 体系,避免每个控件各自手写重复按键判断。

### 3.2 鼠标和触控板

- 所有图标按钮有 tooltip。
- 项目/文档支持右键上下文菜单;危险操作必须二次确认。
- 分隔线支持 hover affordance、拖动和双击重置。
- 单击用于选择/打开;除非功能明确,不依赖双击作为唯一入口。
- 文档树展开箭头与行选择区域分离,避免误触。
- 独立滚动区不得抢夺错误滚轮目标;阅读区和侧栏保留明确滚动边界。

### 3.3 命令与异步安全

- 异步命令执行期间禁用重复提交。
- 保存/创建/登录显示进行中状态和结果。
- revision/锁冲突使用专用恢复界面,提供“重新加载”“保留草稿”“取消”等下一步。
- 网络错误提供 retry,不能只显示红色文字或 toast。

---

## Phase 4 · 组件与状态系统

所有组件从 tokens 获取颜色、尺寸和状态:

- `Button`:primary/secondary/ghost/destructive/icon;支持 loading、disabled、shortcut hint。
- `Input/TextArea`:持久 label、placeholder、inline error、focus ring;错误不能只靠红边。
- `Dialog`:focus 初始位置、focus return、Esc、默认/危险按钮顺序。
- `ProjectCard`:名称、描述、更新时间、归档状态、hover/focus parity。
- `Tree`:选中、展开、loading、空目录、键盘导航、右键菜单。
- `SplitPane`:拖动、最小/最大、折叠、双击重置、持久化。
- `Breadcrumb`:长路径截断 + tooltip,可键盘访问。
- `Badge/MonoLabel`:状态和机器元数据,不承担主体说明。
- `Notification/InlineAlert`:成功、警告、错误、恢复操作。
- `EmptyState`:说明当前为空并提供一个明确 CTA。
- `LoadingState`:局部 skeleton/progress;不能让整个窗口无反馈冻结。
- `ConflictState`:revision、锁、离线和权限问题专用状态。

组件验收必须覆盖 normal/hover/focus/active/disabled/loading/error 和 Light/Dark。

---

## Phase 5 · 业务视图

### 5.1 Login

- 紧凑居中面板,不做网页 landing hero。
- 服务器 URL、用户名、密码有持久 label。
- Enter 提交,执行中禁用,错误显示在相关字段/表单附近。
- 登录成功进入最后工作区或 Workspace。

### 5.2 Workspace

- 左侧可见最近/全部项目;主区显示项目卡片网格。
- 网格按窗口宽度变为 1–4 列,卡片保持合理最小宽度。
- 搜索支持键盘即时过滤;空结果与“没有项目”使用不同空状态。
- 新建项目 Dialog 关闭后焦点返回“新建项目”。

### 5.3 Document Workspace

- Project panel 显示文档树;Main content 显示阅读或编辑。
- 阅读区保持 720–820px measure,窗口宽时不无限拉长正文。
- 代码块使用 graphite surface,支持横向滚动和复制。
- 长标题、长路径、中文段落、附件和空目录必须正确布局。

### 5.4 Editor

- 明确编辑/只读、锁、未保存、保存中和冲突状态。
- `Ctrl/Cmd+S` 保存;关闭/切换前处理未保存内容。
- 提交信息使用 Dialog 或明确的提交区域,不挤压文档标题栏。
- 锁 heartbeat 失败时保留本地文本并给出恢复路径。

### 5.5 History/Diff

- 作为右侧 context panel,可调整宽度和关闭。
- commit 行 message 优先,sha/time/author 为次级 mono 信息。
- 支持键盘选择 commit;详情与 diff 保持选中同步。
- revert 显示影响范围和确认步骤,成功后刷新 revision。

### 5.6 Settings/Connection

- 服务器 URL、连接状态、当前用户、主题和密度设置集中管理。
- 连接测试显示明确成功/失败和恢复建议。

---

## Phase 6 · 代码结构实施

目标结构:

```text
src/
├─ app.rs                 # 顶层状态机、任务编排、Screen/Workspace 状态
├─ ui/
│  ├─ mod.rs
│  ├─ tokens.rs           # semantic/component tokens、spacing、type scale
│  ├─ button.rs
│  ├─ states.rs           # loading/empty/error/conflict
│  ├─ split_pane.rs
│  └─ ...
├─ views/
│  ├─ login.rs
│  ├─ workspace.rs
│  ├─ document.rs
│  ├─ editor.rs
│  ├─ history.rs
│  └─ settings.rs
└─ themes/
   └─ cobalt.json
```

实施原则:

- 先迁移纯视觉组件,再迁移业务视图,避免一次性重写导致功能回归。
- 状态和 API 调用暂留在 app/domain 层,视图仅接收所需状态和 action。
- 先定义 action/command,再绑定按钮和快捷键,保证鼠标/键盘共享同一逻辑。
- 每个迁移阶段保持 `cargo build` 可通过。

---

## Phase 7 · 验证矩阵

### 7.1 功能与视觉

- 窗口:`960×640`、`1280×800`、`1920×1080`。
- DPI:Windows 100%/125%/150%。
- 主题:Light/Dark 分别检查全部状态。
- 内容:中文、英文、中英混排、长标题、长路径、空内容和大量内容。
- 状态:loading、empty、error、offline、permission、lock/revision conflict。

### 7.2 桌面交互

- 仅使用键盘完成:登录 → 打开项目 → 打开文档 → 编辑 → 保存 → 查看历史。
- 检查 Tab 顺序、树方向键、Esc 关闭顺序和 focus return。
- 检查分栏拖动、折叠、双击恢复和窗口缩放。
- 检查按钮 tooltip、快捷键提示和右键菜单。
- 检查异步命令不能重复提交。

### 7.3 可访问性

- 正常文本对比度 ≥4.5:1;大文字/图标 ≥3:1。
- Dark 模式单独测量,不能假设 Light 色值自动合格。
- focus 在 Light/Dark、hover/selected 状态下始终可见。
- reduced motion 下无位移/缩放依赖。
- 错误有文本说明和恢复动作,不能只靠颜色。
- 若 GPUI 当前无法提供完整屏幕阅读器语义,记录为平台限制,但仍保证键盘、焦点、标签和视觉对比度。

### 7.4 构建与回归

- 修改前后运行 LSP diagnostics。
- `cargo test`。
- `cargo build` / `cargo build --release`。
- API/e2e 条件允许时连接 `http://127.0.0.1:9090` 验证真实项目数据。
- Windows 原生运行至少一轮;WSLg 作为补充,不能替代 Windows DPI 验证。

---

## 交付顺序

1. 桌面平台契约 + tokens/font fallback。
2. Cobalt Light/Dark 和组件状态。
3. SplitPane + Desktop Workspace Shell。
4. 键盘/focus/command 模型。
5. Workspace 和 Document Workspace。
6. Editor、History、Settings。
7. 完整桌面验证矩阵和回归修复。

## 最终验收标准

- 视觉上与网页端属于同一 Cobalt 产品家族,但不呈现为“网页套壳”。
- 在 `960×640` 到 `1920×1080` 范围内无关键操作丢失或内容遮挡。
- 核心流程可完全由键盘完成。
- 分栏可调整并持久化;主阅读区始终保持可用。
- Light/Dark 的 normal/hover/focus/active/disabled/error 状态完整。
- 中文和中英混排字体、截断和行高正常。
- 异步、离线、锁和 revision 冲突均有明确反馈与恢复路径。
- `cargo test`、`cargo build` 通过,无新增 blocking diagnostics。

## 已定决策

- ✅ 设计源:网页端 Hallmark Cobalt,作为绑定视觉约束。
- ✅ 平台:GPUI 原生桌面应用,不是 WebView/响应式网页。
- ✅ 字体:系统字体优先 + 跨平台/CJK 回退,不打包字体。
- ✅ 交互:键盘、鼠标、触控板优先;排除移动端专属规则。
- ✅ 方法:设计令牌 → Desktop Shell → 交互模型 → 组件状态 → 业务视图 → 桌面验证。

---

## 实施进度(初版)

| 交付项 | 状态 | 说明 |
| --- | --- | --- |
| 1. 平台契约 + tokens/font | ✅ 2026-08-06 | `src/ui/tokens.rs`、`themes/cobalt.json`(Light/Dark 从 web oklch 精确映射),视图已消费 tokens |
| 2. Cobalt Light/Dark + 组件状态 | ✅ 同上 | 主题双模式注册,按钮/输入/树/卡片状态色来自主题槽位 |
| 3. SplitPane + Desktop Shell | ✅ 本次 | `src/ui/split_pane.rs`(拖动、min/max、双击重置、hover affordance);workspace 项目 rail、doc 树 rail、History context panel 三个分栏可调并持久化(`config::Layout`);History 改为右侧 context panel(可关闭、选中高亮);底部状态栏(连接、服务器、锁/编辑、status_msg、版本);`cargo test` 8 通过 |
| 4. 键盘/focus/command 模型 | ⏳ 未开始 | Esc 关闭顺序、树方向键、focus return、⌘P 快速打开、⌘S 保存 |
| 5. Workspace / Document Workspace 细化 | ⏳ 部分已有 | 项目卡片网格、搜索过滤、空状态已有;窄窗降级(≤1280 自动关 History 等)未做 |
| 6. Editor / History / Settings 深化 | ⏳ 部分已有 | 锁 heartbeat、提交冲突恢复已有;Settings 页、revert 确认未做 |
| 7. 完整验证矩阵 + 回归 | ⏳ 未开始 | Phase 7 全部条目;Windows 原生 + DPI 验证未做 |

### 本次变更文件

- `src/ui/split_pane.rs`(新增):水平分栏,左侧固定(`horizontal`)/右侧固定(`horizontal_right`)两种模式。
- `src/ui/tokens.rs`:面板默认宽度与拖动范围(projects 220–360/260、doc rail 240–400/280、history 300–520/360)、`SPLITTER_HIT`、`STATUS_H`。
- `src/config.rs`:`Layout`(serde JSON `layout.json`,损坏回退默认)与 `load_layout/save_layout`。
- `src/app.rs`:三个分栏接入 + 宽度/History 开合持久化;History 改为右侧 context panel(列表在上、详情在下,选中行高亮 `list.active`);底部状态栏;`status_msg` 收敛到状态栏。
- 未实现且留待后续:窄窗自动降级(ponytail: 有 min/max 钳制,无窗口缩放监听)。
