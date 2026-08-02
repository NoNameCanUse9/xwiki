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

## 项目与 Git 仓库（阶段二）

- 分层：internal/project（store → repo → service），HTTP 处理器 internal/httpapi/handlers/projects.go
- 一项目一仓库：data/repos/<projectID>/repo（bare），SQLite projects 表只存元数据与相对路径
- 初始化：bare init + plumbing（hash-object → mktree → commit-tree → update-ref refs/heads/main）写 README 初始提交，不引入 worktree（阶段四使用）
- 归档：软归档（archived_at 时间戳，COALESCE 幂等），不删除仓库
- 表：projects（id/name 唯一/description/repo_dir/archived_at/created_at/updated_at）
- 仓库命令全部带 --git-dir 绝对路径 + 固定 author/committer 身份，不依赖 cwd

## 文档读取（阶段三）

- 无页面表：文档树与内容全部实时从项目 bare 仓库读取（ls-tree / cat-file）
- internal/project/repo.go 只读扩展：DefaultBranch、ListTree（单层）、ReadBlob、ResolveTree
- 渲染：goldmark（GFM），默认转义 raw HTML；format=raw 返回原文
- 路径安全：服务端 path.Clean 校验（拒绝穿越/绝对路径），Git 侧 --git-dir 绝对路径
- blob 上限 2 MiB（413）

## ChangeSet 写入（阶段四）

- 写路径：临时 worktree（--detach）→ 应用变更 → add -A → write-tree → commit-tree → update-ref（CAS）
- 原子性：update-ref 带 old 值 = compare-and-swap；冲突返回 409 revision_conflict
- 并发：进程内 per-project 互斥锁；跨实例由 CAS 兜底
- 锁粒度：internal/project/changeset.go（projectLocks sync.Map）
- revision = HEAD commit sha；dry-run 只算树不写 ref

## 历史与 Diff（阶段五）

- 只读：log（%H%x1f 分隔）/ show --name-status / log --follow / show --numstat|--patch
- Revert：worktree + reverse-apply（git apply -R，先 --check 预检），通过后走阶段四 CAS 提交管线；原提交永不改写
- patch 输出用原始字节读取（TrimSpace 会破坏 diff 语义）

## Agent Token 与审计（阶段六）

- token 明文 ad_<32hex>，库中只存 SHA-256（同会话模式）；scope read|write
- 授权链：AgentAuth 中间件（Bearer）→ 项目绑定 → 写入路径前缀逐条校验
- 双认证 OR：AgentAuth 处理 Bearer，SessionAuth 兜底 cookie；SessionAuth 跳过已认证请求
- 幂等：(key, project_id) 唯一 + request hash 比对；命中返回首次结果
- 审计：audit_logs 表记录 actor（user|token）、动作、路径、request_id

## 搜索（阶段七）

- FTS5 虚拟表 + 外挂内容表 doc_index_state（project_id, path, blob_sha, content）+ 触发器同步
- 增量索引：写入/回滚后遍历 Git 树，按 blob sha 对比 upsert/delete；二进制与超大文件跳过
- 查询：unicode61 分词 + 前缀 AND；snippet() 输出转义高亮标记
- CLI：agentdocs reindex（全量重建）


## OpenAPI 与导入导出（阶段八）

- openapi.json：静态生成（internal/httpapi/openapi.go），覆盖全部端点 + bearerAuth/sessionCookie
- 前端 /api-docs：@scalar/api-reference（npm 自托管，路由懒加载）
- ZIP：archive/zip 流式打包树快照；导入解码 base64 走统一 changeset（单次提交）
- Bundle：bundle create/fetch（fetch 建立 refs，unbundle 只写对象）；导入创建新项目保留完整历史
- 附件：changeset encoding=base64（≤ 5 MiB）；读取 format=base64


## 用户管理（补充）

- users 表 disabled_at 列（迁移 00005）；登录链路校验禁用状态（403 account_disabled）
- 管理端点 /api/v1/users 仅 admin（AdminOnly 中间件）
- 保护：不能禁用自己、不能禁用 admin 账号；禁用只影响登录（数据与历史保留）

## 项目恢复（补充）

- unarchive 清空 archived_at（幂等）；恢复后写操作立即放行


## Git HTTP 与附件（OtterWiki 对齐）

- Git HTTP：Go handler 代理 git http-backend（CGI env + PATH_INFO=/repo.git/<sub> + GIT_PROJECT_ROOT=repos/<id>）；Basic/Bearer token 认证；Content-Length 在 WriteHeader 前设置（smart HTTP 需要固定长度）
- 附件：attachments/ 目录经 changeset base64 写入；下载端点 mime 映射
- 侧栏：_sidebar.md 解析（- [label](path)）渲染树顶菜单
