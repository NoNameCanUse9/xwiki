# XWiki：面向 AI Agent 的轻量 Git 文档管理系统开发计划

## 1. 你的角色

你是一名资深 Go 全栈架构师，需要从零设计并实现一个轻量、可自托管、以 Git 为唯一内容事实源的文档管理系统。

该系统类似精简版 ShowDoc，但必须针对 AI Agent 使用场景重新设计：

- 每个项目对应一个独立 Git 仓库。
- 每次网页编辑、API 修改、批量同步都必须产生真实 Git Commit。
- API 写入必须支持乐观并发控制、幂等请求、批量原子提交和机器可读 Diff。
- UI 使用 React、TypeScript、Vite、Tailwind CSS 和 shadcn/ui。
- 后端使用 Go。
- 元数据和权限存入 SQLite。
- Markdown 文件、目录、历史版本全部由 Git 管理。
- 系统最终应支持单机部署，并尽量打包为一个 Go 服务。

项目暂定名：

```text
XWiki
```

---

# 2. 产品定位

XWiki 是一个面向人类和 AI Agent 的轻量 Git 文档服务。

它不是传统 Wiki，也不是完整代码托管平台，而是：

```text
Markdown 文档管理
+ 独立 Git 项目
+ Agent REST API
+ Commit 历史
+ Diff 和回滚
+ 项目级权限
+ 极简 Web UI
```

核心差异：

1. 所有内容变更都会创建真实 Git Commit。
2. API 修改和网页修改使用同一套提交链路。
3. 一个项目对应一个独立 Git 仓库。
4. Agent 可以安全地批量修改文档，而不会静默覆盖其他修改。
5. Git 是文档内容和历史版本的唯一事实源。

---

# 3. 技术栈

## 3.1 后端

使用：

```text
Go 当前稳定版本
Chi Router
SQLite
Git CLI
OpenAPI 3.1
```

推荐依赖：

```text
github.com/go-chi/chi/v5
github.com/go-chi/cors
github.com/golang-jwt/jwt/v5
github.com/oklog/ulid/v2
github.com/pressly/goose/v3
modernc.org/sqlite
golang.org/x/crypto
```

要求：

- 优先使用标准库。
- SQLite 驱动优先采用纯 Go 实现，避免强制依赖 CGO。
- Git 操作第一版通过系统 `git` 命令完成。
- 所有 Git 命令必须经过统一封装，禁止在业务层直接调用 `exec.Command`。
- 后端必须有清晰的领域层、服务层、存储层和 HTTP 层。

## 3.2 前端

使用：

```text
React
TypeScript
Vite
Tailwind CSS
shadcn/ui
React Router
TanStack Query
Zustand
React Hook Form
Zod
```

Markdown 相关建议：

```text
react-markdown
remark-gfm
rehype-highlight
Monaco Editor 或 CodeMirror 6
```

图标：

```text
lucide-react
```

## 3.3 部署

需要支持：

```text
Docker
Docker Compose
Linux 单机部署
Windows / WSL2 开发
```

前端构建产物应支持嵌入 Go 二进制中，通过 `embed.FS` 提供静态资源。

最终部署形态优先为：

```text
xwiki
data/
├── xwiki.db
├── repositories/
├── worktrees/
└── assets/
```

---

# 4. 核心领域模型

系统层级为：

```text
System
└── Project
    ├── Git Repository
    ├── Directory
    ├── Markdown Page
    ├── Assets
    ├── Commit History
    ├── Members
    ├── Agent Tokens
    └── Search Index
```

## 4.1 项目

一个 Project 必须对应一个独立 Git 仓库。

示例：

```text
项目名称：T3 API
项目 ID：prj_01K...
项目 Slug：t3-api
Git 仓库：data/repositories/prj_01K....git
默认分支：main
```

仓库目录必须使用稳定的项目 ID，而不能使用项目名称或 Slug。

项目重命名后不允许移动仓库目录。

## 4.2 页面

页面直接对应 Markdown 文件：

```text
Project ID + 文件路径 = 页面定位
```

例如：

```text
prj_01KABC + docs/order/create.md
```

MVP 阶段不建立 `pages` 数据库表。

## 4.3 文件夹

文件夹直接对应 Git Tree 或文件系统目录。

