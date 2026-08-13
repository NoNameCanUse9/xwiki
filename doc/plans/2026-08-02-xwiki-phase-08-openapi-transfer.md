# XWiki 阶段八：OpenAPI 与导入导出 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 开放 API 文档（OpenAPI 3.0 + Scalar 预览）与项目导入导出（ZIP 快照、Git Bundle 全仓、图片/附件 base64 支持）。

**Architecture:** 静态 OpenAPI 3.0 spec（`internal/httpapi/openapi.go` 生成 `GET /api/openapi.json`）；前端 `/api-docs` 页嵌入 **@scalar/api-reference**（npm 自托管，不依赖运行时 CDN）；导出：`internal/project/transfer.go`（ZIP：walk 树打包文本/二进制快照；bundle：`git bundle create` 到临时文件后流式输出）；导入：ZIP → 批量 changeset（单 commit）；bundle → 新项目 bare repo + `git bundle unbundle` + store 记录；附件：changeset `encoding:"base64"` + pages `?format=base64` 读取。

**Tech Stack:** `archive/zip`、`encoding/base64`（标准库）；`@scalar/api-reference`（npm）。

---

## 0. 范围（本计划 = spec 阶段八）

**做：**

- `GET /api/openapi.json`：OpenAPI 3.0 spec（静态生成：servers、security（session cookie + bearer）、全部端点摘要；描述中文）→ 内容校验（JSON schema 形状 + 端点数断言）
- 前端 `/api-docs` 页：@scalar/api-reference 组件（props：url=/api/openapi.json）；设置页链接
- ZIP 导出：`GET /api/v1/projects/{id}/export.zip` → 项目工作树快照（README.md、docs/...，含二进制文件原字节）；流式；需认证
- ZIP 导入：`POST /api/v1/projects/{id}/import`（JSON `{base_revision, files:[{path, content(base64)}]}`）→ 批量 create/update changeset（单 commit）；超 2 MiB 单文件拒绝
- Bundle 导出：`GET /api/v1/projects/{id}/export.bundle` → git bundle（全仓历史）
- Bundle 导入：`POST /api/v1/import/bundle?name=<project>`（multipart `file`）→ `git bundle unbundle` → 新 bare repo + 校验 HEAD → store 记录 → 201 项目
- 附件：changeset Change 增加 `encoding:"base64"`（content 为 base64）；限制 5 MiB；pages `?format=base64` → `{path, format:"base64", encoding:"base64", content:"..."}`
- 文档：api.md / architecture.md / README / plans 索引；全局验收 §29 清单核对（附在计划尾）

**不做：** 大文件流式上传、导出选择路径、bundle 导入覆盖已有项目、增量同步。

**验收（spec §27 阶段八）：**

1. `GET /api/openapi.json` 返回合法 OpenAPI 3.0 文档；Scalar 页面渲染。
2. ZIP 导出 → 导入另一项目 → 文档内容一致（diff 为空）。
3. Bundle 导出 → 导入新项目 → commit 历史完整（rev-list 数量一致）。

## 1. 文件结构

```text
internal/httpapi/
├── openapi.go + openapi_test.go     （新：静态 spec 生成）
├── handlers/transfer.go + 测试      （新：export.zip/export.bundle/import/import/bundle）
internal/project/
├── transfer.go + transfer_test.go   （新：zipExport/zipImport/bundleExport/bundleImport）
internal/httpapi/handlers/changesets.go  （修改：encoding base64）
internal/httpapi/handlers/docs.go        （修改：?format=base64）
internal/server/router.go / app.go       （修改：装配）
web/src/lib/api/transfer.ts + test
web/src/routes/api-docs.tsx + test       （新：/api-docs）
web/src/app/router.tsx + settings 链接   （修改）
web/package.json（@scalar/api-reference）
doc/api.md / doc/architecture.md / README.md / doc/plans/README.md  （修改）
```

## 2. API 设计

```http
GET /api/openapi.json → 200 OpenAPI 3.0 JSON（无需认证）

GET /api/v1/projects/{id}/export.zip → 200 application/zip（流式）
GET /api/v1/projects/{id}/export.bundle → 200 application/octet-stream（流式）

POST /api/v1/projects/{id}/import
  req: {"base_revision":"<40>","message":"import zip","files":[{"path":"docs/a.md","content":"<base64>"}]}
  → 200 {"commit":{...},"revision":"...","imported":N}

POST /api/v1/import/bundle?name=<name>  （multipart/form-data: file）
  → 201 {"project":{...},"commits":N}

GET /api/v1/projects/{id}/docs/pages/{path}?format=base64
  → 200 {"path","format":"base64","encoding":"base64","content":"<base64>"}
```

## 3. 决策

- **ZIP 导出**：`archive/zip` 流式（先 walk 树收集 (path, blob)，再写 zip）；路径安全（拒绝穿越）。
- **ZIP 导入**：解码 base64 → 文本/二进制统一走 changeset（encoding base64 支持二进制）；单文件 ≤ 5 MiB；同 commit。
- **Bundle 导出**：`git bundle create <tmpfile> --all` → 流式读文件 → 删临时。
- **Bundle 导入**：multipart 解析（`r.ParseMultipartForm(64<<20)`）→ 存临时 → `git bundle verify` → `git bundle unbundle` 到 `git init --bare` 临时仓库 → 校验 `rev-parse HEAD` → 移到 repos/<id>/repo.git → store 记录 → 删除临时。名称校验用 ValidateName。
- **附件读取**：blob 二进制检测（`\x00`）→ base64 编码；limit 5 MiB（文本 2 MiB 不变）。
- **OpenAPI spec**：手写结构体（map）避免反射；覆盖全部端点路径 + securitySchemes（sessionCookie + bearerAuth）。

## 4. 任务清单（严格 TDD）

- [x] **Task 1** openapi.go 测试（RED：spec 含 info/openapi 3.0.3、paths 覆盖 ≥ 20 个端点、securitySchemes 含 bearerAuth；JSON 可解析）→ 实现 → GREEN。
- [x] **Task 2** transfer.go 测试（RED：zipExport 内容与树一致（含二进制）；zipImport 单 commit + 内容正确；bundleExport 后 rev-list 一致；bundleImport 新项目 repo HEAD 相同、store 记录、名称校验、无效 bundle 拒绝）→ 实现 → GREEN。
- [x] **Task 3** handlers 测试（RED：export.zip 200 + zip 签名；export.bundle 200；import 200 + revision 前进；bundle import 201；format=base64 读取；401/403/404 矩阵）→ 实现 transfer handlers + router/app 装配 + changesets/docs 的 base64 支持 → GREEN。
- [x] **Task 4** 前端：@scalar/api-reference 安装 + api-docs 页 + 路由 + 测试（渲染标记）；transfer API client + 测试 → GREEN。
- [x] **Task 5** 文档更新 + 勾选 + §29 全局验收清单核对表。
- [x] **Task 6** 端到端验收：全测试；构建重启；curl 冒烟（openapi.json → zip 导出 → 导入新项目 → diff 空；bundle 导出 → 导入 → rev-list 一致；base64 附件读写）；前端 /api-docs 冒烟；`git restore web/dist/index.html`。

## 5. 风险

- Scalar 包体积（~1MB js）——仅 api-docs 路由 chunk 懒加载。
- multipart 大小限制：bundle 上限 256 MiB。
- bundle unbundle 需要 `git bundle verify` 预检失败即拒绝。
