package httpapi

import (
	_ "embed"
	"net/http"

	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/project"
)

//go:embed openapi.yaml
var openapiYAML []byte

// ServiceVersion is the AgentDocs release version reported by /api/v1/meta.
const ServiceVersion = "0.8.0"

// API capabilities reported by /api/v1/meta. Keep in sync with the endpoint
// set actually registered in internal/server/router.go.
var metaCapabilities = []string{
	"meta", "changesets", "revision", "locks", "shares", "backlinks",
	"search", "attachments", "import_bundle", "import_repo", "import_folder",
	"tokens", "users", "audit", "openapi", "export", "revert", "file_history",
}

// MetaHandler serves GET /api/v1/meta — service info and capability probe
// for unauthenticated clients (CLI bootstrap, version checks).
func MetaHandler(w http.ResponseWriter, r *http.Request) {
	response.WriteJSON(w, http.StatusOK, map[string]any{
		"version":     ServiceVersion,
		"api_version": "1",
		"limits": map[string]any{
			"max_doc_bytes":           project.MaxDocBlobBytes,
			"max_import_bytes":        project.MaxImportFileBytes,
			"max_diff_bytes":          project.MaxDiffBytes,
			"max_changes_per_request": project.MaxChangesetFiles,
		},
		"capabilities": metaCapabilities,
	})
}

// OpenAPIHandler serves the OpenAPI 3.0 document (YAML) at /api/openapi.json.
func OpenAPIHandler(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/yaml; charset=utf-8")
	w.Write(openapiYAML)
}
