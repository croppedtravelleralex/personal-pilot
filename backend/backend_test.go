package backend

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/gorilla/websocket"

	"personal-pilot/backend/internal/behavior/humanize"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/config"
	"personal-pilot/backend/internal/proxy"
)

// NOTE: Win32-specific tests are in backend_test_win32.go (requires windows build tag).

// ─── Mock CDP WebSocket Server ────────────────────────────────────────────────

type mockCDPHandler func(method string, params json.RawMessage) map[string]interface{}

func newMockCDPServer(t *testing.T, handler mockCDPHandler) (port int, closeFn func()) {
	t.Helper()
	if handler == nil {
		handler = defaultCDPHandler
	}
	upgrader := websocket.Upgrader{CheckOrigin: func(r *http.Request) bool { return true }}
	mux := http.NewServeMux()

	mux.HandleFunc("/json", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		wsURL := fmt.Sprintf("ws://%s/ws", r.Host)
		json.NewEncoder(w).Encode([]map[string]interface{}{
			{"id": "page1", "type": "page", "url": "https://example.com",
				"webSocketDebuggerUrl": wsURL, "active": true},
		})
	})

	mux.HandleFunc("/json/version", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		wsURL := fmt.Sprintf("ws://%s/devtools/browser/test", r.Host)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"webSocketDebuggerUrl": wsURL,
		})
	})

	mux.HandleFunc("/ws", func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			return
		}
		defer conn.Close()
		serveCDP(t, conn, handler)
	})
	mux.HandleFunc("/devtools/", func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			return
		}
		defer conn.Close()
		serveCDP(t, conn, handler)
	})

	server := httptest.NewServer(mux)
	_, portStr, err := net.SplitHostPort(server.Listener.Addr().String())
	if err != nil {
		t.Fatal(err)
	}
	port, err = strconv.Atoi(portStr)
	if err != nil {
		t.Fatal(err)
	}
	return port, server.Close
}

func serveCDP(t *testing.T, conn *websocket.Conn, handler mockCDPHandler) {
	for {
		_, msg, err := conn.ReadMessage()
		if err != nil {
			return
		}
		var req struct {
			ID     int              `json:"id"`
			Method string           `json:"method"`
			Params json.RawMessage  `json:"params,omitempty"`
		}
		if err := json.Unmarshal(msg, &req); err != nil {
			continue
		}
		result := handler(req.Method, req.Params)
		if result == nil {
			result = map[string]interface{}{}
		}
		resp := map[string]interface{}{"id": req.ID, "result": result}
		if err := conn.WriteJSON(resp); err != nil {
			return
		}
	}
}

func defaultCDPHandler(method string, params json.RawMessage) map[string]interface{} {
	switch method {
	case "Runtime.evaluate":
		return map[string]interface{}{
			"result": map[string]interface{}{"type": "string", "value": ""},
		}
	default:
		return map[string]interface{}{}
	}
}

// ─── Mock Temp Email API Server ───────────────────────────────────────────────

func newMockEmailServer(t *testing.T, handler http.HandlerFunc) *httptest.Server {
	t.Helper()
	if handler == nil {
		handler = defaultEmailHandler
	}
	return httptest.NewServer(handler)
}

func defaultEmailHandler(w http.ResponseWriter, r *http.Request) {
	switch {
	case strings.HasSuffix(r.URL.Path, "/new_address"):
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]string{
			"jwt":     "test-jwt",
			"address": "testuser@tempmail.test",
		})
	case strings.HasSuffix(r.URL.Path, "/mails"):
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"results": []map[string]interface{}{
				{
					"id":      1,
					"subject": "Your DeepSeek verification code: 123456",
					"text":    "Your verification code is: 123456",
					"html":    "",
					"from":    "noreply@deepseek.com",
				},
			},
		})
	default:
		w.WriteHeader(http.StatusNotFound)
	}
}

// ─── Test 1: DeepSeekRegister Input Validation ────────────────────────────────