不得创建以下数据库表：

```text
folders
pages
page_versions
file_contents
```

Git 不支持空目录，因此 MVP 不需要支持空文件夹。

创建新目录时，应同时创建：

```text
index.md
```

或者在目录中创建第一个文档。

## 4.4 版本

版本直接对应 Git Commit。

```text
一次保存 = 一个 Git Commit
一次批量修改 = 一个 Git Commit
一次回滚 = 一个新的 Revert Commit
```

禁止通过硬重置删除历史。

---

# 5. Git 仓库规范

默认仓库结构：

```text
README.md

docs/
├── introduction.md
├── authentication.md
├── order/
│   ├── index.md
│   ├── create.md
│   └── list.md
└── deployment/
    └── docker.md

openapi/
├── order.yaml
└── user.yaml

assets/
└── images/

.xwiki.yaml
```

## 5.1 文件职责

```text
README.md
```

作为项目首页。

```text
docs/
```

保存 Markdown 文档。

```text
openapi/
```

保存 OpenAPI JSON 或 YAML 文件。

```text
assets/
```

保存图片和附件。

```text
.xwiki.yaml
```

保存需要进入 Git 版本管理的项目配置。

示例：

```yaml
version: 1

project:
  homepage: README.md

navigation:
  root: docs
  sort: manual

markdown:
  gfm: true
  mermaid: true
  math: false

openapi:
  directories:
    - openapi
```

以下内容禁止写入 Git：

- 用户信息
- 成员权限
- Token
- Token Hash
- 仓库物理路径
- 登录 Session
- 最近访问记录
- 私有系统配置

---

# 6. Git 提交模型

## 6.1 Commit Message

单文件修改：

```text
docs(order/create.md): 更新订单创建接口
```

多文件修改：

```text
docs(order): 同步订单模块接口文档
```

完整 Commit：

```text
docs(order): 同步订单模块接口文档

Actor-Type: agent
Actor-ID: usr_01KABC
Actor-Name: codex
Source: rest-api
Request-ID: req_01KDEF
Token-ID: pat_01KGHI
Base-Revision: 8e45be4df910
```

## 6.2 Commit Trailer

至少支持：

```text
Actor-Type
Actor-ID
Actor-Name
Source
Request-ID
Token-ID
Base-Revision
```

`Actor-Type` 可选：

```text
human
agent
system
```

`Source` 可选：

```text
web
rest-api
cli
import
system
```

## 6.3 文件变更解析

Commit Message 中的路径只能用于展示，不能作为真实文件变更来源。

真实文件变更必须从 Git Diff 解析：

```bash
git diff-tree \
  --root \
  --no-commit-id \
  --name-status \
  -r \
  -M \
  <commit_sha>
```

必须正确识别：

```text
A 新增
M 修改
D 删除
R 重命名
C 复制
```

页面历史使用：

```bash
git log --follow -- <path>
```

指定版本文件内容使用：

```bash
git show <commit>:<path>
```

---

# 7. 项目级原子提交

所有网页写入和 API 写入必须统一转换成项目级 ChangeSet。

核心 API：

```http
POST /api/v1/projects/{project_id}/changes
```

请求：

```json
{
  "base_revision": "8e45be4df910",
  "message": "docs(order): 同步订单模块接口",
  "dry_run": false,
  "operations": [
    {
      "action": "update",
      "path": "docs/order/create.md",
      "content": "# 创建订单\n"
    },
    {
      "action": "create",
      "path": "docs/order/errors.md",
      "content": "# 错误码\n"
    },
    {
      "action": "delete",
      "path": "docs/order/legacy.md"
    },
    {
      "action": "move",
      "path": "docs/order/list.md",
      "target_path": "docs/order/query.md"
    }
  ]
}
```

支持的操作：

```text
create
update
delete
move
```

响应：

```json
{
  "project_id": "prj_01KABC",
  "commit_id": "b81ad7d8c2e4",
  "parent_commit_id": "8e45be4df910",
  "message": "docs(order): 同步订单模块接口",
  "changes": [
    {
      "status": "modified",
      "old_path": "docs/order/create.md",
      "new_path": "docs/order/create.md"
    },
    {
      "status": "added",
      "new_path": "docs/order/errors.md"
    },
    {
      "status": "deleted",
      "old_path": "docs/order/legacy.md"
    },
    {
      "status": "renamed",
      "old_path": "docs/order/list.md",
      "new_path": "docs/order/query.md"
    }
  ]
}
```

