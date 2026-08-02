package config

import (
	"os"
	"strconv"
	"time"
)

type Config struct {
	DataDir       string
	HTTPAddr      string
	WebOrigin     string
	SessionTTL    time.Duration
	MaxBodyBytes  int64
	SecureCookies bool
}

// Load reads configuration from AGENTDOCS_* environment variables,
// falling back to development-friendly defaults.
func Load() *Config {
	return &Config{
		DataDir:       envOr("AGENTDOCS_DATA_DIR", "data"),
		HTTPAddr:      envOr("AGENTDOCS_HTTP_ADDR", ":8080"),
		WebOrigin:     envOr("AGENTDOCS_WEB_ORIGIN", "http://localhost:5173"),
		SessionTTL:    envDuration("AGENTDOCS_SESSION_TTL", 30*24*time.Hour),
		MaxBodyBytes:  envInt64("AGENTDOCS_MAX_BODY_BYTES", 1<<20),
		SecureCookies: envBool("AGENTDOCS_COOKIE_SECURE", false),
	}
}

func envOr(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func envDuration(key string, def time.Duration) time.Duration {
	if v := os.Getenv(key); v != "" {
		if d, err := time.ParseDuration(v); err == nil {
			return d
		}
	}
	return def
}

func envInt64(key string, def int64) int64 {
	if v := os.Getenv(key); v != "" {
		if n, err := strconv.ParseInt(v, 10, 64); err == nil {
			return n
		}
	}
	return def
}

func envBool(key string, def bool) bool {
	if v := os.Getenv(key); v != "" {
		if b, err := strconv.ParseBool(v); err == nil {
			return b
		}
	}
	return def
}
