package httpapi

import (
	"encoding/json"
	"net/http"
)

// OpenAPISpec is the static OpenAPI 3.0 document for AgentDocs.
func OpenAPISpec() map[string]any {
	security := []map[string]any{{"sessionCookie": []any{}}, {"bearerAuth": []any{}}}
	projectParam := []map[string]any{{
		"name": "id", "in": "path", "required": true, "schema": map[string]any{"type": "string"},
	}}
	ok := map[string]any{"description": "OK"}
	paths := map[string]any{}
	add := func(p, method string, summary string, params []map[string]any) {
		entry, _ := paths[p].(map[string]any)
		if entry == nil {
			entry = map[string]any{}
			paths[p] = entry
		}
		entry[method] = map[string]any{
			"summary": summary, "security": security,
			"parameters": params, "responses": map[string]any{"200": ok},
		}
	}
	// addPublic is like add but without auth requirements.
	addPublic := func(p, method, summary string) {
		entry, _ := paths[p].(map[string]any)
		if entry == nil {
			entry = map[string]any{}
			paths[p] = entry
		}
		entry[method] = map[string]any{
			"summary": summary, "responses": map[string]any{"200": ok},
		}
	}
	withID := func(p, m, s string) { add(p, m, s, projectParam) }

	addPublic("/auth/forgot-password", "post", "请求密码重置（自托管：token 写入服务端日志）")
	addPublic("/auth/reset-password", "post", "用一次性 token 重置密码")
	add("/auth/login", "post", "登录（session cookie）", nil)
	add("/auth/logout", "post", "退出登录", nil)
	add("/auth/me", "get", "当前用户", nil)
	add("/auth/password", "post", "修改密码", nil)
	add("/tokens", "post", "创建 Agent Token（session）", nil)
	add("/tokens", "get", "Token 列表（session）", nil)
	add("/tokens/{id}", "delete", "撤销 Token（session）", []map[string]any{{
		"name": "id", "in": "path", "required": true, "schema": map[string]any{"type": "string"},
	}})
	add("/projects", "post", "创建项目（自动初始化 Git 仓库与 README）", nil)
	add("/projects", "get", "项目列表", nil)
	withID("/projects/{id}", "get", "项目详情")
	withID("/projects/{id}/archive", "post", "归档项目")
	withID("/projects/{id}/revision", "get", "当前 revision（HEAD）")
	withID("/projects/{id}/changesets", "post", "原子提交 ChangeSet（可带 Idempotency-Key）")
	withID("/projects/{id}/commits", "get", "Commit 列表")
	withID("/projects/{id}/commits/{sha}", "get", "Commit 详情")
	withID("/projects/{id}/commits/{sha}/diff", "get", "机器可读 Diff（numstat/patch）")
	withID("/projects/{id}/commits/{sha}/revert", "post", "Revert（新提交）")
	withID("/projects/{id}/docs/tree", "get", "文档树（Git Tree 单层）")
	withID("/projects/{id}/docs/home", "get", "项目首页（README 渲染）")
	withID("/projects/{id}/docs/pages/{path}", "get", "读取文档（raw/html/base64）")
	withID("/projects/{id}/files/history/{path}", "get", "文件历史")
	withID("/projects/{id}/search", "get", "全文搜索")
	withID("/projects/{id}/audit", "get", "审计日志（session）")
	withID("/projects/{id}/export.zip", "get", "导出 ZIP 快照")
	withID("/projects/{id}/export.bundle", "get", "导出 Git Bundle")
	withID("/projects/{id}/import", "post", "导入 ZIP 内容（base64 files）")
	add("/import/bundle", "post", "导入 Git Bundle 创建项目", nil)

	return map[string]any{
		"openapi": "3.0.3",
		"info": map[string]any{
			"title": "AgentDocs API", "version": "0.8.0",
			"description": "面向人类与 AI Agent 的 Git-backed 文档管理系统。写入端点支持 Idempotency-Key 幂等。",
		},
		"servers": []map[string]any{{"url": "/api/v1"}},
		"securitySchemes": map[string]any{
			"sessionCookie": map[string]any{"type": "apiKey", "in": "cookie", "name": "agentdocs_session"},
			"bearerAuth":    map[string]any{"type": "http", "scheme": "bearer"},
		},
		"paths": paths,
	}
}

// OpenAPIHandler serves GET /api/openapi.json.
func OpenAPIHandler(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(OpenAPISpec())
}