一个请求内的所有操作必须生成同一个 Commit。

任何一个操作失败，整个 ChangeSet 都必须失败，不允许产生部分修改。

---

# 8. Git 写入事务

每个项目拥有独立写锁。

不同项目可以并发写入，同一项目的写入必须串行化。

提交过程：

```text
验证身份
↓
验证项目权限
↓
获取项目级写锁
↓
读取当前 HEAD
↓
验证 base_revision
↓
创建临时 Worktree
↓
应用所有操作
↓
验证文件路径和文件类型
↓
执行 Markdown/OpenAPI 校验
↓
生成 Diff
↓
创建 Commit
↓
使用 update-ref 原子更新 main
↓
更新搜索索引
↓
记录幂等结果
↓
释放锁
```

原子更新分支时使用类似：

```bash
git update-ref refs/heads/main <new_commit> <old_commit>
```

即使已有项目锁，也必须通过旧 Commit 参数执行 Compare-And-Swap，防止意外覆盖。

---

# 9. 乐观并发控制

每次写入必须携带：

```json
{
  "base_revision": "8e45be4df910"
}
```

如果当前项目 HEAD 已变化，返回：

```http
409 Conflict
```

响应：

```json
{
  "error": {
    "code": "revision_conflict",
    "message": "Project revision has changed.",
    "base_revision": "8e45be4df910",
    "current_revision": "a83d211cd930",
    "changed_paths_since_base": [
      "docs/order/create.md"
    ]
  }
}
```

禁止静默覆盖。

禁止默认执行自动合并。

后续版本可以提供合并建议，但 MVP 只返回冲突信息。

---

# 10. 幂等请求

Agent 写入接口必须支持：

```http
Idempotency-Key: agent-task-29381-step-4
```

幂等范围：

```text
Token ID + Project ID + Idempotency-Key
```

同一范围内重复请求：

- 不得创建重复 Commit。
- 请求体完全一致时，返回第一次请求的结果。
- 请求体不一致时，返回 `409 idempotency_conflict`。

幂等记录建议保存：

```text
request_hash
response_status
response_body
commit_id
expires_at
```

---

# 11. Dry Run

`changes` 接口支持：

```json
{
  "dry_run": true
}
```

Dry Run 不创建 Commit，不修改分支。

响应：

```json
{
  "valid": true,
  "base_revision": "8e45be4df910",
  "diff": {
    "files": [
      {
        "path": "docs/order/create.md",
        "status": "modified",
        "additions": 12,
        "deletions": 4,
        "patch": "..."
      }
    ]
  },
  "warnings": [
    {
      "code": "broken_markdown_link",
      "path": "docs/order/create.md",
      "target": "./payment.md"
    }
  ]
}
```

网页编辑器的“预览改动”也必须调用同一套 Dry Run 服务。

---

# 12. 数据库设计

使用 SQLite，并开启：

```text
WAL
foreign_keys
busy_timeout
```

基础表：

```text
users
sessions
projects
project_members
personal_access_tokens
token_project_permissions
idempotency_records
audit_logs
search_documents
schema_migrations
```

## 12.1 projects

字段至少包括：

```text
id
slug
name
description
repository_path
default_branch
visibility
status
created_by
created_at
updated_at
```

`status`：

```text
active
archived
deleted
```

## 12.2 project_members

字段：

```text
project_id
user_id
role
created_at
```

角色：

```text
owner
editor
viewer
```

## 12.3 personal_access_tokens

字段：

```text
id
user_id
name
token_prefix
token_hash
expires_at
last_used_at
created_at
revoked_at
```

数据库只能保存 Token Hash，不能保存完整 Token。

## 12.4 token_project_permissions

字段：

```text
token_id
project_id
scopes
allow_paths
deny_paths
```

`scopes` 使用 JSON 数组或标准关联表。

MVP 可以使用 JSON 字段，但必须封装解析逻辑。

---

# 13. 权限系统

## 13.1 用户角色

```text
owner
editor
viewer
```

权限：

### owner

