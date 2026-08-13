# XWiki 阶段七：搜索 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** SQLite FTS5 全文搜索：写入后增量索引、项目内搜索 API、`xwiki reindex` CLI 全量重建。

**Architecture:** 新迁移 `00004_search.sql`（FTS5 虚拟表 + 索引状态表）；`internal/search` 包（store：FTS upsert/delete/query；service：`ReindexProject` 增量——walk Git 树，按 (project_id, path) 对比 blob sha，只更新变化的条目；`Search`：FTS5 MATCH + snippet）。changesets/revert 成功后同步调用增量索引（先同步，规模小）；CLI `xwiki reindex [--project id]` 全量。

**Tech Stack:** modernc sqlite 支持 FTS5（编译进驱动）；无新依赖。

---

## 0. 范围（本计划 = spec 阶段七）

**做：**

- 迁移 00004：
  - `doc_search`（FTS5，content 外挂表）：`doc_search_content(project_id, path, blob_sha, content)` + `doc_search`（content='doc_search_content', content_rowid=rowid）+ `doc_index_state(project_id, path, blob_sha, updated_at)`（去重来源）
  - 实际设计合并：`doc_index_state` 即内容表（外挂 FTS），一表两用
- `internal/search`：
  - `ReindexProject(ctx, projectID)`：走 Git 树 → 每 blob 计算 sha → upsert 变更项、删除消失项；返回 {indexed, removed}
  - `Search(ctx, projectID, query, limit)`：FTS5 MATCH（用户词用 `"word"*` 前缀 + 转义）→ `{path, snippet}`（snippet() 高亮 <> 转义为普通文本）
  - `SearchAll(ctx, query, limit)`（可选跨项目，token 权限在 handler 层过滤）——MVP 仅项目内
- 挂勾：changesets.Apply 成功后（非 dry-run、非重放）与 history.Revert 成功后 → `searchSvc.ReindexProject`（同步；错误仅记日志不阻断写入）
- API：`GET /api/v1/projects/{id}/search?q=...&limit=` → 200 `{query, results:[{path, snippet}]}`（session 或 agent read 认证 + 项目绑定）
- CLI：`xwiki reindex [--project <id>]`（全量重建全部或单项目；打印统计）
- 前端：docs-viewer 顶部搜索框（提交 → 结果列表面板，点击跳转文件）
- 文档：api.md / architecture.md / README / plans 索引

**不做（后续阶段）：** OpenAPI/导入导出（八）、搜索高亮渲染、跨项目聚合搜索、停用词调优、模糊搜索。

**验收（spec §27 阶段七）：**

1. 写入后立即可搜：changeset 创建/更新文档 → search 返回命中（增量索引）。
2. `xwiki reindex` 全量重建后结果一致。

## 1. 文件结构

```text
internal/store/sqlite/migrations/00004_search.sql   （新）
internal/search/
├── store.go / store_test.go      （FTS upsert/delete/query）
├── service.go / service_test.go  （增量 reindex + search）
internal/httpapi/handlers/search.go + 测试   （新：search 端点）
internal/server/router.go          （修改）
internal/httpapi/handlers/changesets.go / history.go  （修改：成功后 reindex）
cmd/xwiki/main.go              （修改：reindex 命令）
internal/app/app.go                （修改：装配 search.Service + Reindex 方法）
web/src/lib/api/search.ts + test
web/src/routes/docs-viewer.tsx     （修改：搜索框 + 结果面板）
doc/api.md / doc/architecture.md / README.md / doc/plans/README.md  （修改）
```

## 2. API 设计

```http
GET /api/v1/projects/{id}/search?q=docs&limit=10
  → 200 {"query":"docs","results":[{"path":"docs/guide.md","snippet":"... docs ..."}]}
  err: 400 invalid_query（空/超长）· 403 agent_forbidden（token 未绑定）· 404 project_not_found
```

## 3. 决策

- **索引内容**：markdown/文本 blob（≤ 2 MiB）全文；二进制跳过（按 blob 大小/内容判断——MVP：全部文本化尝试，含 `\x00` 的跳过）。
- **增量**：walk 树对比 blob_sha（hash-object 已算），逐文件 upsert/delete；大项目也线性可控。
- **FTS 查询**：用户输入分词后每个词 `"<词>"*` 前缀匹配（AND）；转义双引号；空查询拒绝。
- **snippet**：`snippet(doc_search, 2, '[', ']', '…', 24)` 输出后把 `[`/`]` 转义为普通字符（防注入）。
- **同步索引**：写路径串行（项目锁内）→ 索引一致；reindex 失败仅日志（搜索可能短暂陈旧）。
- **CLI**：`xwiki reindex`（全量）、`--project <id>` 单项目；exit 0 + 统计。

## 4. 任务清单（严格 TDD）

- [x] **Task 1** 迁移 + search store 测试（RED：upsert 后 query 命中；delete 后消失；blob_sha 相同不重复写；snippet 输出；空结果）→ 实现 store.go → GREEN。
- [x] **Task 2** search service 测试（RED：ReindexProject 对 repo 全树建索引；再次调用无变化（幂等）；Git 树变化（新增/修改/删除）后 reindex 增量正确；Search 命中/无命中/查询转义）→ 实现 service.go → GREEN。
- [x] **Task 3** 集成测试（RED：写入 changeset → search 命中新文档；revert → 文档消失；Bearer read token 可搜、未绑定 403；空 q 400）→ 实现 search handler + router + changesets/revert 挂勾 → GREEN。
- [x] **Task 4** CLI reindex（测试：run reindex 子命令输出统计）→ 实现 → GREEN。
- [x] **Task 5** 前端搜索（docs-viewer 搜索框 + 结果面板 + 跳转 + 测试）+ API client → GREEN。
- [x] **Task 6** 文档更新 + 勾选 + 端到端验收（构建重启 → 写入 → 搜索命中 → reindex CLI → 结果一致 → 前端冒烟）。

## 5. 风险

- FTS5 在 modernc 中可用（自带）；snippet 函数需 FTS5 表结构正确。
- 中文分词：FTS5 默认 unicode61 对中文按整句切——MVP 接受（搜索连续词），不做 jieba。