func TestDeepSeekRegisterInput(t *testing.T) {
	t.Run("empty profileID returns error", func(t *testing.T) {
		app := &App{}
		result := app.DeepSeekRegister(DeepSeekRegisterInput{ProfileID: "", Password: "MyPass123!"})
		if result.Error != "profileId is required" {
			t.Fatalf("expected 'profileId is required', got: %s", result.Error)
		}
	})

	t.Run("empty password auto-generates", func(t *testing.T) {
		emailServer := newMockEmailServer(t, func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(http.StatusInternalServerError)
			json.NewEncoder(w).Encode(map[string]string{"error": "mock fail"})
		})
		defer emailServer.Close()
		t.Setenv("TEMP_EMAIL_API_BASE", emailServer.URL)

		mgr := browser.NewManager(&config.Config{}, t.TempDir())
		mgr.Profiles = map[string]*browser.Profile{
			"test-profile": {ProfileId: "test-profile", Running: true, DebugReady: true, DebugPort: 9999},
		}
		app := &App{browserMgr: mgr}
		result := app.DeepSeekRegister(DeepSeekRegisterInput{ProfileID: "test-profile", Password: ""})
		if result.Error == "profileId is required" {
			t.Fatal("expected pipeline to start (error should NOT be profileId is required)")
		}
	})

	t.Run("valid input starts pipeline", func(t *testing.T) {
		emailServer := newMockEmailServer(t, nil)
		defer emailServer.Close()
		t.Setenv("TEMP_EMAIL_API_BASE", emailServer.URL)

		mgr := browser.NewManager(&config.Config{}, t.TempDir())
		mgr.Profiles = map[string]*browser.Profile{
			"test-profile": {ProfileId: "test-profile", Running: true, DebugReady: true, DebugPort: 9999},
		}
		app := &App{browserMgr: mgr}
		result := app.DeepSeekRegister(DeepSeekRegisterInput{ProfileID: "test-profile", Password: "MyPass123!"})
		if result.Error == "profileId is required" {
			t.Fatal("expected pipeline to start (valid input)")
		}
	})
}

// ─── Test 2: BizCode Mapping ──────────────────────────────────────────────────

func TestBizCodeMapping(t *testing.T) {
	tests := []struct {
		code     int
		expected bizCodeAction
	}{
		{0, bizRetrySame},
		{200, bizRetrySame},
		{1001, bizSwitchEmail},
		{1002, bizSwitchEmail},
		{1003, bizRetrySame},
		{1004, bizRetrySame},
		{1005, bizSwitchEmail},
		{2001, bizSwitchProxy},
		{2002, bizSwitchProxy},
		{2003, bizAbandon},
		{3001, bizAbandon},
		{3002, bizRetrySame},
		{4001, bizSwitchProxy},
		{4002, bizSwitchProxy},
		{5000, bizAbandon},
		{9999, bizAbandon},
		{-1, bizRetrySame},
		{123, bizRetrySame},
	}
	for _, tt := range tests {
		t.Run(fmt.Sprintf("code_%d", tt.code), func(t *testing.T) {
			got := classifyBizCode(tt.code)
			if got != tt.expected {
				t.Errorf("classifyBizCode(%d) = %v, want %v", tt.code, got, tt.expected)
			}
		})
	}
}

// ─── Test 3: AdaptationState ──────────────────────────────────────────────────

