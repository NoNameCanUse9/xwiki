# REST API（阶段一）

前缀：/api/v1。错误统一为 {error:{code,message,request_id}}。

## 健康检查

- GET /healthz → 200 {"status":"ok"}
- GET /readyz → 200 {"status":"ready"} / 503

## 认证

### POST /auth/login

请求：{"username":"...","password":"..."}

- 200 {"user":{...}}，Set-Cookie: agentdocs_session（HttpOnly）
- 401 invalid_credentials

### POST /auth/logout

清除会话（需 cookie）。→ 200 {"ok":true}

### GET /auth/me（需登录）

→ 200 {"user":{id,username,display_name,is_admin}}

### POST /auth/password（需登录）

请求：{"current_password":"...","new_password":"..."}

- 200 {"ok":true}；401 invalid_credentials；400 validation_failed

## 错误码（本阶段使用）

validation_failed / invalid_credentials / authentication_required / not_found / not_ready / internal_error

## curl 示例

```bash
curl -c /tmp/cj.txt -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' -d '{"username":"admin","password":"secret123"}'
curl -b /tmp/cj.txt http://localhost:8080/api/v1/auth/me
```
---

# 项目 API（阶段二）

全部项目端点需登录（未认证 → 401 authentication_required）。统一错误信封不变。

## POST /api/v1/projects

请求：{"name":"docs-site","description":"产品文档"}

- 201 {"project":{id,name,description,repo_dir,archived,created_at,updated_at}}
- 400 invalid_project_name（1-64 位小写字母/数字/单个连字符）或 validation_failed
- 409 project_name_conflict（同名项目已存在）

创建时自动初始化独立 bare 仓库（data/repos/<id>/repo）并写入 README 初始提交。

## GET /api/v1/projects

- 200 {"projects":[...]}（含已归档，按 created_at 倒序）

## GET /api/v1/projects/{id}

- 200 {"project":{...}}
- 404 project_not_found

## POST /api/v1/projects/{id}/archive

- 200 {"project":{...,"archived":true}}（幂等：重复归档保持原时间戳）
- 404 project_not_found

## 错误码（本阶段新增）

invalid_project_name / project_name_conflict / project_not_found

## curl 示例

curl -c /tmp/cj.txt -X POST http://localhost:8080/api/v1/auth/login -H 'Content-Type: application/json' -d '{"username":"admin","password":"secret123"}'
curl -b /tmp/cj.txt -X POST http://localhost:8080/api/v1/projects -H 'Content-Type: application/json' -d '{"name":"docs-site"}'
curl -b /tmp/cj.txt http://localhost:8080/api/v1/projects

---

# 文档读取 API（阶段三）

内容直接从项目的 Git 仓库读取，不使用任何页面/文件夹数据库表。需登录。

## GET /api/v1/projects/{id}/docs/tree?path=dir/

- path 省略或空 = 仓库根目录；`path=docs` = docs 目录
- 200 `{"path":"docs","tree":[{"name":"guide.md","type":"blob","path":"docs/guide.md"},...]}`（type: blob|tree）
- 400 invalid_doc_path（路径穿越/绝对路径）· 404 doc_not_found / project_not_found

## GET /api/v1/projects/{id}/docs/pages/{path}

- 读取任意文件内容（从 Git blob）
- `?format=html` → goldmark 渲染（GFM，默认转义原始 HTML）
- 200 `{"path":"docs/guide.md","format":"raw","content":"..."}` 或 format=html
- 400 invalid_doc_path / invalid_format · 404 doc_not_found · 413 doc_too_large（>2 MiB）

## GET /api/v1/projects/{id}/docs/home

- 项目首页：README.md 优先，docs/README.md 兜底
- 200 `{"path":"README.md","format":"html","content":"<article>..."}` · 404 doc_not_found

## 错误码（本阶段新增）

invalid_doc_path / invalid_format / doc_not_found / doc_too_large

---

# ChangeSet 写入 API（阶段四）

一次请求 = 一个原子 commit（多文件同批）。需登录。

## GET /api/v1/projects/{id}/revision

