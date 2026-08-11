XWiki 桌面端 UI 设计规则
一、设计流程规则
1. 先理解产品，再开始设计
不能直接套用 Dashboard 模板，必须先明确：


产品是什么；

用户是谁；

用户最频繁完成的任务；

哪些信息需要长期可见；

哪些操作需要键盘完成；

哪些状态会影响用户数据安全；

哪些数据是真实存在的，哪些不能自行编造。
XWiki 是一个：
面向人类和 AI Agent 的 Git-backed 文档管理桌面工作台。
核心任务是：


管理项目；

浏览文档；

编辑 Markdown；

获取编辑锁；

提交 ChangeSet；

查看 revision 和 diff；

处理版本冲突；

查看历史；

管理服务连接、用户和 Token。
设计不能把它做成普通营销页、博客后台或空洞的卡片 Dashboard。
2. 先建立设计系统，再设计页面
必须先确定：


视觉风格；

色彩系统；

字体系统；

间距系统；

圆角系统；

阴影和边框；

图标规范；

组件状态；

Light/Dark 主题；

响应式规则。
不能先写一堆页面，再临时决定颜色和字体。
3. 设计系统必须持久化
设计令牌应当有唯一来源，例如：
design-system/
├── MASTER.md
└── pages/
    ├── workspace.md
    ├── document.md
    ├── editor.md
    └── settings.md

通用规则放在 Master 中，页面特殊规则放到对应页面文件中。
页面规则可以覆盖全局规则，但不能随意破坏全局设计系统。
二、设计系统规则
1. 使用三层 Token 架构
Primitive Tokens
最底层基础值：
color-blue-500
color-gray-100
space-4
radius-6
font-size-15

Semantic Tokens
表达设计语义：
paper
paper-2
ink
ink-muted
rule
accent
danger
success
focus-ring
graphite

Component Tokens
表达组件用途：
button-primary-bg
button-primary-fg
button-hover-bg
input-border
input-focus-ring
tree-selected-bg
panel-divider
project-card-border

视图代码不能直接使用散落的颜色值。
错误：
.bg(rgb(0x2563eb))

正确：
.bg(theme.accent)

或者：
.bg(tokens::button_primary_bg())

2. 禁止组件中硬编码颜色
不能在每个页面里单独写：


#2563eb

#ffffff

rgba(...)

随意的灰色；

随意的蓝色；

临时的红色。
如果发现一个颜色确实需要存在，先把它加入设计令牌，再在组件中使用。
3. Light/Dark 必须分别设计
不能简单地把 Light 模式的背景反转成 Dark 模式。
必须分别定义：


页面背景；

侧栏背景；

卡片背景；

输入框背景；

文字颜色；

次要文字；

边框；

hover；

active；

disabled；

focus；

danger；

success。
Dark 模式需要保持层级，不要所有区域都变成相同的黑色。
三、视觉风格规则
1. XWiki 使用 Cobalt Desktop Workspace
推荐风格：


Modern Minimalism；

Swiss Grid；

Technical Editorial；

Desktop Productivity；

Engineered Paper；

Graphite Register；

Electric Cobalt；

High Information Density。
整体感觉应该是：
冷静、精确、技术化、可靠、适合长时间工作。
不是：


互联网营销网站；

玻璃拟态 SaaS；

炫酷霓虹工具；

移动端 App；

过度圆润的消费级产品。
2. 浅色模式
浅色模式使用：


冷白色背景；

轻微灰度的面板；

深色正文；

灰色辅助文字；

细线分隔；

少量 Cobalt 蓝色作为操作信号；

Graphite 作为代码块、Diff 和终端区域。
不要使用大面积纯蓝背景。
3. 深色模式
深色模式使用：


Graphite 黑色背景；

深灰色侧栏；

轻灰色文本；

更亮的边框；

Cobalt 作为焦点和操作强调色；

代码和 Diff 使用独立的 graphite surface。
不要让所有元素都变成纯黑，也不要让主要文本灰到无法阅读。
4. 避免 AI Slop
必须避免：