func TestAdaptationState(t *testing.T) {
	t.Run("consecutive fails tracking", func(t *testing.T) {
		a := &AdaptationState{}
		a.Update(RoundResult{})
		if a.ConsecutiveFails != 1 {
			t.Fatalf("expected 1 consecutive fail, got %d", a.ConsecutiveFails)
		}
		a.Update(RoundResult{})
		if a.ConsecutiveFails != 2 {
			t.Fatalf("expected 2 consecutive fails, got %d", a.ConsecutiveFails)
		}
	})

	t.Run("turnstile timeout counting", func(t *testing.T) {
		a := &AdaptationState{}
		a.Update(RoundResult{ErrorType: FailureTurnstile})
		a.Update(RoundResult{ErrorType: FailureTurnstile})
		if a.TurnstileTimeouts != 2 {
			t.Fatalf("expected 2 turnstile timeouts, got %d", a.TurnstileTimeouts)
		}
	})

	t.Run("success resets consecutive fails", func(t *testing.T) {
		a := &AdaptationState{}
		a.Update(RoundResult{})
		a.Update(RoundResult{})
		a.Update(RoundResult{Success: true, Proxy: ProxyInfo{ProxyID: "p1"}, Tier: proxy.Tier1TaiwanResidential})
		if a.ConsecutiveFails != 0 {
			t.Fatalf("expected 0 consecutive fails after success, got %d", a.ConsecutiveFails)
		}
	})

	t.Run("proxy blacklisting", func(t *testing.T) {
		a := &AdaptationState{}
		a.Update(RoundResult{ErrorType: FailureProxy, Proxy: ProxyInfo{ProxyID: "bad-proxy"}})
		if !a.isBlacklisted("bad-proxy") {
			t.Fatal("expected bad-proxy to be blacklisted")
		}
		if a.isBlacklisted("good-proxy") {
			t.Fatal("expected good-proxy to NOT be blacklisted")
		}
	})

	t.Run("best tier selection", func(t *testing.T) {
		a := &AdaptationState{BestTier: 0}
		a.Update(RoundResult{
			Success: true,
			Tier:    proxy.Tier1TaiwanResidential,
			Proxy:   ProxyInfo{ProxyID: "p1"},
		})
		if a.BestTier != proxy.Tier1TaiwanResidential {
			t.Fatalf("expected best tier 1, got %d", a.BestTier)
		}
		a.Update(RoundResult{
			Success: true,
			Tier:    proxy.Tier2LowLatency,
			Proxy:   ProxyInfo{ProxyID: "p2"},
		})
		if a.BestTier != proxy.Tier1TaiwanResidential {
			t.Fatalf("best tier should remain 1 (lowest), got %d", a.BestTier)
		}
	})
}

// ─── Test 4: DefaultBatchConfig ────────────────────────────────────────────────

func TestDefaultBatchConfig(t *testing.T) {
	cfg := DefaultBatchConfig()
	if cfg.TotalRounds != 5 {
		t.Errorf("TotalRounds = %d, want 5", cfg.TotalRounds)
	}
	if cfg.HumanizeLevel != humanize.LevelHigh {
		t.Errorf("HumanizeLevel = %v, want LevelHigh", cfg.HumanizeLevel)
	}
	if cfg.RoundDelayMin != 30*time.Second {
		t.Errorf("RoundDelayMin = %v, want 30s", cfg.RoundDelayMin)
	}
	if cfg.RoundDelayMax != 120*time.Second {
		t.Errorf("RoundDelayMax = %v, want 120s", cfg.RoundDelayMax)
	}
	if cfg.TurnstileTimeout != 90*time.Second {
		t.Errorf("TurnstileTimeout = %v, want 90s", cfg.TurnstileTimeout)
	}
	if cfg.CodePollTimeout != 3*time.Minute {
		t.Errorf("CodePollTimeout = %v, want 3m", cfg.CodePollTimeout)
	}
}

// ─── Test 5: DetermineTier ────────────────────────────────────────────────────

func TestDetermineTier(t *testing.T) {
	t.Run("rounds 1-5 map to correct tiers", func(t *testing.T) {
		adapt := &AdaptationState{BestTier: 0, SuccessfulProxies: nil}
		expected := []proxy.PriorityTier{
			proxy.Tier1TaiwanResidential,
			proxy.Tier2LowLatency,
			proxy.Tier2LowLatency,
			proxy.Tier1TaiwanResidential,
			proxy.Tier1TaiwanResidential,
		}
		for round := 1; round <= 5; round++ {
			tier := determineTier(round, adapt)
			if tier != expected[round-1] {
				t.Errorf("round %d: got tier %d, want %d", round, tier, expected[round-1])
			}
		}
	})

	t.Run("turnstile timeouts >= 2 forces tier 1", func(t *testing.T) {
		adapt := &AdaptationState{TurnstileTimeouts: 2, BestTier: 0}
		tier := determineTier(2, adapt)
		if tier != proxy.Tier1TaiwanResidential {
			t.Errorf("expected Tier1 with 2+ turnstile timeouts, got %d", tier)
		}
	})

	t.Run("round > 5 returns Tier4", func(t *testing.T) {
		adapt := &AdaptationState{BestTier: 0}
		tier := determineTier(6, adapt)
		if tier != proxy.Tier4AnyNonCN {
			t.Errorf("expected Tier4 for round > 5, got %d", tier)
		}
	})

	t.Run("uses best tier with successful proxies", func(t *testing.T) {
		adapt := &AdaptationState{
			BestTier:          proxy.Tier1TaiwanResidential,
			SuccessfulProxies: []ProxyInfo{{ProxyID: "p1"}},
		}
		tier := determineTier(4, adapt)
		if tier != proxy.Tier1TaiwanResidential {
			t.Errorf("expected best tier for round 4, got %d", tier)
		}
	})
}

