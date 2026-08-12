package handlers

import (
	"context"
	"encoding/base64"
	"errors"
	"io"
	"log/slog"
	"net/http"
	"os"
	"os/exec"
	"strconv"
	"strings"

	"xwiki/internal/agent"
	"xwiki/internal/httpapi/middleware"
	"xwiki/internal/httpapi/request"
	"xwiki/internal/httpapi/response"
	"xwiki/internal/project"
	"xwiki/internal/search"
)

// GitHTTPHandler proxies git http-backend for smart HTTP clone/pull/push.
type GitHTTPHandler struct {
	svc       *project.Service
	agentSvc  *agent.Service
	searchSvc *search.Service
	log       *slog.Logger
}

func NewGitHTTPHandler(svc *project.Service, agentSvc *agent.Service, searchSvc *search.Service, log *slog.Logger) *GitHTTPHandler {
	return &GitHTTPHandler{svc: svc, agentSvc: agentSvc, searchSvc: searchSvc, log: log}
}

// ServeHTTP handles GET/POST /git/{projectID}/{subpath...}.
func (h *GitHTTPHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "projectID")
	sub := strings.TrimPrefix(r.URL.Path, "/git/"+projectID+"/")
	if sub == "" {
		response.WriteError(w, r, http.StatusNotFound, "not_found", "invalid git endpoint")
		return
	}
	// Validate the project exists.
	if _, err := h.svc.OpenRepo(r.Context(), projectID); err != nil {
		response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
		return
	}
	p, err := h.svc.Get(r.Context(), projectID)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
		return
	}

	isWrite := strings.Contains(sub, "git-receive-pack") ||
		strings.Contains(r.URL.RawQuery, "git-receive-pack")
	if p.IsArchived() && isWrite {
		response.WriteError(w, r, http.StatusGone, "project_archived", "project is archived")
		return
	}

	// Authenticate: Bearer/Basic token, or session cookie.
	secret := middleware.AgentSecret(r)
	if secret == "" {
		secret = basicToken(r)
	}
	if secret == "" && middleware.UserFrom(r) == nil {
		w.Header().Set("WWW-Authenticate", `Basic realm="xwiki"`)
		response.WriteError(w, r, http.StatusUnauthorized, "authentication_required", "token or login required")
		return
	}
	if secret != "" {
		write := isWrite
		if _, err := h.agentSvc.Authorize(r.Context(), secret, projectID, write); err != nil {
			response.WriteError(w, r, http.StatusForbidden, "agent_forbidden", "token lacks permission for this operation")
			return
		}
	} else if isWrite {
		u := middleware.UserFrom(r)
		if u == nil || !u.IsAdmin {
			response.WriteError(w, r, http.StatusForbidden, "admin_required", "push requires admin privileges")
			return
		}
	}

	if isWrite {
		unlock := project.LockProjectWrite(projectID)
		h.proxy(w, r, projectID, sub)
		unlock()
		if h.searchSvc != nil {
			if _, err := h.searchSvc.ReindexProject(r.Context(), projectID); err != nil {
				h.log.Warn("reindex after git push failed", "error", err, "project_id", projectID)
			}
		}
		return
	}
	h.proxy(w, r, projectID, sub)
}

