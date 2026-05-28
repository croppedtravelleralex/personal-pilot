package launchcode

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestWorkflowAPIEndpoints(t *testing.T) {
	server := NewLaunchServer(NewLaunchCodeService(nil), nil, nil, nil, 0)
	mux := server.buildMux()
	body := bytes.NewBufferString(`{"id":"wf-test","steps":[{"id":"step-001","action":{"type":"browser:navigate","params":{"url":"https://example.test"}}}]}`)
	req := httptest.NewRequest(http.MethodPost, "/api/workflow", body)
	w := httptest.NewRecorder()
	mux.ServeHTTP(w, req)
	if w.Code != http.StatusOK {
		t.Fatalf("create code = %d body=%s", w.Code, w.Body.String())
	}
	var created map[string]any
	if err := json.Unmarshal(w.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	if created["workflowId"] != "wf-test" {
		t.Fatalf("created = %+v", created)
	}

	w = httptest.NewRecorder()
	mux.ServeHTTP(w, httptest.NewRequest(http.MethodPost, "/api/workflow/wf-test/execute", nil))
	if w.Code != http.StatusOK {
		t.Fatalf("execute code = %d body=%s", w.Code, w.Body.String())
	}

	w = httptest.NewRecorder()
	mux.ServeHTTP(w, httptest.NewRequest(http.MethodGet, "/api/workflow/wf-test/status", nil))
	if w.Code != http.StatusOK {
		t.Fatalf("status code = %d body=%s", w.Code, w.Body.String())
	}

	w = httptest.NewRecorder()
	mux.ServeHTTP(w, httptest.NewRequest(http.MethodPost, "/api/plugin/install", bytes.NewBufferString(`{}`)))
	if w.Code != http.StatusOK {
		t.Fatalf("plugin install code = %d body=%s", w.Code, w.Body.String())
	}
}
