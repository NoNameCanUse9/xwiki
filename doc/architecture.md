# 架构说明（阶段一）

## 分层

HTTP 层（internal/httpapi）→ 服务层（internal/auth、internal/app）→ 存储层（internal/user、internal/store/sqlite）。

- 路由：internal/server/router.go（chi）
- 中间件链：RequestID → RequestLogger → Recoverer → CORS → 路由
- 认证：SessionAuth 中间件解析 HttpOnly Cookie（只存 SHA-256 哈希）
- 密码：Argon2id（PHC 格式），见 internal/auth/password.go
- 错误：统一信封 {error:{code,message,request_id}}，见 internal/httpapi/response

## 请求生命周期

1. RequestID 中间件生成/透传 request_id（响应头 X-Request-ID）
2. RequestLogger 输出一行 JSON 日志
3. 处理器解码请求体（限制大小、严格字段）
4. 服务层完成业务，写 SQLite（WAL）
5. response 包统一序列化

## 数据存储

- SQLite：data/agentdocs.db（WAL、外键、busy_timeout）
- 迁移：goose，SQL 文件内嵌于二进制（internal/store/sqlite/migrations）
- 表：users、sessions、schema_migrations

## 前端

- Vite + React + shadcn/ui，构建产物嵌入 Go 二进制（web/embed.go）
- 会话恢复：ProtectedRoute 挂载时调用 GET /api/v1/auth/me
- 开发模式：Vite 代理 /api → :8080，无 CORS 问题

## 安全基线（对应 spec §21）

- 密码哈希 Argon2id；会话 Token 只存哈希
- Cookie：HttpOnly + SameSite=Lax（生产加 Secure）
- 请求体大小限制；未知字段拒绝
- 不记录 Token、密码、会话内容（spec §25）
