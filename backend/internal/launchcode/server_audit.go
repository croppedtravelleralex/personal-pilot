package launchcode

import (
	"net/http"
	"strings"
	"time"
)

type auditResponseWriter struct {
	http.ResponseWriter
	status int
}

func (w *auditResponseWriter) WriteHeader(code int) {
	w.status = code
	w.ResponseWriter.WriteHeader(code)
}

func (w *auditResponseWriter) Write(b []byte) (int, error) {
	if w.status == 0 {
		w.status = http.StatusOK
	}
	return w.ResponseWriter.Write(b)
}

func (s *LaunchServer) apiAuditMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		path := r.URL.Path
		if !strings.HasPrefix(path, "/api/") {
			next.ServeHTTP(w, r)
			return
		}
		if path == "/api/health" {
			next.ServeHTTP(w, r)
			return
		}

		startAt := time.Now()
		recorder := &auditResponseWriter{ResponseWriter: w}
		next.ServeHTTP(recorder, r)

		status := recorder.status
		if status == 0 {
			status = http.StatusOK
		}
		s.appendAPICallLog(r.Method, path, remoteIP(r.RemoteAddr), status >= 200 && status < 400, status, startAt)
	})
}

func (s *LaunchServer) appendAPICallLog(method, path, clientIP string, ok bool, status int, startAt time.Time) {
	entry := LaunchCallRecord{
		Timestamp:  time.Now().Format(time.RFC3339),
		Method:     method,
		Path:       path,
		ClientIP:   clientIP,
		Category:   "api",
		OK:         ok,
		Status:     status,
		DurationMs: time.Since(startAt).Milliseconds(),
	}

	s.logMu.Lock()
	s.callLogs = append(s.callLogs, entry)
	if len(s.callLogs) > 500 {
		s.callLogs = append([]LaunchCallRecord(nil), s.callLogs[len(s.callLogs)-500:]...)
	}
	s.logMu.Unlock()
}

func (s *LaunchServer) handleAuditLogs(w http.ResponseWriter, r *http.Request) {
	s.handleLaunchLogs(w, r)
}
