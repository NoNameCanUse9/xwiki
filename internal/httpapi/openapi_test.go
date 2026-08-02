package httpapi

import (
	"encoding/json"
	"testing"
)

func TestOpenAPISpecShape(t *testing.T) {
	spec := OpenAPISpec()
	if spec["openapi"] != "3.0.3" {
		t.Fatalf("openapi version: %v", spec["openapi"])
	}
	info, _ := spec["info"].(map[string]any)
	if info == nil || info["title"] != "AgentDocs API" {
		t.Fatalf("info wrong: %v", info)
	}
	paths, _ := spec["paths"].(map[string]any)
	// All documented endpoint paths.
	expected := []string{
		"/auth/login", "/auth/logout", "/auth/me", "/auth/password",
		"/tokens", "/tokens/{id}",
		"/projects", "/projects/{id}", "/projects/{id}/archive",
		"/projects/{id}/revision", "/projects/{id}/changesets",
		"/projects/{id}/commits", "/projects/{id}/commits/{sha}",
		"/projects/{id}/commits/{sha}/diff", "/projects/{id}/commits/{sha}/revert",
		"/projects/{id}/docs/tree", "/projects/{id}/docs/home",
		"/projects/{id}/docs/pages/{path}", "/projects/{id}/files/history/{path}",
		"/projects/{id}/search", "/projects/{id}/audit",
		"/projects/{id}/export.zip", "/projects/{id}/export.bundle",
		"/projects/{id}/import", "/import/bundle",
	}
	for _, p := range expected {
		if _, ok := paths[p]; !ok {
			t.Fatalf("spec missing path %s", p)
		}
	}
	if len(paths) < 20 {
		t.Fatalf("too few paths: %d", len(paths))
	}
	// Every operation declares security.
	for p, entry := range paths {
		for method, op := range entry.(map[string]any) {
			_ = method
			opMap, _ := op.(map[string]any)
			if opMap == nil || opMap["security"] == nil {
				t.Fatalf("path %s missing security", p)
			}
		}
	}
	// Serializes cleanly.
	if _, err := json.Marshal(spec); err != nil {
		t.Fatalf("marshal: %v", err)
	}
}