- 读取项目
- 修改文档
- 创建 Commit
- Revert Commit
- 管理成员
- 管理 Token
- 修改项目设置
- 归档和删除项目

### editor

- 读取项目
- 修改文档
- 创建 Commit
- 查看历史和 Diff

### viewer

- 读取项目
- 查看历史
- 查看 Diff
- 搜索

## 13.2 Agent Token Scope

支持：

```text
project:read
project:admin
file:read
file:write
commit:read
commit:create
commit:revert
asset:read
asset:write
token:manage
```

Token 可以限制路径：

```json
{
  "allow_paths": [
    "docs/api/**",
    "openapi/**"
  ],
  "deny_paths": [
    ".xwiki.yaml",
    "docs/internal/**"
  ]
}
```

规则：

1. `deny_paths` 优先级高于 `allow_paths`。
2. 所有路径必须转为标准仓库相对路径。
3. 禁止 `..`、绝对路径、空字节和非法分隔符。
4. 默认禁止访问 `.git`。
5. 默认禁止通过符号链接逃逸仓库目录。

---

# 14. REST API

所有接口统一前缀：

```text
/api/v1
```

## 14.1 身份认证

```http
POST   /auth/login
POST   /auth/logout
GET    /auth/me
POST   /auth/password
```

网页使用 HttpOnly Session Cookie。

Agent 使用：

```http
Authorization: Bearer <personal_access_token>
```

## 14.2 项目

```http
GET    /projects
POST   /projects
GET    /projects/{project_id}
PATCH  /projects/{project_id}
DELETE /projects/{project_id}

POST   /projects/{project_id}/archive
POST   /projects/{project_id}/restore
```

## 14.3 文档树

```http
GET /projects/{project_id}/tree
```

参数：

```text
ref
path
depth
```

响应：

```json
{
  "revision": "b81ad7d8c2e4",
  "entries": [
    {
      "name": "order",
      "path": "docs/order",
      "type": "directory"
    },
    {
      "name": "create.md",
      "path": "docs/order/create.md",
      "type": "markdown",
      "size": 1832
    }
  ]
}
```

## 14.4 文件

```http
GET /projects/{project_id}/files
```

查询参数：

```text
path
ref
```

响应：

```json
{
  "path": "docs/order/create.md",
  "type": "markdown",
  "revision": "b81ad7d8c2e4",
  "content": "# 创建订单",
  "last_commit": {
    "id": "b81ad7d8c2e4",
    "message": "docs(order): 更新创建订单",
    "actor_type": "agent",
    "created_at": "..."
  }
}
```

单文件保存可以提供：

```http
PUT /projects/{project_id}/files
```

但该接口必须在内部转换成项目级 ChangeSet，禁止单独实现覆盖文件逻辑。

## 14.5 项目提交

```http
POST /projects/{project_id}/changes
```

## 14.6 Commit

```http
GET  /projects/{project_id}/commits
GET  /projects/{project_id}/commits/{commit_id}
GET  /projects/{project_id}/commits/{commit_id}/diff
POST /projects/{project_id}/commits/{commit_id}/revert
```

Revert 必须创建一个新的 Commit。

不得执行：

```text
git reset --hard
git push --force
删除历史 Commit
```

## 14.7 文件历史

```http
GET /projects/{project_id}/file-history
```

参数：

```text
path
cursor
limit
```

## 14.8 搜索

```http
GET /projects/{project_id}/search?q=order
```

返回：

```json
{
  "items": [
    {
      "path": "docs/order/create.md",
      "title": "创建订单",
      "snippet": "...创建订单接口...",
      "score": 12.8,
      "revision": "b81ad7d8c2e4"
    }
  ]
}
```

## 14.9 Token

```http
GET    /projects/{project_id}/tokens
POST   /projects/{project_id}/tokens
PATCH  /projects/{project_id}/tokens/{token_id}
DELETE /projects/{project_id}/tokens/{token_id}
```

完整 Token 只允许在创建成功后显示一次。

## 14.10 导入导出

```http
POST /projects/import
GET  /projects/{project_id}/export.zip
GET  /projects/{project_id}/export.bundle
```

支持：

- 上传 ZIP 创建项目。
- 上传 Git Bundle。
- 导出当前仓库 ZIP。
- 导出完整 Git Bundle。

远程 Git Clone 可以放到第二阶段。

