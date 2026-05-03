package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
	"time"

	"personal-pilot/backend"
)

func TestAllowedRPCMethodsIncludesFingerprintHealth(t *testing.T) {
	if _, ok := allowedRPCMethods["WorkbenchFingerprintHealthProfile"]; !ok {
		t.Fatal("WorkbenchFingerprintHealthProfile should be allowed by sidecar RPC")
	}
	for _, method := range []string{
		"WorkbenchListDetectionResults",
		"WorkbenchSaveDetectionResult",
		"WorkbenchGetUiState",
		"WorkbenchSaveUiState",
		"WorkbenchListDetectorSites",
		"WorkbenchRunDetectorSite",
	} {
		if _, ok := allowedRPCMethods[method]; !ok {
			t.Fatalf("%s should be allowed by sidecar RPC", method)
		}
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

func TestShutdownClosesEventStream(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	app := backend.NewApp(t.TempDir())
	hub := newEventHub()
	server, bridgeURL, eventURL, err := startBridgeServer(ctx, app, hub, "bridge-token", "event-token", cancel)
	if err != nil {
		t.Fatalf("start bridge server: %v", err)
	}
	defer server.Close()

	req, err := http.NewRequest(http.MethodGet, eventURL, nil)
	if err != nil {
		t.Fatalf("create events request: %v", err)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("open event stream: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("event stream status = %d, want %d", resp.StatusCode, http.StatusOK)
	}

	reader := bufio.NewReader(resp.Body)
	line, err := reader.ReadString('\n')
	if err != nil {
		t.Fatalf("read event stream greeting: %v", err)
	}
	if strings.TrimSpace(line) != ": connected" {
		t.Fatalf("event stream greeting = %q", line)
	}

	streamDone := make(chan error, 1)
	go func() {
		_, err := io.Copy(io.Discard, reader)
		streamDone <- err
	}()

	shutdownReq, err := http.NewRequest(
		http.MethodPost,
		bridgeURL+"/shutdown",
		strings.NewReader(`{"mode":"full"}`),
	)
	if err != nil {
		t.Fatalf("create shutdown request: %v", err)
	}
	shutdownReq.Header.Set("Content-Type", "application/json")
	shutdownReq.Header.Set(bridgeTokenHeader, "bridge-token")

	shutdownResp, err := http.DefaultClient.Do(shutdownReq)
	if err != nil {
		t.Fatalf("post shutdown: %v", err)
	}
	_ = shutdownResp.Body.Close()
	if shutdownResp.StatusCode != http.StatusOK {
		t.Fatalf("shutdown status = %d, want %d", shutdownResp.StatusCode, http.StatusOK)
	}

	select {
	case err := <-streamDone:
		if err != nil {
			t.Fatalf("event stream close returned error: %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("event stream stayed open after shutdown")
	}
}
