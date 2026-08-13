# XWiki 桌面端与 CLI 规划

## 总体方案

- 保留 `main` 分支的 Go 服务端、Git 仓库模型和 `/api/v1`，Rust 客户端只通过 HTTP API 工作，不直接访问服务端 SQLite/Git。
- 构建单一程序 `xwiki`：
  - 双击或无参数运行：启动 GPUI 桌面端。
  - 带子命令运行：执行 CLI，例如 `xwiki project list`。
  - `xwiki gui` 可显式启动桌面端。
- 产品统一命名为 XWiki；现有 Go 服务端继续使用 `xwiki`，避免二进制冲突。
- Windows 为首发平台，Linux 用于日常开发和功能验证；首版提供 Windows MSI 与便携 ZIP。
- 首版实现网页功能全量对齐，但编辑器采用“Markdown 源码 + 实时预览”，不复刻 Tiptap 所见即所得。
- `app` 与 `main` 没有共同 Git 历史。实施时以 `main` 新建集成分支，把当前 Rust 原型迁入 `client/`，不直接合并两个无关历史。

## 架构与公共接口

- Rust 客户端分为四层：
  - `api`：认证、DTO、分页、上传下载、错误映射和能力探测。
  - `domain`：项目、文档、ChangeSet、锁、历史、附件等客户端用例。
  - `desktop`：GPUI 状态、窗口、路由和组件。
  - `cli`：参数解析、表格/JSON 输出和退出码。
- 使用 `gpui-component` 的 Theme、Editor、Markdown、Dialog、Sidebar、Table、Notification、Dock 等组件；普通界面全部原生 GPUI。
- Mermaid、KaTeX、复杂 HTML 预览和 Scalar API Reference 使用受限 WebView 渲染；导航、编辑和权限操作仍由原生 GPUI 控制。
- GPUI 与 `gpui-component` 的漂移控制依赖提交的 `Cargo.lock` + `--locked` CI：`gpui-component` 的 manifest 对 zed 依赖未固定 rev，固定 zed 会分裂出第二个 gpui 源导致类型冲突（已实证），故 zed 系依赖保持锁文件固定、禁止 `cargo update` 盲升；升级时同步升级 `gpui-component` 与 zed 并重新验证。[GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)
- 单一客户端配置保存服务地址、语言、主题和窗口布局；凭据不写入配置文件：
  - Windows 使用 Credential Manager。
  - Linux 开发环境使用 Secret Service。
  - CLI 无人值守时允许 `XWIKI_TOKEN` 临时覆盖 session。
- HTTP 写操作不自动盲重试；只有携带幂等键的操作才允许安全重试。统一处理 401、403、404、409 revision conflict、410 archived、锁丢失和网络中断。
- 后端进行向后兼容补强：
  - 增加 `GET /api/v1/meta`，返回服务版本、API 版本、上传限制和 capability 列表。
  - 补全 OpenAPI 中用户、锁、分享、反向链接、附件、导入和审计端点及请求/响应 schema。
  - 保持现有网页和 `/api/v1` 路径兼容，不引入 v2 或重写领域逻辑。

## 桌面端功能

- 登录与应用外壳：
  - 配置一个服务地址，账号密码登录、退出、修改密码和自动恢复 session。
  - 服务健康状态、版本兼容提示、全局命令面板、通知中心。
  - System/Light/Dark 三种主题；跟随系统并允许手动覆盖，选择持久化。
- 项目工作台：
  - 项目列表、搜索、活动/归档筛选、创建、归档、恢复。
  - 从本地文件夹、Git Bundle、远程仓库导入；导出 ZIP 或 Bundle。
  - 项目概览展示 README、仓库信息和最近提交。
- 文档工作区：
  - 文档树、面包屑、 `_sidebar.md` 菜单、目录 TOC、版本列表和多标签页。
  - Markdown、源码、图片及附件查看；全文搜索、反向链接和 `Ctrl+K` 快速跳转。
  - 分享页面并复制完整 URL。
  - 新建文件/目录、重命名、移动、删除、批量导入和附件上传下载。