---

# 15. 搜索设计

使用 SQLite FTS5。

索引内容：

```text
project_id
path
title
plain_text
headings
revision
updated_at
```

Markdown 保存成功后：

1. 从新 Commit 中读取变更文件。
2. 只更新发生变更的文档索引。
3. 删除已被删除文档的索引。
4. 对重命名文档更新路径。

搜索索引属于可重建派生数据。

需要提供命令：

```bash
xwiki reindex
xwiki reindex --project <project_id>
```

系统启动时不得强制全量重建索引。

---

# 16. Markdown 功能

MVP 支持：

- CommonMark
- GitHub Flavored Markdown
- 表格
- 任务列表
- 代码高亮
- 标题锚点
- 相对链接
- 相对图片
- Mermaid
- 自动目录

需要验证：

- Markdown 内部链接是否存在。
- 图片路径是否存在。
- 路径是否超出项目。
- 文件编码是否为 UTF-8。

禁止默认执行 Markdown 中的原始脚本。

HTML 渲染必须执行 XSS 清理。

---

# 17. OpenAPI 功能

识别：

```text
.yaml
.yml
.json
```

当文件位于 `openapi/` 或被判断为 OpenAPI 文档时：

- 提供源码视图。
- 提供 API Reference 视图。
- 使用 Scalar React 组件渲染。
- 支持切换源码和预览。
- 修改 OpenAPI 文件仍然通过 ChangeSet 创建 Git Commit。

OpenAPI 渲染可以作为 MVP 后半阶段，但需要提前预留文件类型体系。

---

# 18. 前端页面

## 18.1 登录页

使用 shadcn/ui：

- Card
- Input
- Button
- Alert
- Form

页面保持简单，不加入注册流程。

首位管理员通过启动参数或 CLI 创建。

## 18.2 项目列表

路由：

```text
/projects
```

显示：

- 项目名称
- 描述
- 更新时间
- 最近 Commit
- 项目状态
- 当前用户角色

支持：

- 创建项目
- 搜索项目
- 归档筛选
- 列表或卡片布局

## 18.3 项目主界面

路由：

```text
/projects/:projectSlug
```

采用三栏布局：

```text
┌────────────────────────────────────────────────────────────┐
│ 项目名  文档  OpenAPI  历史  搜索  Token  设置              │
├──────────────┬──────────────────────────┬──────────────────┤
│ 文档目录树    │ 文档正文或编辑器          │ 当前文件历史      │
│              │                          │                  │
│ README       │ # 创建订单               │ b81ad7 Agent     │
│ docs         │                          │ 8e45be Human     │
│ ├─ auth      │ POST /orders             │                  │
│ └─ order     │                          │                  │
└──────────────┴──────────────────────────┴──────────────────┘
```

要求：

- 左侧宽度可调整。
- 右侧历史栏可折叠。
- 中间正文保持舒适阅读宽度。
- 页面切换不得整页刷新。
- 当前文件路径和 Revision 清晰可见。

## 18.4 文档树

功能：

- 展开和折叠目录。
- 新建文档。
- 新建目录并创建 `index.md`。
- 重命名。
- 移动。
- 删除。
- 复制相对链接。
- 刷新目录。
- 显示 Markdown、OpenAPI、图片等文件类型图标。

拖拽移动可以放到第二阶段。

## 18.5 文档阅读页

显示：

- 面包屑
- 标题
- 最后修改者
- 最后修改时间
- 当前 Commit
- Markdown 正文
- 页面目录
- 编辑按钮
- 复制链接
- 查看历史

## 18.6 文档编辑页

编辑布局支持：

```text
编辑
预览
左右分栏
```

保存区域必须包括：

- Commit Message
- 当前 Base Revision
- 预览改动
- 保存按钮
- 取消按钮

保存前调用 Dry Run。

如果发生 `revision_conflict`：

- 不允许继续覆盖。
- 显示当前 HEAD。
- 显示冲突期间修改过的文件。
- 保留用户本地编辑内容。
- 提供“重新加载最新版本”和“复制当前内容”操作。

## 18.7 项目 Commit 历史

路由：

```text
/projects/:projectSlug/commits
```

显示：