默认蓝紫渐变；

大面积玻璃拟态；

到处都是圆角卡片；

每个区块都带阴影；

巨大的 Hero 标题；

三列 Feature Card 模板；

不符合产品的营销统计数字；

虚构的用户数量；

虚构的客户 Logo；

虚构的评价；

过多装饰性图形；

每个页面都使用相同的 Hero → Cards → CTA 结构；

用漂亮但无意义的内容填充页面。
如果没有真实数据：


使用 —；

使用“待确认”；

使用空状态；

或者不展示这个模块。
不要凭空生成：
+47% productivity
Trusted by 50,000 teams
10x faster

四、字体规则
1. 字体角色必须明确
正文
用于：


段落；

表单说明；

文档内容；

错误信息；

Dialog 文案。
推荐：
Windows: Segoe UI / Microsoft YaHei UI
macOS: SF Pro Text / PingFang SC
Linux: Inter / Noto Sans CJK SC

Display
用于：


页面标题；

项目名称；

登录品牌；

工作区标题。
推荐：
Space Grotesk

Mono
用于：


路径；

revision；

commit SHA；

时间；

服务地址；

状态标签；

机器元数据；

快捷键；

Git 信息。
推荐：
JetBrains Mono

2. 字号层级
建议：
页面标题：24–32px
项目标题：20–28px
正文：15px
辅助正文：13–14px
Mono Label：11–12px
代码：13–14px

不要让所有文本都一样大，也不要用过小的文字承载重要信息。
3. 中文排版
必须检查：


中文标题；

中英文混排；

长文件路径；

长项目名称；

长 commit message；

中文段落换行；

中文按钮宽度；

中文字体回退。
中文不能因为强行使用英文字体而变成空白、方框或错位。
五、布局规则
1. 使用 Desktop Workspace Shell
XWiki 不应该使用简单的页面跳转结构，而要使用持续存在的桌面 Shell：
Topbar
├── Primary Rail
├── Project Panel
├── Main Content
├── Context Panel
└── Status Area

Topbar
包含：


XWiki；

当前项目；

当前文档；

命令面板；

快速打开；

主题；

账户；

设置；

退出。
Primary Rail
包含：


Workspace；

最近项目；

项目入口；

设置；

其他高层级功能。
Project Panel
根据当前上下文显示：


项目列表；

当前项目的文档树；

文件夹结构；

搜索；

右键菜单。
Main Content
显示：


项目网格；

Markdown 文档；

编辑器；

Diff；

设置内容。
Context Panel
按需显示：


History；

Commit Detail；

Diff；

Conflict Recovery；

Version Preview。
Status Area
显示：


服务连接状态；

锁状态；

当前 revision；

保存状态；

网络状态；

当前操作。
2. 桌面窗口规则
建议：
默认窗口：1280 × 800
最小窗口：960 × 640

面板建议：
Workspace Panel：220–360px
Document Rail：240–400px
History Panel：300–520px
正文宽度：720–820px

窗口不足时：


先折叠 History；

再折叠次级信息；

再压缩 Primary Rail；

最后才处理文档树。
主内容、保存按钮、关闭按钮和冲突操作不能被挤压到不可用。
3. 分栏规则
分栏必须支持：


拖动；

最小宽度；

最大宽度；

折叠；

双击恢复默认宽度；

宽度持久化；

窄窗口降级；

独立滚动。
分隔线应当是 hairline，但鼠标命中区域需要足够大。
4. 正文阅读宽度
文档正文不能无限拉伸。
推荐：
阅读正文：720–820px
代码区：允许横向滚动
长路径：截断并提供 tooltip

这样可以让技术文档保持良好的阅读节奏。
六、间距和密度规则
1. 使用 4px 间距网格
推荐：
4px
8px
12px
16px
20px
24px
32px
40px

页面中不要大量出现：
13px
17px
19px
27px
33px