// ─── Test 6: ComputeRoundDelay ─────────────────────────────────────────────────

func TestComputeRoundDelay(t *testing.T) {
	cfg := BatchConfig{
		RoundDelayMin: 30 * time.Second,
		RoundDelayMax: 120 * time.Second,
	}

	t.Run("base delay without failures", func(t *testing.T) {
		adapt := &AdaptationState{}
		delay := computeRoundDelay(adapt, cfg)
		if delay < 30*time.Second || delay > 120*time.Second {
			t.Errorf("base delay %v out of range [30s, 120s]", delay)
		}
	})

	t.Run("consecutive fails >= 2 doubles delay", func(t *testing.T) {
		adapt := &AdaptationState{ConsecutiveFails: 2}
		delay := computeRoundDelay(adapt, cfg)
		if delay < 60*time.Second {
			t.Errorf("expected doubled delay (%v) >= 60s", delay)
		}
	})

	t.Run("WAF blocks add 1.5x multiplier", func(t *testing.T) {
		adapt := &AdaptationState{WAFBlocks: 1, ConsecutiveFails: 2}
		delay := computeRoundDelay(adapt, cfg)
		if delay < 90*time.Second {
			t.Errorf("expected 3x delay (fail*2 * waf*1.5), got %v", delay)
		}
	})

	t.Run("delay capped at maxDelay * 2", func(t *testing.T) {
		cfg2 := BatchConfig{RoundDelayMin: 100 * time.Second, RoundDelayMax: 101 * time.Second}
		adapt := &AdaptationState{ConsecutiveFails: 2, WAFBlocks: 1}
		delay := computeRoundDelay(adapt, cfg2)
		expectedMax := cfg2.RoundDelayMax * 2
		if delay > expectedMax {
			t.Errorf("delay %v exceeds cap %v", delay, expectedMax)
		}
	})
}

// ─── Test 7: ResolveProfileCDPPort ─────────────────────────────────────────────

func TestResolveProfileCDPPort(t *testing.T) {
	t.Run("profile found and debug ready returns port", func(t *testing.T) {
		mgr := browser.NewManager(&config.Config{}, t.TempDir())
		mgr.Profiles = map[string]*browser.Profile{
			"prof1": {ProfileId: "prof1", Running: true, DebugReady: true, DebugPort: 12345},
		}
		port, err := resolveProfileCDPPort(mgr, "prof1")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if port != 12345 {
			t.Fatalf("expected port 12345, got %d", port)
		}
	})

	t.Run("profile not found returns error", func(t *testing.T) {
		mgr := browser.NewManager(&config.Config{}, t.TempDir())
		_, err := resolveProfileCDPPort(mgr, "nonexistent")
		if err == nil {
			t.Fatal("expected error for nonexistent profile")
		}
	})

	t.Run("profile not running returns error", func(t *testing.T) {
		mgr := browser.NewManager(&config.Config{}, t.TempDir())
		mgr.Profiles = map[string]*browser.Profile{
			"prof1": {ProfileId: "prof1", Running: false, DebugReady: false},
		}
		_, err := resolveProfileCDPPort(mgr, "prof1")
		if err == nil {
			t.Fatal("expected error for non-running profile")
		}
	})
}

