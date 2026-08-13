# XWiki 阶段三：文档读取 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 在项目内直接读取 Git 中的文档：目录树（Git Tree）、Markdown 原始/渲染读取、前端文档树导航、面包屑、项目首页（README 渲染）。**验收：不使用 pages/folders 数据库表，页面内容全部从 Git 读取。**

**Architecture:** 扩展 `internal/project` 包为只读 Git 查询（ls-tree 单层 + cat-file blob/tree + rev-parse 默认分支）；新增 `internal/httpapi/handlers/docs.go` 提供 tree/raw/html 三类端点；渲染用 **goldmark**（后端渲染，默认不渲染 raw HTML，安全）；前端新增 `/projects/:id/docs/*` 浏览路由：左侧文档树（递归懒加载）+ 右侧 Markdown 渲染 + 面包屑；`/projects/:id` 详情页增加「阅读文档」入口并渲染项目首页（README）。

**Tech Stack:** 新增 `github.com/yuin/goldmark`；其余沿用阶段二。

---

## 0. 范围（本计划 = spec 阶段三）

**做：**

- `internal/project/repo.go` 扩展：`DefaultBranch`（rev-parse --abbrev-ref HEAD）、`ListTree`（ls-tree 单层，返回 type/sha/path）、`ReadBlob`（cat-file blob，限制大小）、`ReadFile`（path → blob，含路径穿越防护）、`TreeMeta`（path → tree sha）
- API（SessionAuth，前缀 `/api/v1/projects/{id}/`）：
  - `GET docs/tree?path=dir/` → 200 `{tree:[{name,type,path}]}`（单层）
  - `GET docs/pages/{path}` → 200 `{path,format:"raw",content}`；`?format=html` → `{path,format:"html",content:"<渲染后>"}`；404 文档不存在 / 400 非法路径
  - `GET docs/home` → README.md（或 `docs/README.md` 兜底 → 404）渲染：与 pages 相同响应
- 路径安全：拒绝绝对路径、`..` 穿越、空段；只允许 `.md`/`.markdown`/`.mdx`？——MVP：所有 blob 可读，渲染仅 markdown（其他按 raw 文本返回）
- 前端：`/projects/:id` 详情页加「文档」区块（首页预览 + 进入阅读）；`/projects/:id/docs/*` 浏览页（树侧栏懒加载 + 内容 + 面包屑 + 返回）；渲染器：`dangerouslySetInnerHTML` + 简单 sanitize（阶段三用基础文本清理，不含脚本标签）
- 文档：api.md 补 docs 段；architecture.md 补读取路径；README 功能清单；plans 索引

**不做（后续阶段）：** ChangeSet 写入/锁/worktree（四）、历史/Diff（五）、Agent Token（六）、搜索（七）、OpenAPI（八）、PDF/图片渲染、语法高亮（后续增强）、XSS 完整防护策略（goldmark 默认安全 + 前端基础清理）。

**验收标准（spec §27 阶段三）：**

1. 项目内容直接从 Git 读取——`GET docs/tree` 与 `GET docs/pages` 均不查数据库表（projects 表仅用于定位仓库）。
2. 创建项目后（README root commit）即可读：tree 含 README.md，pages/README.md 返回内容，home 返回渲染 HTML。
3. 手动向仓库提交新文档（git commit 直写）后无需任何索引即可读取——验收时用 CLI 往项目仓库追加 doc.md 并立即读取。

## 1. 文件结构

```text
internal/project/
├── repo.go            （修改：+ListTree/ReadBlob/ReadFile/DefaultBranch）
├── repo_test.go       （修改：树读取、blob 读取、穿越防护测试）
internal/httpapi/handlers/
├── docs.go            （新：tree/pages/home 三个 handler）
└── docs_test.go       （新：集成测试走 router）
internal/server/router.go        （修改：docs 路由）
web/src/
├── lib/api/docs.ts    （新：getTree/getPage/getHome）
├── lib/api/docs.test.ts
├── routes/project-detail.tsx    （修改：文档区块 + 阅读入口）
├── routes/docs-viewer.tsx       （新：/projects/:id/docs/* 浏览页）
├── routes/docs-viewer.test.tsx
└── app/router.tsx     （修改：docs 路由）
doc/api.md / doc/architecture.md / README.md / doc/plans/README.md  （修改）
```

