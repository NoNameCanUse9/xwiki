# AgentDocs

面向人类与 AI Agent 的轻量 Git 文档管理系统。

> 当前进度：**阶段八（OpenAPI 与导入导出）已完成 —— MVP 全部八个阶段完成**。完整需求见 `doc/spec.md`，分阶段实施计划见 `doc/plans/`，全局验收见 `doc/api.md` 末尾核对表。

## 已实现功能

- 阶段一：骨架 —— 管理员 CLI、登录、会话持久化、错误信封、前端嵌入、Docker
- 阶段二：项目 —— 一项目一独立 Git 仓库（bare）、README 自动初始化、项目列表/详情/归档
- 阶段三：文档读取 —— 文档树（Git Tree 实时读取）、Markdown 渲染（goldmark）、项目首页、面包屑导航
- 阶段四：ChangeSet 写入 —— 项目锁、临时 worktree、create/update/delete/move、dry-run、原子 update-ref（409 冲突）
- 阶段五：历史与 Diff —— Commit 列表/详情、文件历史、机器可读 diff（numstat/patch）、Revert（新提交不删历史）
- 阶段六：Agent Token —— Bearer 认证、scope、项目/路径限制、幂等键、审计日志
- 阶段七：搜索 —— SQLite FTS5 全文搜索、写入后增量索引、reindex CLI
- 阶段八：OpenAPI 与导入导出 —— Scalar API 文档、ZIP 快照导入导出、Bundle 全仓导入导出、base64 附件
- 增强：Notion 式编辑器（Tiptap：工具栏/slash/浮动工具条/块操作/Cmd+K）、嵌入组件（代码高亮/KaTeX/Mermaid/admonition）、wiki 链接、附件面板、Git HTTP（clone/push）、_sidebar.md 菜单

## 快速开始

前置：Go 1.26+、Node 26+（构建前端）、Git。

### 开发模式

```bash
# 后端
./agentdocs serve   # 或 go run ./cmd/agentdocs serve
# 前端（开发服务器，/api 代理到 :8080）
cd web && npm run dev
```

### 首次使用

```bash
./agentdocs admin create -username admin -password secret123
```

浏览器打开 http://localhost:8080 登录。

### 构建与测试

```bash
cd web && npm install && npm run build && cd ..
go build -o agentdocs ./cmd/agentdocs
go test ./...
cd web && npm run test
```

### Docker

```bash
docker compose up -d --build
```

## 配置（环境变量）

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `AGENTDOCS_DATA_DIR` | `data` | 数据目录（SQLite + 仓库） |
| `AGENTDOCS_HTTP_ADDR` | `:8080` | HTTP 监听地址 |
| `AGENTDOCS_WEB_ORIGIN` | `http://localhost:5173` | 允许的 CORS 来源 |
| `AGENTDOCS_SESSION_TTL` | `720h` | 会话有效期 |
| `AGENTDOCS_MAX_BODY_BYTES` | `1048576` | 请求体上限 |
| `AGENTDOCS_COOKIE_SECURE` | `false` | 生产环境设为 true |

## 目录结构

见 `doc/architecture.md`。
