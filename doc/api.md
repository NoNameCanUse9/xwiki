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
