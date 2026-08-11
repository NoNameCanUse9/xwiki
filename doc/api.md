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

请求：`{"name":"ci-bot","scope":"write","project_ids":["prj_..."]}`

- 201 `{"token":{...},"secret":"ad_<32hex>"}`（secret 仅此一次）
- 400 invalid_token_input（scope 非 read|write / 无项目绑定）

### GET /api/v1/tokens → 200 `{"tokens":[...]}`

### DELETE /api/v1/tokens/{id} → 200 `{"ok":true}`（幂等撤销）

## Agent 访问（Authorization: Bearer ad_...）

- 读端点（tree/pages/home/commits/diff/history/revision）：token 需绑定该项目（否则 403 agent_forbidden）
- `POST /api/v1/projects/{id}/changesets`：
  - scope 必须 write；Token 可在绑定的项目内写入
  - 可选 `Idempotency-Key` 头：同 key 同 body 重放 → 返回首次结果（不新建 commit）；同 key 不同 body → 409 idempotency_conflict
- 全部 token 操作写入 audit_logs（`GET /api/v1/projects/{id}/audit` 可查，session 登录）

## 错误码（本阶段新增）

invalid_token / invalid_token_input / agent_forbidden / idempotency_conflict / token_not_found

---

# 搜索 API（阶段七）

项目内全文搜索（SQLite FTS5）。写入（changeset/revert）后自动增量索引。

## GET /api/v1/projects/{id}/search?q=pineapple&limit=10

- 200 `{"query":"pineapple","results":[{"path":"docs/a.md","snippet":"... pineapple ..."}]}`
- 400 invalid_query（空/超长）· 403 agent_forbidden（token 未绑定该项目）· 404 project_not_found
- 查询：词级 AND + 前缀匹配；空结果返回 `results: []`

## Reindex CLI

```bash
agentdocs reindex                 # 全量重建全部项目
agentdocs reindex --project <id>  # 单项目
```


---

# OpenAPI 与导入导出 API（阶段八）

## OpenAPI

- `GET /api/openapi.json` → OpenAPI 3.0.3 文档（无需认证）
- 前端 `/api-docs` 页面：Scalar API Reference（构建时打包，懒加载）

## 导入导出

```http
GET  /api/v1/projects/{id}/export.zip     → application/zip（项目工作树快照，含二进制）
GET  /api/v1/projects/{id}/export.bundle  → bundle 文件（全仓历史）
POST /api/v1/projects/{id}/import
  req: {"base_revision":"<40>","message":"...","files":[{"path":"docs/a.md","content":"<base64>"}]}
  → 200 {"commit","revision","imported":N}（单次提交原子导入；单文件 ≤ 5 MiB）
POST /api/v1/import/bundle?name=<name>    （multipart file）
  → 201 {"project":{...},"commits":N}（完整历史保留）
```

## 附件（图片/二进制）

- 写入：changeset 的 change 增加 `"encoding":"base64"`，content 为 base64（≤ 5 MiB）
- 读取：`GET .../docs/pages/{path}?format=base64` → `{"path","format":"base64","encoding":"base64","content":"..."}`
- 文本读取（raw/html）上限仍为 2 MiB

## 错误码（本阶段新增）

invalid_import / invalid_upload / bundle_too_large

---

# 全局验收核对（spec §29，全部阶段完成后）

| # | 验收项 | 状态 | 依据 |
|---|--------|------|------|
| 1 | 两个项目两个独立仓库 | ✅ | 阶段二隔离测试 + E2E |
| 2 | 网页修改创建真实提交 | ✅ | 阶段四 changesets（前端编辑器走同一 API） |
| 3 | Agent API 修改创建真实提交 | ✅ | 阶段六 Bearer 写 + E2E |
| 4 | 网页与 API 同一 ChangeSet 服务 | ✅ | 单一 internal/project.ApplyChangeset |
| 5 | 多文件单次提交 | ✅ | 阶段四多文件单提交测试 |
| 6 | 失败不产生部分提交 | ✅ | worktree + CAS；失败清理测试 |
| 7 | stale revision → 409 | ✅ | 阶段四集成测试 |
| 8 | 不静默覆盖最新版 | ✅ | base revision 校验 + 409 |
| 9 | 幂等键不重复提交 | ✅ | 阶段六 idempotency E2E |
| 10 | 文件列表来自 Diff | ✅ | 阶段五 show --name-status |
| 11 | 文件历史支持重命名追踪 | ✅ | 阶段五 log --follow |
| 12 | Revert 新提交不删历史 | ✅ | 阶段五验收 |
| 13 | 归档后拒绝写入 | ✅ | 410 project_archived |
| 14 | Token 限制项目与目录 | ✅ | 阶段六 403 矩阵 |
| 15 | 数据库不存 Markdown 正文 | ✅ | 正文只在仓库；DB 只有索引/审计/元数据 |
| 16 | 搜索索引可完整重建 | ✅ | agentdocs reindex + 幂等测试 |
| 17 | Markdown 无明显 XSS | ✅ | goldmark 转义 + 前端 sanitize |
| 18 | 路径无法逃逸仓库 | ✅ | validateDocPath + 穿越测试 |
| 19 | 完整 Bundle 导出 | ✅ | 阶段八 export.bundle + 历史保留测试 |
| 20 | Docker Compose 直接启动 | ✅ | 阶段一验证（含运行时） |


---

# 用户管理 API（补充）

仅管理员（session cookie + is_admin）可访问；普通成员访问 → 403 admin_required。

## POST /api/v1/users

请求：`{"username":"alice","password":"password123","display_name":"Alice","is_admin":false}`

- 201 `{"user":{id,username,display_name,is_admin,disabled,created_at}}`
- 400 invalid_username / invalid_password · 409 username_conflict

## GET /api/v1/users

- 200 `{"users":[...]}`

## POST /api/v1/users/{id}/disable | /enable

- 200 `{"user":{...,"disabled":true|false}}`（幂等）
- 400 cannot_disable_self / cannot_disable_admin · 404 user_not_found

## 登录与禁用

- 禁用账号登录 → 403 account_disabled（新增错误码）
- 禁用不删除数据；重新启用后原会话失效、需重新登录

## 项目恢复（补充）

- `POST /api/v1/projects/{id}/unarchive` → 200 `{"project":{...,"archived":false}}`（幂等；恢复后即可写入）
- 404 project_not_found

---

# Git HTTP 与附件 API（OtterWiki 对齐）

## Git 智能 HTTP（实验性）

- URL：`http://<host>:8080/git/<projectId>`（标准 smart HTTP 协议）
- 认证：Basic（任意用户名，password = Agent Token 明文）或 session cookie（push 需 admin）
- 权限：read scope 可 clone/pull；write scope 可 push；归档项目拒绝 push（410）
- 用法：任意版本控制客户端 clone/pull/push 直连；push 后的提交立即出现在网页历史与搜索中（同一仓库）
- 示例：`git clone http://x:<token>@localhost:8080/git/<projectId>`

## 附件

- `GET /api/v1/projects/{id}/attachments/{path}` → 原始字节流（Content-Type 按扩展名映射）
- 前端：docs-viewer 页面底部附件面板（上传/列表/下载/删除，≤ 5 MiB，存于 `attachments/` 目录）
- 上传通过 changeset（encoding base64）写入 Git，与其他文档同一历史

## 自定义侧栏

- 仓库根放 `_sidebar.md`：每行 `- [标签](路径)` → 显示为侧栏顶部菜单
- `_sidebar.md` 自身不出现在文档树中
