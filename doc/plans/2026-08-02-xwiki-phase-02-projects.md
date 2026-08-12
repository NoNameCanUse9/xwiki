# XWiki 阶段二：项目和 Git 仓库 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 实现项目（Project）域：创建项目时自动初始化一个独立的 Git 仓库（bare，含 README root commit）；提供列表、详情、归档；API 与前端均可用；验收「两个项目两个独立仓库、Commit 历史完全隔离」。

**Architecture:** 新增 `internal/project` 包（store + repo + service 三层），SQLite `projects` 表记录元数据，Git 仓库落在 `<dataDir>/repos/<projectID>/repo.git`（bare）。阶段二用 plumbing（write-tree → commit-tree → update-ref）创建 README root commit，不引入 worktree（阶段四才需要）。HTTP 层挂 `/api/v1/projects`（SessionAuth 保护），前端 home 页改为项目列表 + 创建/归档，新增 `/projects/:id` 详情页。

**Tech Stack:** 沿用阶段一：Go 1.26 / chi v5 / modernc.org/sqlite / goose / ULID；React 19 / TS / Tailwind v4 / shadcn/ui / React Router v7 / TanStack Query / Zod / Vitest + RTL。Git 操作仅用 `git` CLI（bare init + plumbing commit），不引入 go-git 依赖。

---

## 0. 范围（本计划 = spec 阶段二）

**做：**

- 迁移 `00002_projects.sql`：`projects` 表（name 唯一、description、repo_dir、archived_at、时间戳）
- `internal/project`：store（SQLite CRUD）、repo（git bare init + README root commit + 只读查询）、service（Create/List/Get/Archive，协调两层）
- API（SessionAuth）：`POST/GET /api/v1/projects`、`GET /api/v1/projects/{id}`、`POST /api/v1/projects/{id}/archive`
- 错误语义：400 校验失败、404 不存在、409 name 冲突、401 未认证
- 前端：api client（`lib/api/projects.ts`）、home 改造为项目列表（创建 dialog、归档按钮、空态）、`/projects/:id` 详情页 + 路由
- 文档：`doc/api.md` 补 projects 段、`doc/architecture.md` 补 project 层与 repo 布局、README 功能清单、plans 索引

**不做（后续阶段）：** 文档树/Git Tree 读取（三）、ChangeSet 写入/worktree/锁（四）、历史/Diff（五）、Agent Token（六）、搜索（七）、OpenAPI/导入导出（八）、项目重命名/删除/取消归档（未列入 spec）、非管理员创建限制（MVP 阶段二不做授权分层）。

**验收标准（spec §27 阶段二）：**

1. 创建两个项目后产生两个独立 bare 仓库（目录不同、各自只有一个 root commit）。
2. 两个项目 Commit 历史完全隔离（互不可见对方 refs/objects）。
3. 创建时自动初始化 README（root commit 内含 README.md，内容含项目名/描述/时间）。
4. 归档后项目仍在列表（标记 archived），详情可读，不可重复归档报错（幂等返回已归档状态）。
5. API 未认证返回 401；name 冲突返回 409；不存在返回 404。

## 1. 文件结构（本阶段创建/修改）

```text
xwiki/
├── internal/
│   ├── project/                     （新包）
│   │   ├── project.go               （Project 模型 + 校验）
│   │   ├── store.go                 （SQLite CRUD）
│   │   ├── store_test.go
│   │   ├── repo.go                  （git bare init + README root commit + 查询）
│   │   ├── repo_test.go
│   │   ├── service.go               （Create/List/Get/Archive）
│   │   └── service_test.go
│   ├── store/sqlite/migrations/
│   │   └── 00002_projects.sql       （新）
│   ├── httpapi/handlers/
│   │   ├── projects.go              （新）
│   │   └── projects_test.go         （新）
│   ├── server/router.go             （修改：projects 路由）
│   └── app/app.go                   （修改：装配 project.Service）
├── web/src/
│   ├── lib/api/
│   │   ├── types.ts                 （修改：Project 类型）
│   │   └── projects.ts              （新：list/create/get/archive）
│   ├── routes/
│   │   ├── home.tsx                 （修改：项目列表 + 创建 + 归档）
│   │   ├── home.test.tsx            （新）
│   │   └── project-detail.tsx       （新：/projects/:id）
│   ├── app/router.tsx               （修改：详情路由）
│   └── components/
│       └── project-create-dialog.tsx（新）
├── doc/
│   ├── api.md                       （修改：projects 段）
│   ├── architecture.md              （修改：project 层 + repo 布局）
│   └── plans/README.md              （修改：索引）
└── README.md                        （修改：功能清单）
```

