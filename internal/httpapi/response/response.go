package response

import (
	"encoding/json"
	"net/http"

	"xwiki/internal/httpapi/request"
)

// ErrorBody is the unified error envelope (spec §20).
type ErrorBody struct {
	Error ErrorDetails `json:"error"`
}

type ErrorDetails struct {
	Code      string `json:"code"`
	Message   string `json:"message"`
	RequestID string `json:"request_id"`
	// Data carries optional structured details (e.g. the lock owned by the
	// other user on a page_locked conflict).
	Data any `json:"data,omitempty"`
}

func WriteJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

func WriteError(w http.ResponseWriter, r *http.Request, status int, code, message string) {
	WriteErrorWith(w, r, status, code, message, nil)
}

// WriteErrorWith writes an error envelope carrying structured data.
func WriteErrorWith(w http.ResponseWriter, r *http.Request, status int, code, message string, data any) {
	WriteJSON(w, status, ErrorBody{
		Error: ErrorDetails{
			Code:      code,
			Message:   message,
			RequestID: request.RequestID(r),
			Data:      data,
		},
	})
}
