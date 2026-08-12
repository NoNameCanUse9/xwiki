package httpapi

import (
	"strings"
	"testing"

	"gopkg.in/yaml.v3"
)

// parseOpenAPI loads the embedded YAML spec into plain maps.
func parseOpenAPI(t *testing.T) map[string]any {
	t.Helper()
	var spec map[string]any
	if err := yaml.Unmarshal(openapiYAML, &spec); err != nil {
		t.Fatalf("openapi.yaml does not parse: %v", err)
	}
	return spec
}

// collectRefs walks the spec tree and returns every $ref string.
func collectRefs(v any, refs *[]string) {
	switch x := v.(type) {
	case map[string]any:
		for k, val := range x {
			if k == "$ref" {
				if s, ok := val.(string); ok {
					*refs = append(*refs, s)
				}
			}
			collectRefs(val, refs)
		}
	case []any:
		for _, item := range x {
			collectRefs(item, refs)
		}
	}
}

func TestOpenAPISpec(t *testing.T) {
	spec := parseOpenAPI(t)
	if spec["openapi"] != "3.0.3" {
		t.Fatalf("openapi version: %v", spec["openapi"])
	}

	// Every endpoint path actually registered in internal/server/router.go.
	expected := map[string][]string{
		"/meta":       {"get"},
		"/auth/login": {"post"}, "/auth/logout": {"post"},
		"/auth/me": {"get"}, "/auth/password": {"post"},
		"/import/bundle": {"post"}, "/import/repo": {"post"},
		"/users":              {"get", "post"},
		"/users/{id}":         {"delete"},
		"/users/{id}/disable": {"post"}, "/users/{id}/enable": {"post"},
		"/tokens": {"post", "get"}, "/tokens/{id}": {"delete"},
		"/projects":               {"post", "get"},
		"/projects/import-folder": {"post"},
		"/projects/{id}":          {"get", "patch", "delete"},
		"/projects/{id}/archive":  {"post"}, "/projects/{id}/unarchive": {"post"},
		"/projects/{id}/restore": {"post"}, "/projects/{id}/purge": {"post", "delete"},
		"/projects/{id}/docs/tree":            {"get"},
		"/projects/{id}/docs/home":            {"get"},
		"/projects/{id}/docs/pages/{path}":    {"get"},
		"/projects/{id}/revision":             {"get"},
		"/projects/{id}/changesets":           {"post"},
		"/projects/{id}/locks":                {"get", "post", "delete"},
		"/projects/{id}/locks/heartbeat":      {"post"},
		"/projects/{id}/locks/force-release":  {"post"},
		"/projects/{id}/shares":               {"post"},
		"/projects/{id}/search":               {"get"},
		"/projects/{id}/backlinks":            {"get"},
		"/projects/{id}/export.zip":           {"get"},
		"/projects/{id}/export.bundle":        {"get"},
		"/projects/{id}/import":               {"post"},
		"/projects/{id}/attachments/{path}":   {"get"},
		"/projects/{id}/audit":                {"get"},
		"/projects/{id}/commits":              {"get"},
		"/projects/{id}/commits/{sha}":        {"get"},
		"/projects/{id}/commits/{sha}/diff":   {"get"},
		"/projects/{id}/commits/{sha}/revert": {"post"},
		"/projects/{id}/files/history/{path}": {"get"},
	}
	paths, _ := spec["paths"].(map[string]any)
	if paths == nil {
		t.Fatalf("no paths in spec")
	}
	if len(paths) != len(expected) {
		t.Fatalf("path count: got %d, want %d", len(paths), len(expected))
	}
	for p, methods := range expected {
		entry, ok := paths[p].(map[string]any)
		if !ok {
			t.Fatalf("spec missing path %s", p)
		}
		for _, m := range methods {
			op, _ := entry[m].(map[string]any)
			if op == nil {
				t.Fatalf("path %s missing %s", p, m)
			}
			if op["responses"] == nil {
				t.Fatalf("path %s %s missing responses", p, m)
			}
			if _, ok := op["responses"].(map[string]any)["200"]; !ok {
				if _, ok := op["responses"].(map[string]any)["201"]; !ok {
					t.Fatalf("path %s %s missing 2xx response", p, m)
				}
			}
		}
		if len(entry) != len(methods) {
			t.Fatalf("path %s has unexpected methods: %v", p, keys(entry))
		}
	}

	// Only the documented public operations may disable security; everything
	// else inherits the top-level sessionCookie/bearerAuth scheme.
	public := map[string]bool{"/meta": true, "/auth/login": true}
	for p, entry := range paths {
		for method, op := range entry.(map[string]any) {
			opMap, _ := op.(map[string]any)
			if opMap == nil || opMap["responses"] == nil {
				t.Fatalf("path %s %s malformed", p, method)
			}
			if sec, ok := opMap["security"]; ok {
				arr, _ := sec.([]any)
				if len(arr) == 0 && !public[p] {
					t.Fatalf("path %s %s: only public ops may declare empty security", p, method)
				}
			}
		}
	}

	// Every $ref must resolve within components.
	components, _ := spec["components"].(map[string]any)
	schemas, _ := components["schemas"].(map[string]any)
	responses, _ := components["responses"].(map[string]any)
	params, _ := components["parameters"].(map[string]any)
	var refs []string
	collectRefs(spec, &refs)
	for _, ref := range refs {
		if !strings.HasPrefix(ref, "#/components/") {
			t.Fatalf("unexpected ref target: %s", ref)
		}
		parts := strings.Split(strings.TrimPrefix(ref, "#/components/"), "/")
		if len(parts) != 2 {
			t.Fatalf("malformed ref: %s", ref)
		}
		kind, name := parts[0], parts[1]
		var exists bool
		switch kind {
		case "schemas":
			_, exists = schemas[name]
		case "responses":
			_, exists = responses[name]
		case "parameters":
			_, exists = params[name]
		}
		if !exists {
			t.Fatalf("unresolved $ref: %s", ref)
		}
	}

	// Required component schemas for the client layers.
	for _, name := range []string{
		"Error", "Meta", "User", "Token", "Project", "TreeEntry", "DocPage",
		"Revision", "Change", "ChangesetRequest", "ChangesetResult",
		"Commit", "CommitDetail", "DiffStats", "DiffPatch", "SearchResult",
		"Backlink", "Lock", "Share", "AuditEntry", "ImportRequest",
		"ImportResult", "ImportBundleResult",
	} {
		if schemas[name] == nil {
			t.Fatalf("missing component schema %s", name)
		}
	}
}

func keys(m map[string]any) []string {
	var out []string
	for k := range m {
		out = append(out, k)
	}
	return out
}
