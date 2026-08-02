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
