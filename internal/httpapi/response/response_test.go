package response

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"xwiki/internal/httpapi/request"
)

func TestWriteErrorEnvelope(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req = req.WithContext(request.WithRequestID(req.Context(), "req_123"))
	rec := httptest.NewRecorder()
	WriteError(rec, req, http.StatusConflict, "revision_conflict", "Project revision has changed.")
	if rec.Code != http.StatusConflict {
		t.Fatalf("status = %d", rec.Code)
	}
	var body struct {
		Error struct {
			Code      string `json:"code"`
			Message   string `json:"message"`
			RequestID string `json:"request_id"`
		} `json:"error"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Error.Code != "revision_conflict" || body.Error.RequestID != "req_123" {
		t.Fatalf("bad envelope: %+v", body)
	}
}
