package main

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"

	"personal-pilot/backend"
)

func TestAllowedRPCMethodsIncludesFingerprintHealth(t *testing.T) {
	if _, ok := allowedRPCMethods["WorkbenchFingerprintHealthProfile"]; !ok {
		t.Fatal("WorkbenchFingerprintHealthProfile should be allowed by sidecar RPC")
	}

	app := backend.NewApp(t.TempDir())
	_, err := callAppMethod(app, "WorkbenchFingerprintHealthProfile", []json.RawMessage{json.RawMessage(`"missing-profile"`)})
	if err == nil {
		t.Fatal("expected missing profile error")
	}
	if strings.Contains(err.Error(), "not allowed") || strings.Contains(err.Error(), "unknown method") {
		t.Fatalf("method was not callable through sidecar RPC: %v", err)
	}
}

func TestWriteJSONUsesContentLengthAndSingleBody(t *testing.T) {
	w := httptest.NewRecorder()
	writeJSON(w, http.StatusCreated, rpcResponse{
		OK:     true,
		Result: map[string]string{"source": "local-cdp"},
	})

	body := w.Body.Bytes()
	if got, want := w.Code, http.StatusCreated; got != want {
		t.Fatalf("status = %d, want %d", got, want)
	}
	if got, want := w.Header().Get("Content-Length"), strconv.Itoa(len(body)); got != want {
		t.Fatalf("Content-Length = %q, want %q", got, want)
	}
	if bytes.HasSuffix(body, []byte("\n")) {
		t.Fatalf("body should be a single json.Marshal body without encoder newline: %q", string(body))
	}

	var decoded rpcResponse
	if err := json.Unmarshal(body, &decoded); err != nil {
		t.Fatalf("decode response body: %v body=%s", err, string(body))
	}
	if !decoded.OK {
		t.Fatalf("decoded response should be ok: %+v", decoded)
	}
}