除非有明确理由。
2. 桌面端采用 Compact/Cozy 两档密度
推荐：
Compact 行高：28–30px
Cozy 行高：34–36px

不要把所有桌面控件都做成移动端的大触控目标。
不过，重要的图标按钮仍然需要有足够的点击区域。
3. 卡片不能被 Flex 拉坏
Project Card 必须：


有明确的最小宽度；

有合理的最大宽度；

卡片高度统一；

内容区域可收缩；

footer 固定在卡片内部；

不因 flex-wrap 被拉伸；

不出现 footer 脱离卡片；

不出现大量空白；

长描述不能破坏整体布局。
七、图标规则
1. 禁止 Emoji 作为结构图标
禁止：
⚙️
🚪
📁
✨
✅
❌

必须使用：


SVG；

统一 icon set；

平台矢量图标；

gpui-component 内置图标；

项目自己的嵌入式 SVG 资源。
2. 图标必须来自同一套视觉语言
必须统一：


线性或填充风格；

stroke width；

圆角风格；

视觉重量；

图标尺寸；

与文字的间距。
不能一个按钮用粗线图标，另一个按钮用细线图标。
3. 常用 SVG 图标
新建：Plus / Folder
设置：Settings
主题：Sun / Moon
返回：ArrowLeft
打开：ArrowRight
重试：Redo / Refresh
保存：Check / Save
取消：Close / X
删除：Trash / Delete
编辑：Edit
历史：History
登录：ArrowRight
退出：LogOut / ArrowLeft
连接测试：Network / Plug
文件：File
文件夹：Folder / FolderOpen

4. 图标尺寸和对齐
图标尺寸应该通过 token 统一：
icon-xs
icon-sm
icon-md
icon-lg

常规按钮中的图标应当：


与文字基线对齐；

使用一致的 gap；

继承主题颜色；

不造成按钮尺寸跳动；

不因为 hover/pressed 改变布局边界；

icon-only 按钮必须提供 tooltip。
5. 资源必须正确注册
使用 SVG 图标时，不能只写：
Button::new("settings")
    .icon(IconName::Settings)

还必须确保：


SVG 文件真实存在；

应用注册了 AssetSource；

Light/Dark 都能加载；

运行时路径正确；

打包后资源仍然存在；

图标颜色可见；

图标尺寸不是 0；

不被父元素裁剪。
八、组件规则
所有组件至少要设计这些状态：
normal
hover
focus
active
disabled
loading
success
error
empty
conflict
offline

Button
必须支持：


primary；

secondary；

ghost；

destructive；

icon + label；

icon-only；

loading；

disabled；

tooltip；

shortcut hint。
按钮不能只通过颜色表达危险状态，也不能因为点击状态改变布局尺寸。
Input
必须包含：


永久 label；

placeholder；

focus ring；

错误文字；

disabled 状态；

loading 状态；

合理的输入宽度；

长文本处理。
错误不能只依赖红色边框，必须有文字说明。
Dialog
必须支持：


初始焦点；

Esc 关闭；

默认按钮；

危险按钮；

取消和确认顺序；

提交中的 disabled；

错误反馈；

关闭后焦点返回原按钮。
Empty State
空状态必须说明：


当前为什么为空；

用户下一步可以做什么；

一个明确的 CTA；

空项目和搜索无结果不能使用同一文案。
Loading State
加载状态应该：


使用局部 skeleton；

保留页面结构；

不能整个窗口变成空白；

不能让用户误以为应用卡死；

异步按钮需要显示 loading；

加载期间禁止重复操作。
Error State
错误状态必须包含：


发生了什么；

是否影响当前数据；

用户可以怎么处理；

Retry；

返回；

保留草稿；

查看远端等恢复路径。
不能只显示：
Error

或一段红色文字。
九、交互规则
1. 键盘优先
核心任务不能依赖鼠标：
Ctrl/Cmd+K：命令面板
Ctrl/Cmd+P：快速打开
Ctrl/Cmd+S：保存
Ctrl/Cmd+Shift+T：切换主题
Esc：分层关闭
Tab / Shift+Tab：移动焦点
↑ / ↓：文档树或 History 导航
← / →：展开和折叠
Enter：打开