## 2. API 设计

统一错误信封沿用 `{error:{code,message,request_id}}`。全部需认证（401 未登录）。

```http
POST /api/v1/projects          # 创建
  req:  { "name": "docs-site", "description": "产品文档" }
  resp: 201 { "project": { "id":"prj_...", "name":"docs-site", "description":"产品文档",
                            "repo_dir":"repos/prj_xxx/repo.git", "archived":false,
                            "created_at":"...", "updated_at":"..." } }
  err:  400 invalid_name / invalid_body；409 project_name_conflict

GET /api/v1/projects           # 列表（含已归档，按 created_at 倒序）
  resp: 200 { "projects": [ ... ] }

GET /api/v1/projects/{id}      # 详情
  err:  404 project_not_found

POST /api/v1/projects/{id}/archive   # 归档（幂等：已归档返回 200 且 archived=true）
  resp: 200 { "project": { ..., "archived": true } }
  err:  404 project_not_found
```

## 3. 数据模型（迁移 00002）

```sql
CREATE TABLE projects (
    id          TEXT PRIMARY KEY,             -- prj_<ulid>
    name        TEXT NOT NULL UNIQUE,         -- 小写字母数字连字符
    description TEXT NOT NULL DEFAULT '',
    repo_dir    TEXT NOT NULL,                -- repos/<id>/repo.git（相对 dataDir）
    archived_at TEXT,                         -- NULL = 活跃；归档时写入时间戳
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX idx_projects_archived_at ON projects(archived_at);
```

`Project` Go 结构用 `Archived bool` 对外，`archived_at` 内部存储；Service 负责映射。

## 4. 任务清单（严格 TDD：先写失败测试 → 实现 → 全绿）

