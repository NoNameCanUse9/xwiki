# AgentDocs 阶段五：历史和 Diff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 提供完整只读历史视图与安全回滚：项目 commit 列表、commit 详情（文件清单）、文件历史、机器可读 diff、revert（创建新 commit，不删除原历史）。**验收：API 与网页写入出现在同一历史；Revert 是新增 commit。**

**Architecture:** `internal/project/history.go` 扩展只读查询（git log / show / diff-tree --numstat / log --follow）；revert 用 worktree + `git show` 生成 patch → `git apply -R --check`（预检）→ 应用 → commit-tree → CAS update-ref（复用阶段四锁与提交管线）。`internal/httpapi/handlers/history.go` 提供端点。

**Tech Stack:** 无新依赖；全部 git CLI。

---

## 0. 范围（本计划 = spec 阶段五）

**做：**

- `internal/project/history.go`：
  - `ListCommits(ctx, projectID, limit, offset)` → `[{sha, message, author, date}]`（`log --format=%H%x1f%s%x1f%an%x1f%aI -n limit`）
  - `GetCommit(ctx, projectID, sha)` → 详情（meta + `show --format= --name-status` 文件清单）
  - `FileHistory(ctx, projectID, path)` → 该路径的 commit 列表（`log --follow -- <path>`）
  - `CommitDiff(ctx, projectID, sha, format)` → 机器可读 diff：`numstat`（`{path, added, deleted}`）或完整 `patch`（`show --format= --patch`）
  - `RevertCommit(ctx, projectID, sha, message)` → 锁 + worktree（detach at main）→ `git show sha --format= --binary > patch` → `git apply -R --check`（预检失败 → ErrConflict 语义）→ `git apply -R` → `git add -A` → write-tree → commit-tree（message：`Revert "<原消息>"`）→ CAS update-ref → 清理；返回新 commit
  - 错误：ErrNotFound（sha 不存在）、ErrConflict（预检失败）
- API（SessionAuth，前缀 `/api/v1/projects/{id}/`）：
  - `GET commits?limit=&offset=` → `{commits:[...]}`
  - `GET commits/{sha}` → `{commit:{sha,message,author,date,files:[{status,path}]}}`
  - `GET files/{path}/history` → `{path, commits:[...]}`
  - `GET commits/{sha}/diff?format=numstat|patch` → `{sha, format, stats:[{path,added,deleted}], patch:"..."}`
  - `POST commits/{sha}/revert` `{message?}` → `{commit:{sha,message}}`（410 archived / 404 / 409 预检失败）
- 前端：docs-viewer 增加历史侧栏区？——MVP：项目详情页加「最近提交」列表（commit sha/消息/日期，点击展开 diff 摘要 numstat）；docs-viewer 文件视图加「历史」入口（FileHistory 列表）。revert 本阶段仅 API（前端按钮放详情页 commit 行：确认后调用）
- 文档：api.md / architecture.md / README / plans 索引

**不做（后续阶段）：** Agent Token（六）、搜索（七）、OpenAPI/导入导出（八）、分支/标签浏览、blame、diff 高亮 UI、冲突自动解决。

**验收标准（spec §27 阶段五）：**

1. API 写入（changesets）与网页写入（同一 API）出现在同一历史：commits 列表包含全部阶段四写入。
2. Revert 创建新 commit：revert 后 `rev-list --count` +1、原 commit 仍存在、文件内容回退。

## 1. 文件结构

```text
internal/project/
├── history.go        （新）
├── history_test.go   （新：列表/详情/文件历史/diff/revert 语义）
internal/httpapi/handlers/
├── history.go        （新）
└── history_test.go   （新：集成）
internal/server/router.go      （修改）
web/src/
├── lib/api/history.ts         （新 + 测试）
├── routes/project-detail.tsx  （修改：最近提交区）
└── routes/docs-viewer.tsx     （修改：文件历史面板）
doc/api.md / doc/architecture.md / README.md / doc/plans/README.md  （修改）
```

## 2. API 设计

```http
GET /api/v1/projects/{id}/commits?limit=20&offset=0
  → 200 {"commits":[{"sha":"<40>","message":"...","author":"AgentDocs","date":"2026-08-02T..."}]}

GET /api/v1/projects/{id}/commits/{sha}
  → 200 {"commit":{"sha","message","author","date",
         "files":[{"status":"A|M|D|R","path":"..."}]}}

GET /api/v1/projects/{id}/files/{path}/history
  → 200 {"path":"docs/a.md","commits":[...]}

GET /api/v1/projects/{id}/commits/{sha}/diff?format=numstat
  → 200 {"sha","format","stats":[{"path","added","deleted"}]}
  ?format=patch → {"sha","format","patch":"diff --git ..."}

POST /api/v1/projects/{id}/commits/{sha}/revert   body: {"message":"..."}
  → 200 {"commit":{"sha","message"}} · 404 commit_not_found · 409 revert_conflict · 410 project_archived
```

## 3. 决策

- **log 格式**：`%H%x1f%s%x1f%an%x1f%aI`（unit separator 分割，解析稳定）；sha 完整 40 位。
- **diff**：`git show <sha> --format= --numstat`（机器统计）与 `--patch`（完整 diff，`--no-color`、`--find-renames`）。
- **revert**：reverse-apply patch；预检失败返回 409（不自动解决冲突）；成功则与阶段四相同 CAS 提交路径。
- **历史一致性**：所有写入都经过 main 分支 → 列表天然统一（验收 1）。
- **limit 上限**：100（默认 20）。

## 4. 任务清单（严格 TDD）

- [x] **Task 1** `history_test.go`（RED：a) 多次 changeset 后 ListCommits 含全部（倒序）；b) GetCommit 详情含文件清单；c) FileHistory 只含该路径相关 commit；d) CommitDiff numstat/patch 内容正确；e) Revert 后 count+1、原 commit 仍在、内容回退、revert 本身可 revert（幂等语义）；f) 未知 sha → ErrNotFound；g) revert 预检失败 → ErrConflict）→ 实现 `history.go` → GREEN。
- [x] **Task 2** `history_test.go`（集成，RED：commits 列表/详情/diff/文件历史/revert 全链路 + 401/404/409/410）→ 实现 handlers + router → GREEN。
- [x] **Task 3** 前端 `lib/api/history.ts`（listCommits/getCommit/fileHistory/getDiff/revertCommit + 测试）→ GREEN。
- [x] **Task 4** 前端：project-detail 加「最近提交」（列表 + numstat 展开）；docs-viewer 文件视图加「历史」面板（FileHistory 列表）；revert 按钮（confirm + 调用 + 刷新）→ 测试 → GREEN。
- [x] **Task 5** 文档更新 + 勾选计划。
- [x] **Task 6** 端到端验收：全测试；构建重启；curl 冒烟（阶段四的写入出现在 commits 列表 = 验收 1；revert 后 count+1 且内容回退 = 验收 2）；前端冒烟；`git restore web/dist/index.html`。

## 5. 风险

- revert 冲突：预检 `git apply -R --check` 失败 → 409，不产生任何提交。
- 大 diff：patch 输出限 1 MiB（413）。