2. Esc 必须分层工作
Esc 关闭顺序：


Dialog；

Command Palette；

History/Context Panel；

临时提示；

编辑状态。
不能让 Esc 直接退出整个页面或丢失编辑内容。
3. 鼠标规则


图标按钮必须有 tooltip；

项目支持右键菜单；

文档树支持右键菜单；

分隔线支持拖动；

可拖动区域必须有 hover affordance；

单击用于选择或打开；

不能把双击作为唯一入口；

危险操作必须二次确认；

滚动区域边界清晰。
4. 异步操作规则
异步操作期间：


禁止重复提交；

显示 loading；

保留当前内容；

显示成功或失败结果；

网络错误必须可重试；

写操作不能盲目自动重试；

只有带幂等键的操作才允许安全重试。
十、无障碍和可用性规则
必须检查：


键盘可操作；

焦点可见；

焦点顺序符合视觉顺序；

Dialog 焦点不会丢失；

图标按钮有可理解的 label；

重要信息不能只靠颜色表达；

Light/Dark 对比度足够；

disabled 状态不仅是降低颜色；

错误有文字说明；

长文本可读；

长路径不会撑坏布局；

图标与背景有足够对比度；

窄窗口中操作仍然可访问；

高 DPI 下文字和 hairline 清晰。
十一、Hallmark 反 AI 套模板规则
Hallmark 最重要的规则是：
1. 不要毁掉已有项目结构
在已有项目中：


不删除生产文件；

不删除路由；

不删除组件目录；

不重写整个项目；

不随意改变业务逻辑；

不随意改变 API；

不复制 README 文字作为页面内容；

修改前必须知道要改哪些文件。
2. 不要编造内容
没有真实指标时，不要写：
10x faster
50,000+ teams
47% productivity

没有真实数据时使用：
—
待确认
暂无数据

3. 不要中途临时改变 Token
主题确定后：


所有颜色使用 Token；

所有字体使用 Token；

所有组件使用语义化变量；

不要在渲染过程中随意加入新的颜色；

新需求必须先补充 Token。
4. 不要伪造浏览器或 IDE Chrome
不要手动画：


假浏览器地址栏；

假手机外框；

假浏览器窗口；

假 IDE 标题栏；

假终端窗口装饰。
桌面端应使用真实的窗口结构和真正的应用 Shell。
5. 必须进行自我审查
交付前至少检查六个维度：
Philosophy：是否符合产品定位
Hierarchy：层级是否清晰
Execution：实现是否精致
Specificity：是否真正属于 XWiki
Restraint：是否克制而不堆装饰
Variety：不同页面是否有合理结构变化

每一项低于 3 分都应该重新修改。
十二、最终验收标准
一个合格的 XWiki 桌面 UI 必须满足：


不是只有首页；

不是只有几个卡片；

不是营销落地页；

有完整 Desktop Workspace Shell；

有项目、文档、编辑器、历史和设置；

有 loading、empty、error、conflict 状态；

所有主要按钮都有合适 SVG；

SVG 资源运行时真实可见；

Light/Dark 都可用；

所有颜色和字体来自 Token；

没有 Emoji 图标；

没有虚构指标；

没有大面积渐变和玻璃拟态；

正文宽度适合长时间阅读；

项目卡片不会被 Flex 拉坏；

分栏可以拖动和持久化；

窄窗口不会遮挡核心操作；

键盘可以完成主要任务；

Dialog 关闭后焦点能返回；

网络错误和 revision conflict 有恢复路径；

高 DPI 下图标和 hairline 清晰；

代码通过构建、测试和 UI 启动验证。
一句话总结：
XWiki 的桌面 UI 应该是一个克制、精确、技术化、键盘优先、状态完整的 Cobalt 文档工作台，而不是一个堆满卡片、渐变和装饰的通用 SaaS Dashboard。