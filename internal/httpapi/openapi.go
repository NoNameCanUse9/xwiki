package httpapi

import (
	_ "embed"
	"encoding/json"
	"net/http"
	"sync"

	"gopkg.in/yaml.v3"
	"xwiki/internal/httpapi/response"
	"xwiki/internal/project"
)

//go:embed openapi.yaml
var openapiYAML []byte

var (
	openapiJSONCache []byte
	openapiJSONOnce  sync.Once
)

// openapiJSON converts the embedded YAML spec to JSON once and caches it.
func openapiJSON() []byte {
	openapiJSONOnce.Do(func() {
		var spec any
		if err := yaml.Unmarshal(openapiYAML, &spec); err != nil {
			openapiJSONCache = []byte(`{"error":"openapi spec failed to parse"}`)
			return
		}
		out, err := json.Marshal(spec)
		if err != nil {
			openapiJSONCache = []byte(`{"error":"openapi spec failed to encode"}`)
			return
		}
		openapiJSONCache = out
	})
	return openapiJSONCache
}

// ServiceVersion is the XWiki release version reported by /api/v1/meta.
const ServiceVersion = "0.8.0"

// API capabilities reported by /api/v1/meta. Keep in sync with the endpoint
// set actually registered in internal/server/router.go.
var metaCapabilities = []string{
	"meta", "changesets", "revision", "locks", "shares", "backlinks",
	"search", "attachments", "import_bundle", "import_repo", "import_folder",
	"tokens", "users", "audit", "openapi", "export", "revert", "file_history",
	"commit_search", "project_trash", "document_revision",
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

// OpenAPIHandler serves the OpenAPI 3.0 document at /api/openapi.json.
// Scalar (the web api-docs viewer) consumes this endpoint and expects a
// JSON document; the embedded spec is authored in YAML so we convert it
// on the fly and cache the result.
func OpenAPIHandler(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.Write(openapiJSON())
}