// ─── Test 8: ParseIPHealth ────────────────────────────────────────────────────

func TestParseIPHealth(t *testing.T) {
	t.Run("empty string returns nil", func(t *testing.T) {
		result := parseIPHealth("")
		if result != nil {
			t.Fatal("expected nil for empty string")
		}
	})

	t.Run("valid JSON parses correctly", func(t *testing.T) {
		json := `{"country":"US","countryCode":"US","fraudScore":12.5,"isResidential":true}`
		result := parseIPHealth(json)
		if result == nil {
			t.Fatal("expected non-nil result")
		}
		if result.Country != "US" || result.CountryCode != "US" {
			t.Errorf("country mismatch: %s / %s", result.Country, result.CountryCode)
		}
		if result.FraudScore != 12.5 {
			t.Errorf("fraudScore = %f, want 12.5", result.FraudScore)
		}
		if !result.IsResidential {
			t.Error("expected isResidential = true")
		}
	})

	t.Run("missing fields handled gracefully", func(t *testing.T) {
		result := parseIPHealth(`{"country":"DE"}`)
		if result == nil {
			t.Fatal("expected non-nil result")
		}
		if result.Country != "DE" {
			t.Errorf("country = %s, want DE", result.Country)
		}
		if result.FraudScore != 0 {
			t.Errorf("expected fraudScore=0 for missing field, got %f", result.FraudScore)
		}
	})

	t.Run("empty object returns nil", func(t *testing.T) {
		result := parseIPHealth("{}")
		if result != nil {
			t.Fatal("expected nil for empty object")
		}
	})
}

// ─── Test 10: AppendToDesktopTxt ───────────────────────────────────────────────

func TestAppendToDesktopTxt(t *testing.T) {
	if runtime.GOOS == "windows" {
		tmpDir := t.TempDir()
		desktopDir := filepath.Join(tmpDir, "Desktop")
		if err := os.MkdirAll(desktopDir, 0755); err != nil {
			t.Fatal(err)
		}
		t.Setenv("USERPROFILE", tmpDir)

		t.Run("file created with correct content", func(t *testing.T) {
			err := appendToDesktopTxt("test@example.com", "TestPass123!", "sk-test-api-key")
			if err != nil {
				t.Fatal(err)
			}
			encPath := filepath.Join(desktopDir, "DeepSeek_Accounts.enc")
			if _, err := os.Stat(encPath); os.IsNotExist(err) {
				t.Fatal("encrypted file was not created")
			}
		})

		t.Run("XOR obfuscation is reversible", func(t *testing.T) {
			encPath := filepath.Join(desktopDir, "DeepSeek_Accounts.enc")
			data, err := os.ReadFile(encPath)
			if err != nil {
				t.Fatal(err)
			}
			const xorKey = byte(0xAB)
			decrypted := make([]byte, len(data))
			for i, b := range data {
				decrypted[i] = b ^ xorKey
			}
			content := string(decrypted)
			if !strings.Contains(content, "test@example.com") ||
				!strings.Contains(content, "TestPass123!") ||
				!strings.Contains(content, "sk-test-api-key") {
				t.Errorf("decrypted content missing expected fields: %s", content)
			}
		})

		t.Run("append mode works", func(t *testing.T) {
			err := appendToDesktopTxt("user2@test.com", "Pass456!", "sk-key-2")
			if err != nil {
				t.Fatal(err)
			}
			encPath := filepath.Join(desktopDir, "DeepSeek_Accounts.enc")
			data, err := os.ReadFile(encPath)
			if err != nil {
				t.Fatal(err)
			}
			const xorKey = byte(0xAB)
			decrypted := make([]byte, len(data))
			for i, b := range data {
				decrypted[i] = b ^ xorKey
			}
			content := string(decrypted)
			if !strings.Contains(content, "test@example.com") {
				t.Error("first entry missing after append")
			}
			if !strings.Contains(content, "user2@test.com") {
				t.Error("second entry missing after append")
			}
		})
	} else {
		t.Skip("AppendToDesktopTxt is Windows-only (uses USERPROFILE)")
	}
}

