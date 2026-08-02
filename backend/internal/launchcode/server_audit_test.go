package launchcode

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestAPIAuditMiddlewareRecordsWorkbenchCalls(t *testing.T) {
	t.Parallel()

	srv := NewLaunchServer(nil, nil, nil, nil, 0)
	handler := srv.apiAuditMiddleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true})
	}))

	req := httptest.NewRequest(http.MethodPost, "/api/workbench/actions", nil)
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rec.Code)
	}
	items := srv.listLaunchLogs(10)
	if len(items) == 0 {
		t.Fatal("expected audit log entry")
	}
	found := false
	for _, item := range items {
		if item.Category == "api" && item.Path == "/api/workbench/actions" && item.Method == http.MethodPost {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("audit log missing workbench call: %+v", items)
	}
}
