package launchcode

import (
	"net/http"
	"regexp"
	"strings"
	"time"

	"personal-pilot/backend/internal/email"
	"personal-pilot/backend/internal/logger"
)

type createInboxRequest struct{}

type waitCodeRequest struct {
	FromSuffix      string `json:"fromSuffix"`
	SubjectContains string `json:"subjectContains"`
	CodePattern     string `json:"codePattern"`
	Timeout         int    `json:"timeout"`
}

func (s *LaunchServer) SetEmailService(svc *email.EmailService) {
	s.emailService = svc
}

func (s *LaunchServer) HandleCreateInbox(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if s.emailService == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "email service not available"})
		return
	}

	session, err := s.emailService.CreateInbox(r.Context())
	if err != nil {
		logger.New("EmailAPI").Error("create inbox failed", logger.F("error", err.Error()))
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "error": "create inbox failed"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true,
		"data": map[string]interface{}{
			"id":        session.ID,
			"email":     session.Email,
			"provider":  session.Provider,
			"status":    session.Status,
			"createdAt": session.CreatedAt.Format(time.RFC3339),
			"expiresAt": session.ExpiresAt.Format(time.RFC3339),
		},
	})
}

func (s *LaunchServer) handleInboxByID(w http.ResponseWriter, r *http.Request) {
	if s.emailService == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "email service not available"})
		return
	}

	path := strings.TrimPrefix(r.URL.Path, "/api/email/inbox/")
	path = strings.TrimSpace(path)

	if strings.HasSuffix(path, "/wait-code") {
		sessionID := strings.TrimSuffix(path, "/wait-code")
		sessionID = strings.TrimSpace(sessionID)
		if sessionID == "" || strings.Contains(sessionID, "/") {
			writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "inbox not found"})
			return
		}
		if r.Method != http.MethodPost {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		s.handleInboxWaitCode(w, r, sessionID)
		return
	}

	sessionID := path
	if sessionID == "" || strings.Contains(sessionID, "/") {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "inbox not found"})
		return
	}

	switch r.Method {
	case http.MethodGet:
		s.handleGetInbox(w, r, sessionID)
	case http.MethodDelete:
		s.handleDeleteInbox(w, r, sessionID)
	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
	}
}

func (s *LaunchServer) handleGetInbox(w http.ResponseWriter, r *http.Request, sessionID string) {
	session, err := s.emailService.GetStore().Get(sessionID)
	if err != nil {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "inbox not found"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true,
		"data": map[string]interface{}{
			"id":        session.ID,
			"email":     session.Email,
			"status":    session.Status,
			"createdAt": session.CreatedAt.Format(time.RFC3339),
			"expiresAt": session.ExpiresAt.Format(time.RFC3339),
		},
	})
}

func (s *LaunchServer) handleDeleteInbox(w http.ResponseWriter, r *http.Request, sessionID string) {
	if err := s.emailService.ReleaseInbox(r.Context(), sessionID); err != nil {
		logger.New("EmailAPI").Error("release inbox failed", logger.F("error", err.Error()))
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "error": "release inbox failed"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "released": true})
}

func (s *LaunchServer) handleInboxWaitCode(w http.ResponseWriter, r *http.Request, sessionID string) {
	var req waitCodeRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}

	timeout := 300 * time.Second
	if req.Timeout > 0 {
		timeout = time.Duration(req.Timeout) * time.Second
	}

	var codePattern *regexp.Regexp
	if strings.TrimSpace(req.CodePattern) != "" {
		pattern, err := regexp.Compile(req.CodePattern)
		if err != nil {
			writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid codePattern"})
			return
		}
		codePattern = pattern
	}

	filter := &email.MailFilter{
		FromSuffix:      req.FromSuffix,
		SubjectContains: req.SubjectContains,
		CodePattern:     codePattern,
	}

	start := time.Now()
	code, err := s.emailService.WaitForCode(r.Context(), sessionID, filter, timeout)
	elapsed := time.Since(start).Milliseconds()

	if err != nil {
		logger.New("EmailAPI").Error("wait code failed", logger.F("error", err.Error()))
		writeJSON(w, http.StatusGatewayTimeout, map[string]interface{}{
			"ok":        false,
			"error":     "verification code not received",
			"elapsedMs": elapsed,
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true,
		"data": map[string]interface{}{
			"code":      code,
			"elapsedMs": elapsed,
		},
	})
}