- Commit 短 SHA
- Commit Message
- Actor Type
- Actor Name
- Source
- 修改文件数量
- 新增和删除行数
- 时间

Agent Commit 需要使用 Bot 或 Sparkles 图标区分。

## 18.8 Commit Diff 页面

显示：

- Commit 信息
- Parent Commit
- 文件变更列表
- 文件状态
- 行级 Diff
- 增删行数
- Revert 按钮

需要支持：

- 新增文件
- 删除文件
- 修改文件
- 重命名文件
- Markdown 源码 Diff

第一版不需要实现逐行评论。

## 18.9 Token 页面

显示：

- Token 名称
- Token Prefix
- 权限 Scope
- 允许路径
- 禁止路径
- 到期时间
- 最近使用时间
- 状态

创建 Token 时：

- 完整 Token 只显示一次。
- 提供复制按钮。
- 明确提醒用户妥善保存。

## 18.10 项目设置

支持：

- 项目名称
- Slug
- 描述
- 默认首页
- 项目可见性
- 归档项目
- 导出项目
- 删除项目

危险操作放在独立 Danger Zone。

---

# 19. UI 风格

整体参考：

```text
Outline 的文档布局
Forgejo 的 Commit 和 Diff
Scalar 的 OpenAPI 展示
shadcn/ui 的组件体系
```

视觉要求：

- 简洁。
- 高信息密度但不拥挤。
- 不使用大面积渐变。
- 不设计复杂仪表盘。
- 默认浅色，同时支持深色模式。
- 桌面端优先。
- 适配最小宽度 1280px。
- 窄屏时右侧历史栏自动隐藏。
- 使用系统字体或 Inter。
- Markdown 正文最大宽度约 850px。

优先使用 shadcn/ui：

```text
Button
Card
Dialog
Sheet
DropdownMenu
ContextMenu
Command
Tabs
ResizablePanel
ScrollArea
Table
Badge
Tooltip
AlertDialog
Form
Input
Textarea
Select
Skeleton
Sonner
```

---

# 20. 错误响应规范

统一错误结构：

```json
{
  "error": {
    "code": "revision_conflict",
    "message": "Project revision has changed.",
    "details": {},
    "request_id": "req_01KABC"
  }
}
```

常见错误代码：

```text
authentication_required
permission_denied
project_not_found
file_not_found
invalid_path
unsupported_file_type
revision_conflict
idempotency_conflict
no_changes
validation_failed
token_expired
token_revoked
project_archived
git_operation_failed
internal_error
```

禁止直接将 Git stderr 完整返回给普通用户。

服务端日志可以记录详细 Git 错误，但必须过滤 Token 和敏感字段。

---

# 21. 安全要求

必须实现：

1. 防止目录穿越。
2. 禁止绝对路径。
3. 禁止访问 `.git`。
4. 禁止通过符号链接逃逸仓库。
5. 限制单文件大小。
6. 限制单次提交文件数量。
7. 限制请求体大小。
8. Token 只保存 Hash。
9. Session Cookie 使用 HttpOnly。
10. 生产环境 Cookie 使用 Secure。
11. 设置 SameSite。
12. Markdown HTML 必须清理。
13. 上传文件验证 MIME 和扩展名。
14. Git 命令不得拼接 Shell 字符串。
15. Git 子进程需要超时。
16. 清理 Git 子进程环境变量。
17. 项目删除默认先软删除。
18. 所有写操作写入审计日志。
19. 密码使用 Argon2id 或 bcrypt。
20. API 输出不得泄露仓库绝对路径。

---

# 22. 后端目录结构

建议：

```text
cmd/
└── xwiki/
    └── main.go

internal/
├── app/
├── config/
├── server/
├── httpapi/
│   ├── middleware/
│   ├── handlers/
│   ├── request/
│   └── response/
├── auth/
├── user/
├── project/
├── changeset/
├── gitrepo/
├── document/
├── commit/
├── token/
├── search/
├── audit/
├── store/
│   ├── sqlite/
│   └── migrations/
├── validation/
└── platform/
    ├── clock/
    ├── id/
    └── filesystem/

web/
├── src/
└── dist/

docs/
├── architecture.md
├── api.md
└── development.md
```

`gitrepo` 包至少封装：