// ─── Test 11: CDP Helper Functions ─────────────────────────────────────────────

func TestCDPHelperFunctions(t *testing.T) {
	t.Run("connectCDPExecutor creates executor correctly", func(t *testing.T) {
		port, close := newMockCDPServer(t, nil)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor failed: %v", err)
		}
		if executor == nil {
			t.Fatal("expected non-nil executor")
		}
		executor.Close()
	})

	t.Run("navigatePageCDP sends Page.navigate", func(t *testing.T) {
		var called bool
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Page.navigate" {
				called = true
			}
			return defaultCDPHandler(method, params)
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		err := navigatePageCDP(port, "https://example.com/test")
		if err != nil {
			t.Fatalf("navigatePageCDP failed: %v", err)
		}
		if !called {
			t.Fatal("expected Page.navigate to be called")
		}
	})

	t.Run("waitForTurnstileToken finds token in DOM", func(t *testing.T) {
		var evalCount int
		var mu sync.Mutex
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				mu.Lock()
				evalCount++
				mu.Unlock()
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": "0.mock-turnstile-token"},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		token, err := waitForTurnstileToken(executor, 5*time.Second)
		if err != nil {
			t.Fatalf("waitForTurnstileToken: %v", err)
		}
		if token != "0.mock-turnstile-token" {
			t.Fatalf("expected turnstile token, got %q", token)
		}
	})

	t.Run("waitForTurnstileToken times out", func(t *testing.T) {
		var mu sync.Mutex
		var evalCount int
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				mu.Lock()
				evalCount++
				mu.Unlock()
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": ""},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		_, err = waitForTurnstileToken(executor, 1*time.Millisecond)
		if err == nil {
			t.Fatal("expected timeout error")
		}
		if !strings.Contains(err.Error(), "timeout") {
			t.Fatalf("expected timeout error, got: %v", err)
		}
	})

	t.Run("clickFormButton finds button and sends CDP mouse events", func(t *testing.T) {
		var mouseEvents []string
		var mu sync.Mutex
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			mu.Lock()
			if method == "Input.dispatchMouseEvent" {
				var p struct {
					Type string `json:"type"`
				}
				json.Unmarshal(params, &p)
				mouseEvents = append(mouseEvents, p.Type)
			}
			mu.Unlock()

			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{
						"type": "string",
						"value": `{"x":500,"y":300,"label":"sign up"}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = clickFormButton(executor)
		if err != nil {
			t.Fatalf("clickFormButton: %v", err)
		}

		mu.Lock()
		hasMousePressed := false
		hasMouseReleased := false
		for _, ev := range mouseEvents {
			if ev == "mousePressed" {
				hasMousePressed = true
			}
			if ev == "mouseReleased" {
				hasMouseReleased = true
			}
		}
		mu.Unlock()
		if !hasMousePressed {
			t.Error("expected mousePressed event")
		}
		if !hasMouseReleased {
			t.Error("expected mouseReleased event")
		}
	})
}

// ─── Test: WaitForNavigation and WaitForPageReady ──────────────────────────────

func TestWaitHelpers(t *testing.T) {
	t.Run("waitForNavigation success", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": "true"},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = waitForNavigation(executor, 5*time.Second)
		if err != nil {
			t.Fatalf("waitForNavigation: %v", err)
		}
	})

	t.Run("waitForNavigation timeout", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": "false"},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = waitForNavigation(executor, 50*time.Millisecond)
		if err == nil {
			t.Fatal("expected timeout error")
		}
	})

	t.Run("waitForPageReady success", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": "true"},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = waitForPageReady(executor, 5*time.Second)
		if err != nil {
			t.Fatalf("waitForPageReady: %v", err)
		}
	})

	t.Run("waitForPageReady timeout", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": "false"},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = waitForPageReady(executor, 50*time.Millisecond)
		if err == nil {
			t.Fatal("expected timeout error")
		}
	})
}

// ─── Test: SendVerificationCode, CheckEmailCode, RegisterUser ──────────────────

func TestAPICalls(t *testing.T) {
	t.Run("sendVerificationCode succeeds", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{
						"type":  "string",
						"value": `{"status":200,"bizCode":0,"raw":"{}"}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = sendVerificationCode(executor, "test@example.com", "mock-token")
		if err != nil {
			t.Fatalf("sendVerificationCode: %v", err)
		}
	})

	t.Run("sendVerificationCode bizCode error", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{
						"type":  "string",
						"value": `{"status":200,"bizCode":1001,"raw":"email exists"}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = sendVerificationCode(executor, "test@example.com", "mock-token")
		if err == nil {
			t.Fatal("expected bizCode error")
		}
	})

	t.Run("checkEmailCode succeeds", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{
						"type":  "string",
						"value": `{"status":200,"bizCode":0}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = checkEmailCode(executor, "test@example.com", "123456")
		if err != nil {
			t.Fatalf("checkEmailCode: %v", err)
		}
	})

	t.Run("registerUser succeeds", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{
						"type":  "string",
						"value": `{"status":200,"bizCode":0,"token":"sk-test-token"}`,
					},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		err = registerUser(executor, "test@example.com", "Pass123!", "123456")
		if err != nil {
			t.Fatalf("registerUser: %v", err)
		}
	})
}

