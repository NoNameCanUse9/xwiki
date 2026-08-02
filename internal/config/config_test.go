package config

import "testing"

func TestEnvOr(t *testing.T) {
	t.Setenv("AGENTDOCS_TEST_X", "value")
	if got := envOr("AGENTDOCS_TEST_X", "default"); got != "value" {
		t.Fatalf("envOr = %q, want %q", got, "value")
	}
	t.Setenv("AGENTDOCS_TEST_X", "")
	if got := envOr("AGENTDOCS_TEST_X", "default"); got != "default" {
		t.Fatalf("envOr = %q, want %q", got, "default")
	}
}

func TestLoadDefaults(t *testing.T) {
	t.Setenv("AGENTDOCS_DATA_DIR", "")
	t.Setenv("AGENTDOCS_HTTP_ADDR", "")
	cfg := Load()
	if cfg.DataDir != "data" || cfg.HTTPAddr != ":8080" {
		t.Fatalf("unexpected defaults: %+v", cfg)
	}
	if cfg.SessionTTL <= 0 || cfg.MaxBodyBytes <= 0 {
		t.Fatalf("unexpected defaults: %+v", cfg)
	}
}

func TestLoadFromEnv(t *testing.T) {
	t.Setenv("AGENTDOCS_HTTP_ADDR", ":9090")
	t.Setenv("AGENTDOCS_SESSION_TTL", "1h")
	cfg := Load()
	if cfg.HTTPAddr != ":9090" {
		t.Fatalf("HTTPAddr = %q", cfg.HTTPAddr)
	}
	if cfg.SessionTTL.String() != "1h0m0s" {
		t.Fatalf("SessionTTL = %v", cfg.SessionTTL)
	}
}