- 200 `{"revision":"<40-hex>"}`（当前 HEAD，作为写入 base）

## POST /api/v1/projects/{id}/changesets

请求：`{"base_revision":"<40-hex>","message":"update docs","changes":[
  {"op":"create|update","path":"docs/a.md","content":"# A\n"},
  {"op":"delete","path":"old.md"},
  {"op":"move","path":"a.md","new_path":"b.md"}]}`

- 200 `{"commit":"<40-hex>","revision":"<40-hex>","preview":null}`
- `?dry_run=true` → 200 `{"commit":"","revision":"<当前>","preview":{"tree":"<40-hex>","changes":[...]}}`（不写任何 ref）
- 400 invalid_changeset / invalid_doc_path · 404 project_not_found · 409 revision_conflict（base 过期，需重读）· 410 project_archived

语义：

- 项目级锁串行化写入；临时 worktree 应用变更；write-tree → commit-tree → `update-ref refs/heads/main <new> <old>`（CAS，old 不匹配即 409）
- 任意失败不产生 commit、不留 worktree
- 单文件 ≤ 2 MiB；单请求 ≤ 100 个变更
- 写入后立即可读（无索引）

## 错误码（本阶段新增）

invalid_changeset / revision_conflict / project_archived

---

# 历史与 Diff API（阶段五）

需登录。历史包含所有写入（API 与网页同一提交流）。

## GET /api/v1/projects/{id}/commits?limit=20&offset=0

- 200 `{"commits":[{"sha","message","author","date"}]}`（倒序；limit ≤ 100）

## GET /api/v1/projects/{id}/commits/{sha}

- 200 `{"commit":{"sha","message","author","date","files":[{"status":"A|M|D|R","path"}]}}`
- 404 commit_not_found

## GET /api/v1/projects/{id}/files/history/{path}

- 200 `{"path","commits":[...]}`（该路径的全部提交，含重命名追踪）

## GET /api/v1/projects/{id}/commits/{sha}/diff?format=numstat|patch

- numstat：200 `{"sha","format","stats":[{"path","added","deleted"}]}`
- patch：200 `{"sha","format","patch":"diff --git ..."}`（≤ 1 MiB）
- 404 commit_not_found

## POST /api/v1/projects/{id}/commits/{sha}/revert

请求：`{"message":"可选"}` → 200 `{"commit":{"sha","message"}}`

- 创建新提交回滚目标提交（原历史保留）；冲突 → 409 revert_conflict
- 404 commit_not_found · 410 project_archived

## 错误码（本阶段新增）

commit_not_found / revert_conflict

---

# Agent Token API（阶段六）

Token 让 AI Agent 以 Bearer 认证访问。库中只存 SHA-256 哈希，明文仅创建时返回一次。

## Token 管理（session 登录）

### POST /api/v1/tokens

请求：`{"name":"ci-bot","scope":"write","project_ids":["prj_..."],"path_prefixes":["docs"]}`

- 201 `{"token":{...},"secret":"ad_<32hex>"}`（secret 仅此一次）
- 400 invalid_token_input（scope 非 read|write / 无项目绑定 / 前缀带斜杠）

### GET /api/v1/tokens → 200 `{"tokens":[...]}`

### DELETE /api/v1/tokens/{id} → 200 `{"ok":true}`（幂等撤销）

## Agent 访问（Authorization: Bearer ad_...）

- 读端点（tree/pages/home/commits/diff/history/revision）：token 需绑定该项目（否则 403 agent_forbidden）
- `POST /api/v1/projects/{id}/changesets`：
  - scope 必须 write；每个写入路径（含 new_path）必须以某 path_prefix 开头（否则 403）
  - 可选 `Idempotency-Key` 头：同 key 同 body 重放 → 返回首次结果（不新建 commit）；同 key 不同 body → 409 idempotency_conflict
- 全部 token 操作写入 audit_logs（`GET /api/v1/projects/{id}/audit` 可查，session 登录）

## 错误码（本阶段新增）

invalid_token / invalid_token_input / agent_forbidden / idempotency_conflict / token_not_found
