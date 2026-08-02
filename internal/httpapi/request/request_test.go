package request

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestDecodeJSON(t *testing.T) {
	req := httptest.NewRequest(http.MethodPost, "/",
		strings.NewReader(`{"username":"admin"}`))
	rec := httptest.NewRecorder()
	var v struct {
		Username string `json:"username"`
	}
	if err := DecodeJSON(rec, req, &v, 1024); err != nil {
		t.Fatalf("DecodeJSON: %v", err)
	}
	if v.Username != "admin" {
		t.Fatalf("username = %q", v.Username)
	}
}

func TestDecodeJSONRejectsBadJSON(t *testing.T) {
	req := httptest.NewRequest(http.MethodPost, "/",
		strings.NewReader(`{"username":`))
	rec := httptest.NewRecorder()
	var v map[string]any
	if err := DecodeJSON(rec, req, &v, 1024); err == nil {
		t.Fatal("want error for malformed JSON")
	}
}

func TestDecodeJSONRejectsOversizedBody(t *testing.T) {
	req := httptest.NewRequest(http.MethodPost, "/",
		strings.NewReader(strings.Repeat("x", 2048)))
	rec := httptest.NewRecorder()
	var v map[string]any
	if err := DecodeJSON(rec, req, &v, 100); err == nil {
		t.Fatal("want error for oversized body")
	}
}

func TestRequestIDContext(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	ctx := WithRequestID(req.Context(), "req_123")
	if got := RequestID(req.WithContext(ctx)); got != "req_123" {
		t.Fatalf("RequestID = %q", got)
	}
	if got := RequestID(req); got != "" {
		t.Fatalf("RequestID without ctx = %q", got)
	}
}