## 2. API 设计

```http
GET /api/v1/projects/{id}/docs/tree?path=            # 单层目录
  → 200 {"path":"","tree":[{"name":"README.md","type":"blob","path":"README.md"},...]}
  → 404 project_not_found / doc_not_found（目录不存在）

GET /api/v1/projects/{id}/docs/pages/{path...}      # 读取文档（URL 路径参数，可含 /）
  → 200 {"path":"docs/guide.md","format":"raw","content":"..."}
  ?format=html → {"path":...,"format":"html","content":"<article>...</article>"}
  → 400 invalid_doc_path（穿越/绝对/空）· 404 doc_not_found（blob 不存在或非 blob）

GET /api/v1/projects/{id}/docs/home
  → README.md 优先，docs/README.md 兜底
  → 200 同上 · 404 doc_not_found
```

## 3. 安全与决策

- **路径规范化**：`filepath.Clean` + 拒绝结果含 `..`/以 `/` 开头/空路径；Git 侧始终用正斜杠相对路径。
- **渲染**：goldmark（默认扩展：GFM table/strikethrough/autolink），不启用 raw HTML 输出；`format=html` 时输出完整 `<article>` 片段。
- **大小限制**：blob 读取上限 2 MiB（超出 → 413 doc_too_large）。
- **树排序**：目录在前、名称排序（ls-tree 输出已按名排序，直接处理）。
- **前端渲染**：html 响应直接注入；raw 响应用 `<pre>` 显示。

## 4. 任务清单（严格 TDD）

- [x] **Task 1** repo 扩展测试（RED：DefaultBranch=main；ListTree 单层含 README.md（root）与 docs/（tree 类型）；ReadBlob 内容；ReadFile 穿越 `../` 拒绝、绝对路径拒绝、缺失 blob 报错；TreeMeta docs/ 返回 tree sha）→ 实现 repo.go 扩展（ls-tree/cat-file 复用 gitOutput；path 用 `--` 分隔防选项注入）→ GREEN。
- [x] **Task 2** handlers/docs.go 测试（RED：集成——创建项目 → tree 根层；pages/README.md raw+html；home；穿越 400；缺失 404；未认证 401）→ 实现（DocsHandler{cfg,svc,log}；goldmark 渲染器单例；路径校验函数）→ 挂 router（/projects/{id}/docs 组，SessionAuth）→ GREEN。
- [x] **Task 3** 前端 docs api client（getTree/getPage/getHome + 测试：URL 编码、format 参数）→ GREEN。
- [x] **Task 4** project-detail.tsx 加「文档」区块（getHome 渲染 + 「阅读文档」按钮 → /projects/:id/docs）；docs-viewer.tsx：树侧栏（getTree 递归：目录可展开，点击加载子层；blob 点击读页）+ 内容区（html 注入 / raw pre）+ 面包屑（path 分段链接）+ 404/错误态；router.tsx 挂 `/projects/:id/docs/*`；docs-viewer.test.tsx（mock api：树渲染、点击展开、面包屑、错误态）→ GREEN。
- [x] **Task 5** 文档：api.md docs 段、architecture.md 读取路径段、README 功能清单、plans 索引；勾选本计划。
- [x] **Task 6** 端到端验收：go test/vet + vitest 全绿；构建重启；API 冒烟（tree/pages/home）；**手动向 proj 仓库追加新文档并立即读取**（无索引即读，验收标准 3）；前端页面截图冒烟；`git restore web/dist/index.html`。

## 5. 风险

- goldmark 依赖新增：go get 网络（Docker Hub 不稳但 Go proxy 正常）。
- 渲染 XSS：goldmark 默认转义 raw HTML → 安全；前端注入 html 响应前做基础清洗（去掉 script/iframe 标签）。
