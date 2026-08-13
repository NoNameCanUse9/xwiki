# XWiki 阶段四：ChangeSet 写入 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 通过 API 原子写入文档：创建/更新/删除/移动文件，一次提交一个 ChangeSet；项目级锁 + 临时 worktree + 带旧值校验的 update-ref（CAS）；stale revision 返回 409；dry-run 不产生提交。

**Architecture:** `internal/project/changeset.go` 新增变更应用器（per-project 内存锁 → 临时 worktree → 应用变更 → write-tree/commit-tree → `update-ref <ref> <new> <old>` CAS → 清理 worktree）。revision = HEAD sha（响应与请求均携带）。`internal/httpapi/handlers/changesets.go` 提供 REST 端点。前端：docs-viewer 增加单文件编辑（textarea + 保存 ChangeSet + 冲突提示）。

**Tech Stack:** 无新依赖；git worktree/plumbing 命令沿用。

---

## 0. 范围（本计划 = spec 阶段四）

**做：**

- `internal/project/changeset.go`：`ApplyChangeset(ctx, projectID, cs ChangesetInput) (*ChangesetResult, error)`（dry_run 时返回预览）
  - 项目级锁：`sync.Map[string]*sync.Mutex`（跨请求串行化同项目写入）
  - 临时 worktree：`git worktree add --detach <tmpdir> <branch>`（detach 不移动 ref，安全）
  - 变更应用：create/update（写文件+父目录 mkdir）、delete（删文件）、move（rename，含跨目录）
  - 路径校验：validateDocPath + 拒绝操作已归档项目（ErrArchived）
  - 提交：worktree 内 `git add -A` → `git write-tree` → `git commit-tree -p <base>` → `git update-ref refs/heads/main <new> <old>`（old=base revision，失败 → ErrConflict）
  - dry_run：只应用变更 + write-tree，返回 preview（tree sha + 变更清单），不写任何 ref
  - 失败清理：任何错误 → worktree remove --force + rmdir 临时目录
- API（SessionAuth）：
  - `POST /api/v1/projects/{id}/changesets`：`{base_revision, message, changes:[{op:"create|update|delete|move", path, content, new_path}]}` → 200/201 `{commit:{sha, message}, revision, changes:[...]}`；409 revision_conflict；400 校验；404 项目不存在
  - `?dry_run=true` → 200 `{preview:{tree, changes:[...]}}` 不写 ref
  - `GET /api/v1/projects/{id}/revision` → 200 `{revision}`（供编辑前读取 base）
- 前端：docs-viewer 文件视图增加「编辑」按钮 → 编辑面板（textarea 载入 raw 内容 + 保存 → 提交 ChangeSet → 刷新）；冲突（409）提示重新加载；delete/move 本阶段仅 API（前端不加）
- 文档：api.md changesets 段；architecture.md 写入路径；README；plans 索引

**不做（后续阶段）：** 历史/Diff（五）、Agent Token（六）、搜索（七）、OpenAPI/导入导出（八）、并发冲突自动合并、分支操作、目录级操作（MVP 文件级）、归档项目写入。

**验收标准（spec §27 阶段四）：**

1. 多个文件修改只创建一个 Commit（一次 changeset 含 create+update → HEAD 前进 1）。
2. 任意操作失败时不产生 Commit（非法路径/冲突 → HEAD 不变、worktree 无残留）。
3. stale revision 返回 409（并发两次提交，第二次带旧 revision → 409，HEAD 仍为第一次结果）。

## 1. 文件结构

```text
internal/project/
├── changeset.go        （新：锁 + worktree + 应用 + CAS 提交）
├── changeset_test.go   （新：单提交多文件、失败无残留、409、dry-run、归档拒绝、路径穿越）
internal/httpapi/handlers/
├── changesets.go       （新：POST changesets / GET revision）
└── changesets_test.go  （新：集成测试）
internal/server/router.go        （修改）
web/src/
├── lib/api/changesets.ts        （新：getRevision/submitChangeset）
├── lib/api/changesets.test.ts
└── routes/docs-viewer.tsx       （修改：编辑面板）
doc/api.md / doc/architecture.md / README.md / doc/plans/README.md  （修改）
```

## 2. API 设计

```http
GET /api/v1/projects/{id}/revision
  → 200 {"revision":"<40-hex>"} · 404 project_not_found

POST /api/v1/projects/{id}/changesets
  req: {"base_revision":"<40-hex>","message":"update docs",
        "changes":[{"op":"create|update|delete|move","path":"docs/a.md",
                    "content":"# A\n","new_path":"docs/b.md"}]}
  → 200 {"commit":{"sha":"<40-hex>","message":"update docs"},"revision":"<new-40-hex>",
         "changes":[{"op":"create","path":"docs/a.md","status":"created"}]}
  ?dry_run=true → 200 {"preview":{"tree":"<40-hex>","changes":[...]}}（不写 ref）
  err: 400 invalid_changeset / invalid_doc_path · 404 project_not_found
       · 409 revision_conflict · 410 project_archived
```

## 3. 决策

- **锁粒度**：进程内 per-project mutex（MVP 单实例；多实例由 update-ref CAS 兜底）。
- **原子性**：`update-ref <ref> <new> <old>` 是 Git 内建 CAS——old 不匹配即失败，天然 409，无并发窗口。
- **revision 语义**：base_revision = 客户端读取时的 HEAD sha；提交时与当前 HEAD 比对。
- **worktree**：`--detach` 避免分支移动；提交后 `worktree remove --force` + `os.RemoveAll`。
- **content 限制**：单文件 ≤ 2 MiB（413）；changeset 总变更 ≤ 100 个。
- **delete/move**：目标路径校验同 create；move 的 new_path 也校验。
- **幂等**：本阶段不做幂等键（阶段六 Agent Token 引入）。

## 4. 任务清单（严格 TDD）

- [x] **Task 1** `changeset_test.go`（RED）：a) 单 changeset 含 create+update+move → HEAD 前进 1、树含全部变更、提交消息正确；b) 失败（非法路径）→ HEAD 不变、`git worktree list` 无残留；c) 带旧 base 第二次提交 → ErrConflict；d) dry_run → 无新 commit、返回 preview；e) 归档项目 → ErrArchived；f) 路径穿越 → 校验拒绝 → 实现 `changeset.go`（锁/worktree/apply/write-tree/commit-tree/CAS update-ref/清理）→ GREEN。
- [x] **Task 2** `changesets_test.go`（RED：集成——revision 端点；changesets 成功 200 + revision 前进；stale 409；dry_run 无写入；归档 410；非法路径 400；未认证 401）→ 实现 `changesets.go` + router 挂载 → GREEN。
- [x] **Task 3** 前端 `lib/api/changesets.ts`（getRevision/submitChangeset + 测试）→ GREEN。
- [x] **Task 4** docs-viewer 编辑面板（文件视图「编辑」→ textarea 载入 raw + base revision 隐藏 + 保存 → submitChangeset → 刷新内容与树；409 → 冲突提示重新加载）+ 测试（mock：编辑流、409 提示）→ GREEN。
- [x] **Task 5** 文档更新（api/architecture/README/plans 索引）+ 勾选计划。
- [x] **Task 6** 端到端验收：全测试；构建重启；curl 冒烟（revision → changesets 创建 → 内容变化 → stale 409 → dry-run 无副作用）；前端编辑冒烟；`git restore web/dist/index.html`。

## 5. 风险

- worktree 残留：defer 清理 + 测试断言无残留；极端失败由 update-ref CAS 保证 ref 不动。
- 并发：单实例内存锁；跨实例 CAS 兜底（验收 3 覆盖）。