```text
InitRepository
GetHead
ListTree
ReadFile
CreateWorktree
ApplyOperations
GenerateDiff
CreateCommit
UpdateRef
ListCommits
GetCommit
GetCommitDiff
GetFileHistory
RevertCommit
ExportBundle
RemoveWorktree
```

业务层不得依赖具体 Git 命令输出文本，应由 `gitrepo` 转换成结构化类型。

---

# 23. 前端目录结构

建议：

```text
web/src/
├── app/
├── routes/
├── components/
│   ├── layout/
│   ├── project/
│   ├── document/
│   ├── commit/
│   ├── diff/
│   └── ui/
├── features/
│   ├── auth/
│   ├── projects/
│   ├── documents/
│   ├── commits/
│   ├── tokens/
│   └── search/
├── hooks/
├── lib/
│   ├── api/
│   ├── markdown/
│   └── utils/
├── stores/
└── types/
```

API 类型应从 OpenAPI 自动生成，避免前后端手工维护两套类型。

---

# 24. CLI 功能

Go 服务同时提供管理命令：

```bash
xwiki serve
xwiki admin create
xwiki project create
xwiki project list
xwiki project export
xwiki project import
xwiki reindex
xwiki doctor
```

`doctor` 检查：

- Git 是否可用。
- Git 版本。
- SQLite 是否可写。
- 数据目录权限。
- 仓库完整性。
- 孤立 Worktree。
- 搜索索引状态。

---

# 25. 日志和可观测性

使用结构化 JSON 日志。

每个请求生成：

```text
request_id
```

日志字段：

```text
request_id
method
path
status
duration
user_id
token_id
project_id
commit_id
error_code
```

禁止记录：

- 完整 Token
- 密码
- Session 内容
- 完整文档正文

提供：

```http
GET /healthz
GET /readyz
```

可选提供：

```http
GET /metrics
```

---

# 26. 测试要求

## 26.1 单元测试

重点覆盖：

- 路径规范化。
- Token Scope。
- allow/deny 路径匹配。
- Commit Trailer 生成。
- Git Diff 解析。
- 幂等键验证。
- Markdown 链接解析。
- 错误响应映射。

## 26.2 Git 集成测试

测试必须使用临时真实 Git 仓库，覆盖：

1. 初始化项目。
2. 创建文件并 Commit。
3. 修改文件。
4. 删除文件。
5. 重命名文件。
6. 多文件单 Commit。
7. stale base revision 返回冲突。
8. `update-ref` Compare-And-Swap 失败。
9. Revert 创建新 Commit。
10. 文件历史跟随重命名。
11. Dry Run 不修改 HEAD。
12. 幂等重试不重复创建 Commit。

禁止完全通过 Mock 替代 Git 集成测试。

## 26.3 API 测试

覆盖：

- 登录。
- 项目创建。
- 权限拒绝。
- Agent Token。
- 批量提交。
- Revision Conflict。
- Idempotency。
- Commit Diff。
- Revert。
- 搜索。
- 归档项目禁止写入。

## 26.4 前端测试

至少覆盖：

- 项目列表。
- 文档树。
- 文档加载。
- 编辑保存。
- 冲突提示。
- Commit 历史。
- Token 创建。
- 权限控制。

使用：

```text
Vitest
React Testing Library
Playwright
```

---

# 27. MVP 阶段规划

## 阶段一：项目骨架

实现：

- Go 服务。
- React + shadcn/ui。
- SQLite Migration。
- 配置系统。
- 日志。
- 登录。
- 管理员 CLI。
- 前端静态资源嵌入。

验收：

- 可以创建管理员。
- 可以登录。
- 服务重启后 Session 和数据库正常。

## 阶段二：项目和 Git 仓库

实现：

- 创建项目。
- 一个项目一个 Git 仓库。
- 初始化 README。
- 项目列表。
- 项目详情。
- 项目归档。

验收：

- 创建两个项目后产生两个独立仓库。
- 两个项目 Commit 历史完全隔离。

## 阶段三：文档读取

实现：

- Git Tree。
- 读取 Markdown。
- Markdown 渲染。
- 文档树。
- 面包屑。
- 项目首页。

验收：

- 不使用 pages/folders 数据库表。
- 页面内容直接从 Git 读取。

## 阶段四：ChangeSet 写入

实现：