- [x] **Task 1** 迁移 `00002_projects.sql` + `internal/project/project.go`（模型 + `ValidateName`：1-64 字符、`^[a-z0-9]+(-[a-z0-9]+)*$`、拒绝 `..`/空格/大写/`/`）+ `store_test.go`（RED：Create/GetByID/List/Archive/GetByID 不存在 → ErrNotFound/name 冲突 → ErrConflict）→ 实现 `store.go`（复用 `user.Store` 的 DB 访问模式）→ GREEN。
- [x] **Task 2** `repo_test.go`（RED：`InitBare` 后目录存在且 `git --git-dir ... rev-parse --is-bare-repository` = true；`WriteReadme` 后 `rev-parse HEAD` 存在、`cat-file -p HEAD^{tree}` 含 `README.md` blob、`log --oneline` 恰一条；两个 repo 的 `git --git-dir A rev-parse HEAD` 与 B 互不相同且 A 中 `cat-file -e B 的 commit` 失败）→ 实现 `repo.go`（`exec.Command("git", ...)`，plumbing：hash-object -w / mktree / commit-tree / update-ref；`CommitReadme` 一步完成）→ GREEN。
- [x] **Task 3** `service_test.go`（RED：Create 成功 → store 有记录 + repo 目录存在 + README commit 存在；name 非法 → ErrInvalidName 且无 repo 残留；Create 重复 → ErrConflict；List 排序；Archive 幂等；Get 不存在 → ErrNotFound）→ 实现 `service.go`（`projectID := id.New("prj")`；先写 store 再 init repo？**先 init repo 再写 store**，repo 失败时无半成品记录；Archive 用 `COALESCE` 语义的 UPDATE：`archived_at = COALESCE(archived_at, ?)` 幂等）→ GREEN。
- [x] **Task 4** `projects_test.go`（RED：未认证 401；创建 201 且响应结构正确；name 冲突 409；非法 name 400；列表 200；详情 404/200；归档 200 幂等）→ 实现 `handlers/projects.go`（沿用 auth.go 的 DecodeJSON/WriteJSON/WriteError 模式；handler 持有 service 引用；RequestID 已由中间件注入）→ GREEN。
- [x] **Task 5** `router.go` 挂 `r.Route("/projects", ...)`（SessionAuth 组内，POST/GET/GET{id}/POST{id}/archive）+ `app.go` 装配 `project.NewService(db, cfg.DataDir)` → `go test ./...` 全绿 + `go vet ./...`。
- [x] **Task 6** 前端 `lib/api/projects.ts`（`listProjects/getProject/createProject/archiveProject`，泛型 `apiFetch` 复用 client.ts；`types.ts` 加 `Project`）+ `projects.test.ts`（RED：mock fetch 断言 URL/方法/body/错误处理）→ GREEN。
- [x] **Task 7** `project-create-dialog.tsx`（shadcn Dialog + RHF + zod：name/description；成功 toast + 刷新列表；409 → 字段错误）+ `home.tsx` 改造（useQuery 项目列表；空态「还没有项目」+ 创建按钮；卡片列表：name/description/创建时间/归档标记；归档按钮 confirm + toast）+ `home.test.tsx`（RED：mock api 模块，断言空态、列表渲染、创建流程、归档调用）→ GREEN。
- [x] **Task 8** `project-detail.tsx`（`/projects/:id`：useQuery 详情；基本信息 hairline-panel；归档状态；返回列表链接；404 → 错误态）+ `router.tsx` 加路由（`/projects/:id` 在 Protected 下）→ 补测试（详情渲染 + 404 态）→ GREEN。
- [x] **Task 9** 文档：`doc/api.md` 补 projects 段（含 curl 示例）；`doc/architecture.md` 补 project 层、repo 布局、错误码表；`README.md` 功能清单补项目管理；`doc/plans/README.md` 索引补阶段二；本计划文件勾选全部完成。
- [x] **Task 10** 端到端验收（每步记录证据）：
  1. `go test ./... -count=1`、`go vet ./...`、`npx vitest run` 全绿
  2. `npm run build` → `go build` → 启动 → curl 登录 → 创建 `proj-a`、`proj-b`
  3. 断言两个独立仓库：`ls data/repos/` 两个目录；各 `git --git-dir ... rev-parse HEAD` 不同
  4. 归档 `proj-a` → 列表含 archived 标记 → 详情 archived=true
  5. 冒烟前端：列表页显示两项目、创建 dialog 可用
  6. `git restore web/dist/index.html`（占位符策略）

## 5. 风险与决策

- **Git 依赖**：用 `git` CLI（阶段一 Docker 镜像已含 git？——Dockerfile 需确认，若缺则补 `apt-get install git`）。全部 plumbing 命令带 `--git-dir` 绝对路径，不依赖 cwd。
- **repo 失败残留**：先 init repo 再写 store；store 写入失败时删除已建 repo 目录（`os.RemoveAll` 兜底）。
- **名称规范**：`^[a-z0-9]+(-[a-z0-9]+)*$`，与 Git 仓库目录名安全兼容（无空格/斜杠/点）。
- **归档语义**：软归档（标记），不删除仓库；列表默认含归档（前端分组显示）；阶段二不做取消归档。
- **JSON 字段**：对外 `archived`（bool），内部 `archived_at`（TEXT/空）。
- **README 内容**：`# <name>\n\n<description>\n\nXWiki 项目 · <UTC 时间>\n`，经 `hash-object -w --stdin` 写入。
