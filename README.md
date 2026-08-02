# AgentDocs

面向人类与 AI Agent 的轻量 Git 文档管理系统。

> 当前进度：阶段二（项目与 Git 仓库）已完成。完整需求见 `doc/spec.md`，分阶段实施计划见 `doc/plans/`。

## 已实现功能

- 阶段一：骨架 —— 管理员 CLI、登录、会话持久化、错误信封、前端嵌入、Docker
- 阶段二：项目 —— 一项目一独立 Git 仓库（bare）、README 自动初始化、项目列表/详情/归档

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
