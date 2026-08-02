package response

import (
	"encoding/json"
	"net/http"

	"agentdocs/internal/httpapi/request"
)

// ErrorBody is the unified error envelope (spec §20).
type ErrorBody struct {
	Error ErrorDetails `json:"error"`
}

type ErrorDetails struct {
	Code      string `json:"code"`
	Message   string `json:"message"`
	RequestID string `json:"request_id"`
}

func WriteJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

func WriteError(w http.ResponseWriter, r *http.Request, status int, code, message string) {
	WriteJSON(w, status, ErrorBody{
		Error: ErrorDetails{
			Code:      code,
			Message:   message,
			RequestID: request.RequestID(r),
		},
	})
}
