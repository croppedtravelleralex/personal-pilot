package proxy

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestCheckHTTPClientGETReturnsStatusAndLatency(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusNoContent)
	}))
	defer server.Close()

	client := &http.Client{Timeout: time.Second}
	latency, statusCode, err := checkHTTPClientGET(context.Background(), client, server.URL, "test")
	if err != nil {
		t.Fatalf("checkHTTPClientGET returned error: %v", err)
	}
	if statusCode != http.StatusNoContent {
		t.Fatalf("statusCode = %d, want %d", statusCode, http.StatusNoContent)
	}
	if latency < 0 {
		t.Fatalf("latency = %d, want non-negative", latency)
	}
}

func TestIsUsableHTTPStatus(t *testing.T) {
	for _, statusCode := range []int{http.StatusOK, http.StatusNoContent, http.StatusFound} {
		if !isUsableHTTPStatus(statusCode) {
			t.Fatalf("status %d should be usable", statusCode)
		}
	}
	for _, statusCode := range []int{http.StatusUnauthorized, http.StatusInternalServerError, 0} {
		if isUsableHTTPStatus(statusCode) {
			t.Fatalf("status %d should not be usable", statusCode)
		}
	}
}
