# OtterWiki 对齐（附件 UI + Git HTTP + 自定义侧栏）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 对齐 OtterWiki 的三项基础能力：页面附件（前端上传/列表/下载）、实验性 Git HTTP server（clone/pull/push 直连仓库）、可定制侧栏（`_sidebar.md` 菜单）。

**Architecture:** 附件 = 前端文件选择 → base64 changeset 写入 `attachments/` 目录 + 新增二进制下载端点；Git HTTP = Go handler 代理 `git http-backend`（CGI 环境变量 + 认证：Basic Auth 接受 Agent Token，session cookie 亦可），挂 `/git/{projectID}/`；侧栏 = docs-viewer 树顶部检测 `_sidebar.md`（存在则渲染为菜单）。

**Tech Stack:** 无新依赖（git http-backend 随 git 分发；附件复用 base64 现有管线）。

---

## 0. 范围

**做：**

- **附件**：
  - 端点：`GET /api/v1/projects/{id}/attachments/{path:*}` 原始字节流（Content-Type 按扩展名映射）
  - 前端：docs-viewer 附件面板（上传按钮 → FileReader base64 → create changeset 写入 `attachments/` + 列表 + 下载链接 + 删除）
- **Git HTTP server**（实验性，对齐 OtterWiki）：
  - `GET/POST /git/{projectID}/...`（info/refs、git-upload-pack、git-receive-pack）代理 `git http-backend`
  - 认证：Basic（password=Agent Token；scope write 允许 push，read 仅 pull）或 session cookie
  - 安全：GIT_PROJECT_ROOT=repos/、GIT_HTTP_EXPORT_ALL=1；projectID 映射 repos/<id>/repo.git；归档项目拒绝 push
  - 验证：clone → 修改 → push → 网页/API 同一历史立即可读
- **自定义侧栏**：树根 `_sidebar.md` → 顶部渲染链接菜单（`- [label](path)` 解析）；`_sidebar.md` 不显示在树中
- 文档：api.md（git http + attachments）、development.md、README 功能清单

**不做：** 匿名访问、分支保护策略、附件独立版本展示、注册流程（保持 admin 创建）。

**验收：**

1. 前端上传附件 → 附件区显示 → 下载字节一致。
2. `git clone`（token）→ 修改 → push → API 立即可读；无 token 401；read token push 拒绝。
3. 项目根有 `_sidebar.md` → 树顶部显示自定义菜单，点击跳转正确。

## 1. 文件结构

```text
internal/httpapi/handlers/
├── attachments.go + attachments_test.go   （新：二进制下载端点）
├── githttp.go + githttp_test.go           （新：http-backend 代理）
internal/server/router.go                  （修改：/git + attachments 路由）
web/src/
├── components/editor/attachments.tsx + attachments.test.tsx（新：附件面板）
├── lib/api/attachments.ts + attachments.test.ts             （新）
├── routes/docs-viewer.tsx                 （修改：附件面板 + _sidebar 渲染）
├── routes/docs-viewer.test.tsx            （修改）
doc/api.md / doc/development.md / README.md / doc/plans/README.md  （修改）
```

## 2. 任务清单（严格 TDD）

### Task 1: 附件读取端点（原始字节流）

**Files:**
- Create: `internal/httpapi/handlers/attachments.go`
- Create: `internal/httpapi/handlers/attachments_test.go`
- Modify: `internal/server/router.go`

- [x] **Step 1: 写失败测试**（集成测试：写二进制附件 → 下载断言字节/Content-Type；穿越 400；未认证 401）

```go
// internal/server/attachments_test.go
func TestAttachmentDownload(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "attach-site")
	base := getRevision(t, h, cookie, projectID)
	png := []byte{0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01}
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"attach",
		  "changes":[{"op":"create","path":"attachments/logo.png",
		    "content":"%s","encoding":"base64"}]}`,
			base, base64.StdEncoding.EncodeToString(png)))
	if rec.Code != http.StatusOK { t.Fatalf("write: %d", rec.Code) }
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/attachments/attachments/logo.png", cookie, "")
	if rec.Code != http.StatusOK { t.Fatalf("download: %d", rec.Code) }
	if !bytes.Equal(rec.Body.Bytes(), png) { t.Fatal("bytes differ") }
	if ct := rec.Header().Get("Content-Type"); !strings.HasPrefix(ct, "image/png") {
		t.Fatalf("content-type: %s", ct)
	}
	// traversal -> 400; unauth -> 401
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/attachments/../README.md", cookie, "")
	if rec.Code != http.StatusBadRequest { t.Fatalf("traversal: %d", rec.Code) }
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/attachments/attachments/logo.png", "", "")
	if rec.Code != http.StatusUnauthorized { t.Fatalf("unauth: %d", rec.Code) }
}
```

- [x] **Step 2: 运行确认失败**（`go test ./internal/server/ -run TestAttachmentDownload` → FAIL：404 无路由）

- [x] **Step 3: 实现**

```go
// internal/httpapi/handlers/attachments.go
package handlers