- 编辑与提交：
  - GPUI Editor 源码编辑，支持语法高亮、查找替换、撤销重做和实时预览。
  - 编辑前获取页面锁，每 30 秒续租；关闭、切页或退出时安全释放。
  - 草稿本地恢复、未提交离开确认、提交消息对话框和 `Ctrl+S`。
  - 保存前获取当前 revision，通过 ChangeSet 提交；冲突时保留草稿并提供“查看远端、重新载入、复制草稿”，不自动覆盖或合并。
- 历史与管理：
  - 提交列表、提交详情、numstat/patch Diff、文件历史、历史版本预览、恢复版本和 Revert。
  - Token 创建、列表和撤销；明文 Token 仅创建后显示一次。
  - 管理员用户创建、启用、禁用；非管理员隐藏无权入口。
  - 审计日志与 OpenAPI Reference 页面。
- Windows 桌面行为：
  - 原生窗口、文件/文件夹选择器、剪贴板、外部链接打开和系统密钥库。
  - MSI 与便携 ZIP；首版不包含自动更新，发布包提供 SHA-256 校验值。

## CLI 功能

- 全局约定：
  - `--server` 临时覆盖保存的服务地址，`--json` 输出稳定 JSON。
  - 正常数据写 stdout，进度和错误写 stderr。
  - 退出码固定为：`0` 成功、`2` 参数错误、`3` 认证/权限、`4` 不存在、`5` revision/锁冲突、`6` 网络或服务端错误。
- 命令组：
  - `login`、`logout`、`whoami`、`config show|set-server`、`server status|info`
  - `project list|show|create|archive|restore|import-folder|import-repo|import-bundle|export`
  - `doc tree|get|create|update|delete|move|edit|import`
  - `search`、`backlinks`、`share`
  - `attachment list|upload|download|delete`
  - `history list|show|diff|file|revert|restore`
  - `lock status|acquire|release|force-release`
  - `token list|create|revoke`
  - `user list|create|enable|disable`
  - `audit list`、`openapi export`
- `doc edit` 获取锁后把内容交给 `$EDITOR`，关闭编辑器后展示 diff、确认 commit message 并提交；异常退出尽力释放锁并保留临时草稿。
- `doc create/update/import` 支持 `--file` 与 stdin；写操作支持 `--base-revision`、`--message`、`--dry-run`、`--idempotency-key`，适用于脚本和 Agent 自动化。
- 原有服务端 `xwiki serve/admin/reindex` 保持不变，不重复放进客户端程序。

## 测试与验收

- API 客户端单元测试覆盖 URL、cookie/PAT、multipart、流式下载、错误信封、超时和敏感信息脱敏。
- 使用 Go 测试服务做跨语言集成测试，覆盖登录、所有资源 CRUD、锁续租、ChangeSet 原子性、冲突、幂等、导入导出和权限矩阵。
- CLI 使用快照测试验证帮助文本、表格、JSON schema、stdout/stderr 和退出码。
- 桌面逻辑以可测试 ViewModel 为核心，覆盖路由、加载/空/错误状态、主题持久化、草稿恢复和冲突流程。
- Windows CI 执行 Rust 测试、Clippy、构建、安装包与桌面启动冒烟测试；Linux CI 执行 Rust/Go 全套测试和 X11/Wayland 开发构建。
- 最终验收逐项对照 `main` 网页：项目、阅读、搜索、编辑锁、提交、历史、Diff、恢复、附件、分享、反链、导入导出、Token、用户、审计和 API 文档均能从桌面完成，并存在对应 CLI 命令。

## 明确假设

- 采用未回复问题的推荐默认：程序名为 `xwiki`，并允许最小、向后兼容的后端补强。
- 只维护一个当前服务和当前账号，不做多 profile。
- 不支持离线同步、本地内嵌服务、自动合并和 Tiptap 式所见即所得。
- “网页全量对齐”指能力和数据语义对齐，不要求像素级复制网页布局。