- 项目级锁。
- Temporary Worktree。
- Create、Update、Delete、Move。
- Dry Run。
- Git Commit。
- Atomic Update Ref。
- Revision Conflict。

验收：

- 多个文件修改只创建一个 Commit。
- 任意操作失败时不产生 Commit。
- stale revision 返回 409。

## 阶段五：历史和 Diff

实现：

- 项目 Commit 列表。
- 文件历史。
- Commit Detail。
- 机器可读 Diff。
- Revert Commit。

验收：

- API 和网页写入都出现在同一个历史中。
- Revert 不删除原历史，而是创建新 Commit。

## 阶段六：Agent Token

实现：

- 创建 Token。
- Scope。
- 项目限制。
- 路径限制。
- 幂等键。
- 审计日志。

验收：

- Token 无法访问未授权项目。
- Token 无法写入禁止目录。
- 相同幂等请求不产生重复 Commit。

## 阶段七：搜索

实现：

- SQLite FTS5。
- 增量索引。
- 项目内搜索。
- Reindex CLI。

## 阶段八：OpenAPI 和导入导出

实现：

- Scalar 预览。
- ZIP 导入导出。
- Git Bundle 导入导出。
- 图片和附件。

---

# 28. MVP 明确不做

第一版不要实现：

- 实时多人协作。
- WebSocket 协同编辑。
- 评论系统。
- 行级评论。
- Pull Request。
- 分支可视化管理。
- 复杂审批流。
- 邮件通知。
- 第三方 OAuth。
- 插件市场。
- 自定义主题市场。
- 数据大屏。
- 复杂组织架构。
- 空文件夹。
- 在线 Shell。
- 任意 Git 命令执行。
- 自动 AI 生成 Commit Message。
- Agent 自动解决冲突。

系统应优先保证：

```text
写入可靠
版本可靠
权限可靠
历史可靠
API 可靠
```

---

# 29. 关键验收标准

项目完成后必须满足以下条件：

1. 创建两个项目时，生成两个独立 Git 仓库。
2. 网页修改 Markdown 后创建真实 Git Commit。
3. Agent API 修改 Markdown 后创建真实 Git Commit。
4. 网页和 API 使用完全相同的 ChangeSet 服务。
5. 一次修改多个文件只产生一个 Commit。
6. 一个操作失败时不允许产生部分 Commit。
7. 提供过期 Base Revision 时返回 409。
8. 不允许静默覆盖最新版本。
9. 相同 Idempotency-Key 重试不重复创建 Commit。
10. Commit 修改文件列表从 Git Diff 获取，而不是解析 Commit Message。
11. 页面历史支持文件重命名跟踪。
12. Revert 创建新 Commit，不删除历史。
13. 项目归档后所有写操作被拒绝。
14. Agent Token 可以限制项目和目录。
15. 数据库中不保存 Markdown 正文和页面版本。
16. 搜索索引可以完整重建。
17. Markdown 渲染不存在明显 XSS。
18. 路径无法逃逸项目仓库。
19. 项目能够导出为完整 Git Bundle。
20. Docker Compose 可以直接启动系统。

---

# 30. Agent 开发执行要求

开发时必须遵守：

1. 先检查现有仓库结构，再制定实施步骤。
2. 先完成架构和数据模型，再开始堆叠页面。
3. 每个阶段完成后必须运行测试。
4. 不得用内存假数据代替真实数据库或 Git。
5. 不得创建无实现的占位接口。
6. 不得为追求速度绕过 Git Commit 链路。
7. 不得在网页编辑接口中直接覆盖文件。
8. 不得建立第二套文档版本数据库。
9. 所有 Git 操作必须有超时和错误处理。
10. 所有写入必须经过权限、Revision 和路径校验。
11. 优先提交小而完整、可测试的改动。
12. 每完成一个阶段，更新 README 和架构文档。
13. API 变化时同步更新 OpenAPI。
14. 前后端类型从 OpenAPI 生成。
15. 遇到不明确的非核心细节时，选择最简单、可维护的实现，不扩展产品边界。

最终交付：

```text
可运行的 Go 后端
React + shadcn/ui 前端
SQLite Migration
Git 仓库服务
REST API
OpenAPI 文档
Dockerfile
docker-compose.yml
测试套件
README
架构说明
示例项目
```