import (
	"errors"
	"log/slog"
	"mime"
	"net/http"
	"path/filepath"
	"strings"

	"xwiki/internal/agent"
	"xwiki/internal/config"
	"xwiki/internal/httpapi/middleware"
	"xwiki/internal/httpapi/request"
	"xwiki/internal/httpapi/response"
	"xwiki/internal/project"
)

// AttachmentHandler serves raw binary downloads.
type AttachmentHandler struct {
	cfg      *config.Config
	svc      *project.Service
	agentSvc *agent.Service
	log      *slog.Logger
}

func NewAttachmentHandler(cfg *config.Config, svc *project.Service, agentSvc *agent.Service, log *slog.Logger) *AttachmentHandler {
	return &AttachmentHandler{cfg: cfg, svc: svc, agentSvc: agentSvc, log: log}
}

// Download handles GET /api/v1/projects/{id}/attachments/{path:*}.
func (h *AttachmentHandler) Download(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	filePath := request.PathParam(r, "*")
	if !validateDocPath(filePath) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_doc_path", "invalid attachment path")
		return
	}
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
	repo, err := h.svc.OpenRepo(r.Context(), projectID)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
		return
	}
	branch, err := repo.DefaultBranch(r.Context())
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not resolve branch")
		return
	}
	blob, err := repo.ReadBlob(r.Context(), branch, filePath)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "attachment_not_found", "attachment not found")
		return
	}
	ctype := mime.TypeByExtension(filepath.Ext(filePath))
	if ctype == "" {
		ctype = "application/octet-stream"
	}
	w.Header().Set("Content-Type", ctype)
	w.Header().Set("Content-Disposition", `inline; filename="`+filepath.Base(filePath)+`"`)
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(blob)
}

var _ = errors.Is
var _ = strings.TrimSpace
```

路由（router.go docs 组内）：

```go
r.Get("/{id}/attachments/{path:*}", ah.Download)
```

- [x] **Step 4: 运行确认通过** → **Step 5: 提交**（`feat(attachments): binary download endpoint`）

### Task 2: 附件前端面板

**Files:**
- Create: `web/src/lib/api/attachments.ts` + test
- Create: `web/src/components/editor/attachments.tsx` + test
- Modify: `web/src/routes/docs-viewer.tsx`

- [x] **Step 1: 失败测试**

```tsx
// attachments.test.tsx
it("uploads a file via base64 changeset", async () => {
  vi.mock("@/lib/api/changesets", () => ({ getRevision: vi.fn().mockResolvedValue({ revision: "r1" }), submitChangeset: vi.fn() }));
  vi.mock("@/lib/api/docs", () => ({ getTree: vi.fn().mockResolvedValue({ path: "attachments", tree: [] }) }));
  const file = new File(["hello"], "note.txt", { type: "text/plain" });
  render(<AttachmentsPanel projectId="prj_1" currentPath="docs/a.md" />);
  await user.upload(screen.getByLabelText("上传附件"), file);
  await waitFor(() =>
    expect(submitChangeset).toHaveBeenCalledWith("prj_1", expect.objectContaining({
      changes: [{ op: "create", path: "attachments/note.txt", content: expect.any(String), encoding: "base64" }],
    })),
  );
});
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**

```tsx
// lib/api/attachments.ts
export function attachmentUrl(projectId: string, path: string) {
  return `/api/v1/projects/${encodeURIComponent(projectId)}/attachments/${path}`;
}
// components/editor/attachments.tsx
// props: projectId; useQuery getTree(projectId, "attachments") 过滤 blob；
// 上传：FileReader.readAsDataURL → 去前缀 → submitChangeset(create, `attachments/${name}`, base64, encoding)
// 列表：文件名 + 下载 <a href={attachmentUrl}> + 删除按钮（delete changeset）
// 大小显示：base64 长度 * 3/4
```

- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(attachments): frontend panel`）

### Task 3: Git HTTP server

**Files:**
- Create: `internal/httpapi/handlers/githttp.go` + test
- Modify: `internal/server/router.go`

- [x] **Step 1: 失败测试**（认证矩阵 + 路径映射）

```go
// internal/httpapi/handlers/githttp_test.go
func TestGitHTTPAuthMatrix(t *testing.T) {
	// read token: info/refs -> 200; receive-pack -> 403
	// write token: receive-pack -> 200(代理层放行，实际由 http-backend 决定)
	// 无凭据 -> 401; 未知项目 -> 404; 归档项目 receive-pack -> 410
}
func TestGitHTTPPathMapping(t *testing.T) {
	// /git/prj_1/info/refs?service=git-upload-pack
	// -> repoPath=repos/prj_1/repo.git, subPath=/repo.git/info/refs, service=git-upload-pack
}
```

- [x] **Step 2: 确认失败** → **Step 3: 实现**

```go
// internal/httpapi/handlers/githttp.go（核心逻辑）
package handlers

// GitHTTPHandler proxies git http-backend for smart HTTP clone/pull/push.
type GitHTTPHandler struct {
	svc      *project.Service
	agentSvc *agent.Service
	reposRoot string
	log      *slog.Logger
}

// ServeHTTP handles GET/POST /git/{projectID}/{subpath...}
func (h *GitHTTPHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// 1. 解析 projectID 与子路径；未知项目 404
	// 2. 认证：Basic password=token（scope read=只读路径；write=全部）或 session cookie
	//    只读路径: info/refs, git-upload-pack；写路径: git-receive-pack（write scope 或 admin）
	// 3. 归档项目拒绝 receive-pack -> 410
	// 4. exec git http-backend:
	//    env: REQUEST_METHOD, PATH_INFO=/repo.git/<sub>, QUERY_STRING,
	//         CONTENT_TYPE, CONTENT_LENGTH, REMOTE_USER,
	//         GIT_PROJECT_ROOT=<reposRoot>, GIT_HTTP_EXPORT_ALL=1, GIT_CONFIG_NOSYSTEM=1
	//    stdin=请求体（POST），stdout 直通；解析 "Status: NNN" 头行设置响应码
	//    透传 Content-Type / Content-Length / Expires / Cache-Control
}
```

路由：

```go
r.Handle("/git/{projectID}/*", gith)
```

- [x] **Step 4: 确认通过** → **Step 5: 提交**（`feat(githttp): git smart http backend with token auth`）

### Task 4: Git HTTP 端到端验证（clone/push）

- [x] **Step 1: 手工验证脚本**

```bash
TOKEN=<write token>; ID=<project id>
git clone http://x:$TOKEN@127.0.0.1:8080/git/$ID /tmp/wiki-clone
cd /tmp/wiki-clone
echo "# via git" >> README.md
git add -A && git commit -m "push via git client"
git push origin main
curl -s -b /tmp/cj.txt http://127.0.0.1:8080/api/v1/projects/$ID/docs/pages/README.md | grep "via git"
```

- [x] **Step 2: 验证失败场景**（read token push 拒绝；无认证 401）→ **Step 3: 修复直至全过** → **Step 4: 提交**（如实现无需改动，跳过提交，evidence 记录）

### Task 5: 自定义侧栏 `_sidebar.md`

**Files:**
- Modify: `web/src/routes/docs-viewer.tsx`
- Modify: `web/src/routes/docs-viewer.test.tsx`

- [x] **Step 1: 失败测试**（mock getPage("_sidebar.md") 返回链接列表 → 渲染菜单；无值不渲染；点击导航；`_sidebar.md` 不在树中）

- [x] **Step 2: 确认失败** → **Step 3: 实现**（docs-viewer：`sidebarQuery`；解析 `- [label](path)` 正则 → 侧栏顶部菜单（当前文档高亮）；树查询过滤掉 `_sidebar.md`）→ **Step 4: 通过** → **Step 5: 提交**（`feat(sidebar): custom _sidebar.md menu`）

### Task 6: 文档 + 全量验收

- [x] **Step 1: api.md** 补 Git HTTP（URL 模式、Basic 认证、示例）与 attachments；**development.md** 补附件/侧栏；**README** 功能清单
- [x] **Step 2: 全量验证**（go test ./... + vet + vitest + build + Docker 部署冒烟）
- [x] **Step 3: 浏览器手工验收**（上传图片附件 → 显示/下载；`_sidebar.md` 生效；clone/push 走通）
- [x] **Step 4: 提交**（`docs: otterwiki alignment`）

## 3. 风险

- **http-backend 差异**：不同 git 版本 CGI 输出差异——exec 直通 + 透传响应头；Docker 镜像已含 git
- **大附件**：base64 膨胀 33%；单文件 ≤ 5 MiB（MaxImportFileBytes 已有）
- **push 权限**：token scope 判定（write=push）；归档项目拒绝 push
- **PATH_INFO 重写**：内部将 `/git/{id}/` 重写为 `/repo.git/` 传给 http-backend
