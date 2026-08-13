# XWiki 阶段六：Agent Token Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 为 AI Agent 提供受控访问：Bearer Token（创建/撤销/列表）、scope（read|write）、项目限制、路径前缀限制、幂等键（相同请求不重复提交）、审计日志。**验收：Token 无法访问未授权项目；无法写入禁止目录；相同幂等请求不产生重复 commit。**

**Architecture:** 新迁移 `00003_tokens_audit.sql`（tokens/idempotency_keys/audit_logs 三表）；`internal/agent` 包（store + service：创建/校验/幂等/审计）；中间件 `AgentAuth`（Bearer 头）挂在文档/变更/历史端点旁（与 SessionAuth 二选一）；changesets handler 增加幂等键处理与审计写入。

**Tech Stack:** 无新依赖；Token 明文 = `ad_<32 hex>`，库中只存 SHA-256 哈希（沿用会话模式）。

---

## 0. 范围（本计划 = spec 阶段六）

**做：**

- 迁移 00003：`agent_tokens`（id/name/hash/scope/project_ids/path_prefixes/created_at/last_used_at/revoked_at）、`idempotency_keys`（key/project_id/request_hash/result_json/created_at，PK(key, project_id)）、`audit_logs`（id/actor_type/actor_id/project_id/action/path/detail/request_id/created_at）
- `internal/agent`：
  - `Store`：CreateToken（返回明文一次）、ListTokens、RevokeToken、GetByHash、幂等键读写、审计追加
  - `Service`：`Create(ctx, name, scope, projectIDs, pathPrefixes)` → 明文；`Authorize(ctx, rawToken, projectID, path, write bool)` → 校验（撤销/scope/项目/路径前缀）；`IdempotentApply` 包装 changesets（key 命中 → 返回已记录结果）
- 中间件 `AgentAuth(agentSvc)`：解析 `Authorization: Bearer <token>` → Authorize 项目无关校验 → 把 actor 注入 ctx（`request.Actor(ctx)`）
- handler 集成：
  - 变更写端点（changesets）接受 Bearer token（代替 session cookie）；写前按项目+路径前缀校验；带 Idempotency-Key 时查重
  - 读端点（tree/pages/commits/diff）接受 Bearer（scope=read 即可）
  - 所有 token 动作写 audit_logs
- Token 管理 API（session 登录后）：`POST/GET /api/v1/tokens`、`DELETE /api/v1/tokens/{id}`
- 前端：设置页（/settings/tokens）：token 列表 + 创建表单（name/scope/项目/路径前缀）+ 撤销 + 创建时一次性显示明文 → 简化：仅 API + 最小页面（列表 + 创建 + 复制明文 + 撤销）
- 文档：api.md / architecture.md / README / plans 索引

**不做（后续阶段）：** 搜索（七）、OpenAPI（八）、token 过期时间（MVP 手动撤销）、scope 细粒度（read/write 两档）、限流。

**验收标准（spec §27 阶段六）：**

1. Token 无法访问未授权项目（读 404/403、写 403）。
2. Token 无法写入禁止目录（路径前缀外 → 403）。
3. 相同幂等请求不产生重复 commit（同 key 重放 → 返回首次结果，rev-list 不变）。

## 1. 文件结构

```text
internal/store/sqlite/migrations/00003_agent.sql   （新）
internal/agent/
├── store.go / store_test.go
├── service.go / service_test.go
internal/httpapi/middleware/agent.go   （新：Bearer 解析）
internal/httpapi/request/request.go    （修改：Actor 存取）
internal/httpapi/handlers/
├── tokens.go / tokens_test.go         （新：token 管理）
├── changesets.go                      （修改：Bearer 认证 + 幂等 + 审计）
├── docs.go / history.go               （修改：Bearer 读认证）
internal/server/router.go / app.go     （修改：装配）
web/src/lib/api/tokens.ts + test
web/src/routes/tokens.tsx + test       （新：/settings/tokens）
web/src/app/router.tsx                 （修改）
doc/api.md / doc/architecture.md / README.md / doc/plans/README.md  （修改）
```

## 2. API 设计

```http
# Token 管理（session 登录）
POST /api/v1/tokens  {"name":"ci-bot","scope":"write","project_ids":["prj_..."],"path_prefixes":["docs/"]}
  → 201 {"token":{"id","name","scope","project_ids","path_prefixes","created_at"},"secret":"ad_<32hex>"}（secret 仅此一次）
GET /api/v1/tokens → 200 {"tokens":[...]}（无 secret）
DELETE /api/v1/tokens/{id} → 200 {"ok":true}

# Agent 访问（Bearer ad_...）
GET  /api/v1/projects/{id}/docs/tree|pages|home、commits、files/history、revision → scope read 足够
POST /api/v1/projects/{id}/changesets → scope write + 项目绑定 + 路径前缀校验 + 可选 Idempotency-Key
  Idempotency-Key: <任意字符串>（重复 → 返回首次结果，不新建 commit）
```

## 3. 决策

- Token 格式 `ad_` + 32 hex（128-bit）；库中存 SHA-256。
- scope: `read`（只读端点）| `write`（读 + 写）。
- project_ids：token 显式绑定；未绑定项目 → 403 agent_forbidden（读 404 语义保持 project_not_found 防探测？——验收要求"无法访问"，用 403 更明确；读取未授权项目返回 404 防枚举，写入返回 403）。
- path_prefixes：写入时 changeset 内**每个路径**必须以某前缀开头（含 new_path）；delete/move 同样。
- 幂等：`(key, project_id)` 唯一；命中时校验 request_hash（相同 payload）→ 返回记录结果；不同 payload 同 key → 409 idempotency_conflict。
- 审计：记录 actor_type（user|token）、动作（project.create/read/change/revert/token.create 等）、路径、request_id；token 端点自身不审计 token 内容。
- SessionAuth 与 AgentAuth 组合：读/写端点接受任一（OR 语义：两个中间件分别尝试，成功即放行）。

## 4. 任务清单（严格 TDD）

- [x] **Task 1** 迁移 00003 + agent store 测试（RED：CreateToken 返回明文且库中只有哈希；GetByHash 命中/撤销后失败；ListTokens；Revoke 幂等；幂等键 set/get 冲突；审计追加/列表）→ 实现 store.go → GREEN。
- [x] **Task 2** agent service 测试（RED：Authorize scope/项目/路径校验矩阵：read token 写 → 拒绝；未绑定项目 → 拒绝；前缀外路径 → 拒绝；撤销后 → 拒绝；空 project_ids 创建被拒；明文格式校验）→ 实现 service.go → GREEN。
- [x] **Task 3** 中间件 + handler 集成测试（RED：Bearer 读 tree 200；Bearer 写 changesets 200 + audit 行；未授权项目 403/404；前缀外 403；Idempotency-Key 重放 → 同 commit 且 rev-list 不变；不同 payload 同 key → 409；无凭据 → 401；Token 管理端点 session 才能访问）→ 实现 middleware/agent.go + handlers 修改 + router/app 装配 → GREEN。
- [x] **Task 4** 前端 tokens 页（列表/创建/明文一次性显示/撤销 + 测试）+ 路由 + API client → GREEN。
- [x] **Task 5** 文档更新 + 勾选计划。
- [x] **Task 6** 端到端验收：全测试；构建重启；curl 冒烟（创建 token → Bearer 读/写 → 越权 403 → 幂等重放 → 审计表查询）。

## 5. 风险

- 双认证中间件组合：OR 语义实现要小心（任一通过即放行，都失败才 401）。
- 幂等键存储膨胀：按 (key, project_id) 索引，MVP 不清理。
