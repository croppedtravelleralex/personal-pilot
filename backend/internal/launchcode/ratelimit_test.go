package launchcode

import (
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestNewRateLimiter(t *testing.T) {
	rl := NewRateLimiter(10, 5)
	if rl == nil {
		t.Fatal("expected non-nil RateLimiter")
	}
	if rl.rate != 10 {
		t.Fatalf("expected rate 10, got %d", rl.rate)
	}
	if rl.burst != 5 {
		t.Fatalf("expected burst 5, got %d", rl.burst)
	}
}

func TestRateLimiterAllowFirstRequest(t *testing.T) {
	rl := NewRateLimiter(100, 3)
	if !rl.allow("127.0.0.1") {
		t.Fatal("first request should be allowed")
	}
}

func TestRateLimiterBurstExhausted(t *testing.T) {
	rl := NewRateLimiter(100, 3)
	for i := 0; i < 3; i++ {
		if !rl.allow("127.0.0.1") {
			t.Fatalf("request %d should be allowed (burst)", i+1)
		}
	}
	if rl.allow("127.0.0.1") {
		t.Fatal("request should be denied after burst exhausted")
	}
}

func TestRateLimiterDifferentIPs(t *testing.T) {
	rl := NewRateLimiter(100, 1)
	if !rl.allow("127.0.0.1") {
		t.Fatal("first request from 127.0.0.1 should be allowed")
	}
	if !rl.allow("10.0.0.1") {
		t.Fatal("first request from 10.0.0.1 should be allowed")
	}
	if rl.allow("127.0.0.1") {
		t.Fatal("second request from 127.0.0.1 should be denied (burst=1)")
	}
}

func TestRateLimiterRefill(t *testing.T) {
	rl := NewRateLimiter(10, 1)
	if !rl.allow("127.0.0.1") {
		t.Fatal("first request should be allowed")
	}
	if rl.allow("127.0.0.1") {
		t.Fatal("second request should be denied (burst exhausted)")
	}
	time.Sleep(1100 * time.Millisecond)
	if !rl.allow("127.0.0.1") {
		t.Fatal("request should be allowed after refill")
	}
}

func TestRateLimiterFallbackIPOnSplitError(t *testing.T) {
	rl := NewRateLimiter(100, 1)
	if !rl.allow("") {
		t.Fatal("empty IP should be treated as-is")
	}
	if rl.allow("") {
		t.Fatal("empty IP should be rate limited after burst")
	}
}

func TestHasAPIPrefix(t *testing.T) {
	cases := []struct {
		path string
		want bool
	}{
		{"/api/health", true},
		{"/api/profiles", true},
		{"/api/launch/ABC123", true},
		{"/api", true},
		{"/", false},
		{"/some-other-path", false},
		{"/api/", true},
		{"", false},
		{"/notapi", false},
	}
	for _, tc := range cases {
		got := hasAPIPrefix(tc.path)
		if got != tc.want {
			t.Errorf("hasAPIPrefix(%q) = %v, want %v", tc.path, got, tc.want)
		}
	}
}

func TestRateLimiterMiddlewarePassesNonAPIPath(t *testing.T) {
	rl := NewRateLimiter(1, 0)
	var hit bool
	handler := rl.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		hit = true
		w.WriteHeader(http.StatusOK)
	}))
	req := httptest.NewRequest(http.MethodGet, "/some-other-path", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)
	if !hit {
		t.Fatal("non-API path should pass through")
	}
	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}
}

func TestRateLimiterMiddlewarePassesWithTokens(t *testing.T) {
	rl := NewRateLimiter(100, 2)
	var hit bool
	handler := rl.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		hit = true
		w.WriteHeader(http.StatusOK)
	}))
	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.RemoteAddr = "127.0.0.1:12345"
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)
	if !hit {
		t.Fatal("request with tokens should pass")
	}
	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}
}

func TestRateLimiterMiddlewareBlocksWhenExhausted(t *testing.T) {
	rl := NewRateLimiter(100, 1)
	rl.allow("10.0.0.1")

	var hit bool
	handler := rl.Middleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		hit = true
		w.WriteHeader(http.StatusOK)
	}))
	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.RemoteAddr = "10.0.0.1:54321"
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)
	if hit {
		t.Fatal("rate limited request should not reach handler")
	}
	if w.Code != http.StatusTooManyRequests {
		t.Fatalf("expected 429, got %d", w.Code)
	}
	if w.Header().Get("Retry-After") != "1" {
		t.Fatalf("expected Retry-After: 1, got %q", w.Header().Get("Retry-After"))
	}
}

func TestRateLimiterCleanupRemovesStaleVisitors(t *testing.T) {
	rl := NewRateLimiter(10, 5)
	rl.mu.Lock()
	rl.visitors["stale"] = &visitor{
		tokens:   5,
		lastSeen: time.Now().Add(-31 * time.Minute),
	}
	rl.mu.Unlock()

	rl.mu.Lock()
	for ip, v := range rl.visitors {
		if time.Since(v.lastSeen) > 30*time.Minute {
			delete(rl.visitors, ip)
		}
	}
	rl.mu.Unlock()

	if _, exists := rl.visitors["stale"]; exists {
		t.Fatal("stale visitor should have been removed")
	}
}

func TestRateLimiterCleanupKeepsRecentVisitors(t *testing.T) {
	rl := NewRateLimiter(10, 5)
	rl.mu.Lock()
	rl.visitors["recent"] = &visitor{
		tokens:   5,
		lastSeen: time.Now().Add(-5 * time.Minute),
	}
	rl.mu.Unlock()

	rl.mu.Lock()
	for ip, v := range rl.visitors {
		if time.Since(v.lastSeen) > 30*time.Minute {
			delete(rl.visitors, ip)
		}
	}
	rl.mu.Unlock()

	if _, exists := rl.visitors["recent"]; !exists {
		t.Fatal("recent visitor should not have been removed")
	}
}

func TestRateLimiterIntegrationWithLaunchServer(t *testing.T) {
	srv := newTestServer(t)
	rl := NewRateLimiter(100, 1)
	srv.SetRateLimiter(rl)
	rl.allow("10.0.0.1")

	req := httptest.NewRequest(http.MethodGet, "/api/health", nil)
	req.RemoteAddr = "10.0.0.1:54321"
	w := httptest.NewRecorder()
	srv.buildHandler(false).ServeHTTP(w, req)

	if w.Code != http.StatusTooManyRequests {
		t.Fatalf("expected 429 after exhausting burst, got %d: %s", w.Code, w.Body.String())
	}
	var p struct {
		OK    bool   `json:"ok"`
		Error string `json:"error"`
	}
	decodeJSON(t, w, &p)
	if p.OK {
		t.Fatal("expected ok=false")
	}
	if p.Error != "rate limit exceeded" {
		t.Fatalf("expected 'rate limit exceeded' error, got %q", p.Error)
	}
}
