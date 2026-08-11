# XWiki Web Demo → Desktop GPUI 功能审计

> 参考来源：`login.html`、`workspace.html`、`editor-stitch-original.html`、`history-stitch-original.html`。
>
> 审计方法：按照 `baoyu-design` 的 `import-from-html` 规则读取 HTML/CSS 的真实控件、状态和交互，不把 demo 的静态按钮误判成已接通的业务能力；再对照 `src/app`、`src/api` 的实际实现。

## 结论

当前桌面端已经覆盖了网页 demo 的核心工作流：登录 → 项目列表 → 文档树 → 阅读/编辑 → 锁与冲突恢复 → 版本历史。

本轮已补齐三项明显缺口：

- Workspace：`全部 / 活跃 / 已归档` 项目筛选。
- Editor：`编辑 / 预览` 模式切换，预览使用当前编辑缓冲区实时渲染 Markdown。
- History：版本搜索、作者头像/版本标记、Diff 汇总。
- Login：登录请求中的禁用态和连接中反馈。

## 功能对照

| 网页 demo 能力 | 桌面端现状 | 结论 |
| --- | --- | --- |
| 服务地址、用户名、密码登录 | `src/app/views/login.rs` + `Client::login` | 已覆盖 |
| 登录错误提示 | `login_error` | 已覆盖 |
| 登录加载态 | `loading`，本轮补充按钮禁用和状态文字 | 已覆盖 |
| Forgot / setup guide | demo 有链接，客户端没有对应 API/页面 | 未覆盖，需产品确认后补 |
| Remember me / 密码可见切换 | demo 优化稿提出，但原始业务 API 没有对应字段 | 未覆盖，暂不伪造 |
| 项目搜索 | `filter_input`，按名称/描述过滤 | 已覆盖 |
| All / Active / Archived | `ProjectFilter` + 三个筛选控件 | 已覆盖 |
| New Project | GPUI Dialog + `Client::create_project` | 已覆盖 |
| 项目状态 | active / archived 状态点和标签 | 已覆盖 |
| Loading skeleton | Workspace skeleton | 已覆盖 |
| 加载错误 / Retry | `projects_error` + retry | 已覆盖 |
| 项目卡片打开 | click + quick open + context menu | 已覆盖 |
| 项目卡片更多菜单 | demo 有 `more_vert`，当前只提供“打开项目” | 部分覆盖 |
| Archive / Unarchive 项目 | API 已有 `set_archived`，桌面卡片菜单尚未接入 | **缺口 P1** |
| 分页 / 无限滚动 | demo 未提供真实分页协议 | 不应凭空添加 |
| 文件树 | 文档树、目录进入、右键菜单、重试 | 已覆盖 |
| 面包屑 | docs → 目录层级，可点击返回 | 已覆盖 |
| Markdown 编辑 | 锁、心跳、提交消息、ChangeSet 保存 | 已覆盖 |
| Edit / Preview tabs | 本轮添加，Preview 渲染当前编辑缓冲区 | 已覆盖 |
| Review Changes / History | History context panel + commit detail | 已覆盖 |
| 冲突恢复 | reload / force retry / abandon | 已覆盖，且比 demo 更完整 |
| 锁状态 | status bar + editor header | 已覆盖 |
| 编辑器标题输入 | demo 有标题输入；客户端当前由文档路径生成标题 | 部分覆盖 |
| 自动保存 | demo 文案有 save 状态，但 API 是显式 ChangeSet 提交 | 未覆盖，当前设计为显式保存 |
| 版本搜索 | 本轮添加 `history_input`，按消息/作者/SHA 过滤 | 已覆盖 |
| 版本选择 | commit timeline + detail | 已覆盖 |
| 文件变更状态 | A/M/D 颜色标记 | 已覆盖 |
| numstat | `DiffStat` + additions/deletions | 已覆盖 |
| Compare | demo 有按钮；当前选择版本可查看 detail/stats，但没有独立 compare 状态 | **缺口 P1** |
| Restore | demo 有按钮；当前 API 没有历史版本恢复接口 | **缺口 P0（需 API）** |
| Git diff 逐行内容 | 当前 API 只有 `DiffStat`，没有 patch/diff lines | **缺口 P0（需 API）** |
| 右侧用户/通知菜单 | 客户端有设置、退出、通知 toast，但没有完整用户下拉/通知中心 | 部分覆盖 |
| 暗色模式 | Cobalt Light/Dark + 快捷键持久化 | 已覆盖 |
| 快速打开 Cmd/Ctrl+P | quick-open overlay | 已覆盖 |
| 命令面板 Cmd/Ctrl+K | command palette | 已覆盖 |
| 可调整分栏 | project/doc/history split pane，宽度持久化 | 已覆盖 |
| Settings / server test | GPUI settings view + connection test | 已覆盖 |

## 需要注意的 Demo 差异

1. `history-stitch-original.html` 的 Compare、Restore、搜索和菜单是静态 HTML 控件；不能只看按钮存在就认为业务已完成。
2. 网页 demo 用整页左右分栏展示文档与历史；桌面端使用可调整的 History context panel，这是符合桌面工作台设计原则的适配，不是功能缺失。
3. 网页 demo 的逐行 Diff 需要后端返回 patch；当前 `src/api` 只定义了 `DiffStat`，客户端无法可靠生成历史内容差异。
4. 网页 demo 的 Restore 需要服务端提供“以某个 commit 恢复文件/创建 ChangeSet”的能力；不能用当前文档内容直接伪造恢复。

## 优先级建议

### P0：补齐后端契约

- 新增 `GET /projects/:id/commits/:sha/diff`，返回文件级 patch/逐行 diff。
- 新增基于 commit 的 restore API，服务端生成新的 ChangeSet，而不是客户端直接改 revision。
- DTO 增加 patch 行、旧/新行号、文件状态和二进制文件标记。

### P1：客户端业务补齐

- 项目卡片右键菜单接入 Archive / Unarchive。
- History 增加“对比当前版本”明确状态，并在 Diff 面板中显示左右版本信息。
- 编辑器支持文档路径重命名，复用现有 `Change.new_path`。
- 将用户菜单、通知中心和设置入口收敛到统一账户菜单。

### P2：体验增强

- Forgot password / setup guide 要先确定服务端路由，再加入客户端页面。
- 密码显示切换、Remember me 只有在认证协议支持后加入。
- 编辑器增加复制代码、撤销/重做快捷键提示，以及 reduced-motion 处理。

## 本轮修改文件

- `src/app/mod.rs`
- `src/app/views/login.rs`
- `src/app/views/workspace.rs`
- `src/app/views/editor.rs`
- `src/app/views/history.rs`