// proxy runs git http-backend with CGI environment and streams the response.
func (h *GitHTTPHandler) proxy(w http.ResponseWriter, r *http.Request, projectID, sub string) {
	ctx, cancel := context.WithCancel(r.Context())
	defer cancel()

	env := append(os.Environ(),
		"REQUEST_METHOD="+r.Method,
		"PATH_INFO=/repo.git/"+sub,
		"QUERY_STRING="+r.URL.RawQuery,
		"CONTENT_TYPE="+r.Header.Get("Content-Type"),
		"CONTENT_LENGTH="+strconv.FormatInt(r.ContentLength, 10),
		"REMOTE_USER=xwiki",
		"GIT_PROJECT_ROOT="+h.svc.ReposRoot()+string(os.PathSeparator)+projectID,
		"GIT_HTTP_EXPORT_ALL=1",
		"GIT_CONFIG_NOSYSTEM=1",
	)

	cmd := exec.CommandContext(ctx, "git", "http-backend")
	cmd.Env = env
	cmd.Stdin = r.Body

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		h.log.Error("http-backend stdout pipe", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "git backend failed")
		return
	}
	var stderr strings.Builder
	cmd.Stderr = &stderr

	if err := cmd.Start(); err != nil {
		h.log.Error("http-backend start", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "git backend failed")
		return
	}

	// Read the CGI headers (until blank line), then stream the body.
	headerBuf := make([]byte, 0, 4096)
	one := make([]byte, 1)
	status := http.StatusOK
	contentType := ""
	contentLength := int64(-1)
	headerDone := false
	headerEnd := 0
	for {
		n, err := stdout.Read(one)
		if n > 0 {
			headerBuf = append(headerBuf, one[0])
			if !headerDone {
				// detect end of headers: "\r\n\r\n" or "\n\n"
				if len(headerBuf) >= 4 && string(headerBuf[len(headerBuf)-4:]) == "\r\n\r\n" {
					headerDone = true
					headerEnd = len(headerBuf) - 4
				} else if len(headerBuf) >= 2 && string(headerBuf[len(headerBuf)-2:]) == "\n\n" {
					headerDone = true
					headerEnd = len(headerBuf) - 2
				}
			}
		}
		if err != nil {
			break
		}
		if headerDone {
			break
		}
	}
	if headerDone {
		headerText := string(headerBuf[:headerEnd])
		for _, line := range strings.Split(headerText, "\n") {
			line = strings.TrimSpace(line)
			if line == "" {
				continue
			}
			if strings.HasPrefix(line, "Status:") {
				if code, err := strconv.Atoi(strings.TrimSpace(strings.TrimPrefix(line, "Status:"))); err == nil {
					status = code
				}
				continue
			}
			key, val, ok := strings.Cut(line, ":")
			if !ok {
				continue
			}
			key = strings.TrimSpace(key)
			val = strings.TrimSpace(val)
			switch key {
			case "Content-Type":
				contentType = val
			case "Content-Length":
				contentLength, _ = strconv.ParseInt(val, 10, 64)
			default:
				w.Header().Set(key, val)
			}
		}
	}

	// Set headers BEFORE WriteHeader (headers set after are ignored).
	if contentType != "" {
		w.Header().Set("Content-Type", contentType)
	}
	if contentLength >= 0 {
		w.Header().Set("Content-Length", strconv.FormatInt(contentLength, 10))
	}
	w.WriteHeader(status)
	// Write any bytes after the header terminator (skip the separator), then
	// stream the rest.
	sepLen := 2
	if len(headerBuf) >= 4 && string(headerBuf[headerEnd:headerEnd+4]) == "\r\n\r\n" {
		sepLen = 4
	}
	rest := headerBuf[headerEnd+sepLen:]
	if len(rest) > 0 {
		_, _ = w.Write(rest)
	}
	_, _ = io.Copy(w, stdout)
	_ = cmd.Wait()
	if ctx.Err() != nil {
		h.log.Debug("git http request cancelled", "project_id", projectID)
	}
	_ = stderr.String()
}

// basicToken extracts the password from Basic auth (username is ignored;
// the password carries the agent token).
func basicToken(r *http.Request) string {
	auth := r.Header.Get("Authorization")
	if !strings.HasPrefix(auth, "Basic ") {
		return ""
	}
	raw, err := base64.StdEncoding.DecodeString(strings.TrimPrefix(auth, "Basic "))
	if err != nil {
		return ""
	}
	_, pass, ok := strings.Cut(string(raw), ":")
	if !ok {
		return ""
	}
	return pass
}

var _ = errors.Is