// ─── Test: ClickByTextJS ───────────────────────────────────────────────────────

func TestClickByTextJS(t *testing.T) {
	var expression string
	handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
		if method == "Runtime.evaluate" {
			var p struct {
				Expression string `json:"expression"`
			}
			json.Unmarshal(params, &p)
			expression = p.Expression
			return map[string]interface{}{
				"result": map[string]interface{}{"type": "string", "value": "true"},
			}
		}
		return map[string]interface{}{}
	})
	port, close := newMockCDPServer(t, handler)
	defer close()

	executor, err := connectCDPExecutor(port)
	if err != nil {
		t.Fatalf("connectCDPExecutor: %v", err)
	}
	defer executor.Close()

	err = clickByTextJS(executor, "Create new key")
	if err != nil {
		t.Fatalf("clickByTextJS: %v", err)
	}
	if !strings.Contains(expression, "Create new key") {
		t.Fatalf("expected expression to contain 'Create new key', got: %s", expression)
	}
}

// ─── Test: ExtractAPIKey ───────────────────────────────────────────────────────

func TestExtractAPIKey(t *testing.T) { // skipcq: GO-S1065
	t.Run("finds API key in page text", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": "sk-test-api-key-found"},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		key, err := extractAPIKey(executor)
		if err != nil {
			t.Fatalf("extractAPIKey: %v", err)
		}
		if key != "sk-test-api-key-found" {
			t.Fatalf("expected 'sk-test-api-key-found', got %q", key)
		}
	})

	t.Run("extractAPIKeyFromBody", func(t *testing.T) {
		handler := mockCDPHandler(func(method string, params json.RawMessage) map[string]interface{} {
			if method == "Runtime.evaluate" {
				return map[string]interface{}{
					"result": map[string]interface{}{"type": "string", "value": "sk-body-key"},
				}
			}
			return map[string]interface{}{}
		})
		port, close := newMockCDPServer(t, handler)
		defer close()

		executor, err := connectCDPExecutor(port)
		if err != nil {
			t.Fatalf("connectCDPExecutor: %v", err)
		}
		defer executor.Close()

		key, err := extractAPIKeyFromBody(executor)
		if err != nil {
			t.Fatalf("extractAPIKeyFromBody: %v", err)
		}
		if key != "sk-body-key" {
			t.Fatalf("expected 'sk-body-key', got %q", key)
		}
	})
}

// ─── Test: SleepCtx ────────────────────────────────────────────────────────────

func TestSleepCtx(t *testing.T) {
	ctx := context.Background()
	if !sleepCtx(ctx, 1*time.Millisecond) {
		t.Fatal("expected sleep to return true")
	}

	cancelledCtx, cancel := context.WithCancel(context.Background())
	cancel()
	if sleepCtx(cancelledCtx, 1*time.Hour) {
		t.Fatal("expected cancelled context to return false")
	}
}
