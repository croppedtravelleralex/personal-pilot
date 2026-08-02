package captcha

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"sync"
	"testing"
	"time"
)

type mockSolver struct {
	name    string
	solveFn func(ctx context.Context, req *SolveRequest) (*SolveResult, error)
}

func (m *mockSolver) Name() string { return m.name }
func (m *mockSolver) Solve(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
	return m.solveFn(ctx, req)
}
func (m *mockSolver) GetBalance(ctx context.Context) (float64, error) {
	return 100.0, nil
}

func TestNewManager(t *testing.T) {
	cfg := &Config{CacheTTL: 60 * time.Second}
	m := NewManager(cfg)
	if m == nil {
		t.Fatal("NewManager returned nil")
	}
	if m.metrics == nil {
		t.Fatal("metrics not initialized")
	}
	if m.cache == nil {
		t.Fatal("cache not initialized")
	}
}

func TestManagerSolveWithMockSolver(t *testing.T) {
	cfg := &Config{CacheTTL: 60 * time.Second}
	m := NewManager(cfg)

	expected := &SolveResult{Text: "ABC123", Provider: "mock", Cost: 0.001}
	m.AddSolver(&mockSolver{
		name: "mock",
		solveFn: func(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
			return expected, nil
		},
	})

	result, err := m.Solve(context.Background(), &SolveRequest{Type: CaptchaImage})
	if err != nil {
		t.Fatalf("Solve returned error: %v", err)
	}
	if result.Text != "ABC123" {
		t.Fatalf("result.Text = %q, want %q", result.Text, "ABC123")
	}

	metrics := m.GetMetrics()
	if metrics.TotalSolved != 1 {
		t.Fatalf("TotalSolved = %d, want 1", metrics.TotalSolved)
	}
	if metrics.TotalCost != 0.001 {
		t.Fatalf("TotalCost = %f, want 0.001", metrics.TotalCost)
	}
}

func TestManagerSolveAllSolversFail(t *testing.T) {
	cfg := &Config{CacheTTL: 60 * time.Second}
	m := NewManager(cfg)

	m.AddSolver(&mockSolver{
		name: "fail1",
		solveFn: func(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
			return nil, fmt.Errorf("solver1 failed")
		},
	})
	m.AddSolver(&mockSolver{
		name: "fail2",
		solveFn: func(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
			return nil, fmt.Errorf("solver2 failed")
		},
	})

	_, err := m.Solve(context.Background(), &SolveRequest{Type: CaptchaImage})
	if err == nil {
		t.Fatal("expected error, got nil")
	}

	metrics := m.GetMetrics()
	if metrics.TotalFailed != 2 {
		t.Fatalf("TotalFailed = %d, want 2", metrics.TotalFailed)
	}
}

func TestManagerSolveWithFallback(t *testing.T) {
	cfg := &Config{CacheTTL: 60 * time.Second}
	m := NewManager(cfg)

	expected := &SolveResult{Text: "FALLBACK", Provider: "fallback", Cost: 0.002}
	m.AddSolver(&mockSolver{
		name: "primary",
		solveFn: func(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
			return nil, fmt.Errorf("primary failed")
		},
	})
	m.AddSolver(&mockSolver{
		name: "fallback",
		solveFn: func(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
			return expected, nil
		},
	})

	result, err := m.Solve(context.Background(), &SolveRequest{Type: CaptchaImage})
	if err != nil {
		t.Fatalf("Solve returned error: %v", err)
	}
	if result.Text != "FALLBACK" {
		t.Fatalf("result.Text = %q, want %q", result.Text, "FALLBACK")
	}

	metrics := m.GetMetrics()
	if metrics.TotalSolved != 1 {
		t.Fatalf("TotalSolved = %d, want 1", metrics.TotalSolved)
	}
	if metrics.TotalFailed != 1 {
		t.Fatalf("TotalFailed = %d, want 1", metrics.TotalFailed)
	}
}

func TestManagerSolveCacheHit(t *testing.T) {
	cfg := &Config{CacheTTL: 60 * time.Second}
	m := NewManager(cfg)

	callCount := 0
	m.AddSolver(&mockSolver{
		name: "mock",
		solveFn: func(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
			callCount++
			return &SolveResult{Text: "CACHED", Provider: "mock", Cost: 0.001}, nil
		},
	})

	req := &SolveRequest{Type: CaptchaImage, SiteKey: "sk1", PageURL: "https://example.com"}
	result1, err := m.Solve(context.Background(), req)
	if err != nil {
		t.Fatalf("first Solve error: %v", err)
	}
	if callCount != 1 {
		t.Fatalf("callCount after first Solve = %d, want 1", callCount)
	}

	result2, err := m.Solve(context.Background(), req)
	if err != nil {
		t.Fatalf("second Solve error: %v", err)
	}
	if callCount != 1 {
		t.Fatalf("callCount after second Solve = %d, want 1 (cache hit)", callCount)
	}

	if result1.Text != result2.Text {
		t.Fatalf("cached result mismatch: %q vs %q", result1.Text, result2.Text)
	}
}

func TestManagerSolveCacheKeyVariesByType(t *testing.T) {
	cfg := &Config{CacheTTL: 60 * time.Second}
	m := NewManager(cfg)

	callCount := 0
	m.AddSolver(&mockSolver{
		name: "mock",
		solveFn: func(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
			callCount++
			return &SolveResult{Text: string(req.Type), Provider: "mock", Cost: 0.001}, nil
		},
	})

	req1 := &SolveRequest{Type: CaptchaImage, SiteKey: "sk1", PageURL: "https://example.com"}
	req2 := &SolveRequest{Type: CaptchaTurnstile, SiteKey: "sk1", PageURL: "https://example.com"}

	_, _ = m.Solve(context.Background(), req1)
	_, _ = m.Solve(context.Background(), req2)

	if callCount != 2 {
		t.Fatalf("callCount = %d, want 2 (different types should not cache-hit)", callCount)
	}
}

func TestGetMetrics(t *testing.T) {
	cfg := &Config{CacheTTL: 60 * time.Second}
	m := NewManager(cfg)

	metrics := m.GetMetrics()
	if metrics.TotalSolved != 0 || metrics.TotalFailed != 0 {
		t.Fatal("fresh metrics should be zero")
	}

	m.AddSolver(&mockSolver{
		name: "mock",
		solveFn: func(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
			return &SolveResult{Text: "OK", Provider: "mock", Cost: 0.001}, nil
		},
	})

	m.Solve(context.Background(), &SolveRequest{Type: CaptchaImage})
	metrics = m.GetMetrics()

	if metrics.TotalSolved != 1 {
		t.Fatalf("TotalSolved = %d, want 1", metrics.TotalSolved)
	}
	if metrics.TotalCost != 0.001 {
		t.Fatalf("TotalCost = %f, want 0.001", metrics.TotalCost)
	}
	if metrics.ByProvider["mock"] == nil || metrics.ByProvider["mock"].Solved != 1 {
		t.Fatalf("mock provider stats not recorded correctly")
	}

	// Verify snapshot isolation - modifying returned metrics shouldn't affect internal state
	metrics.TotalSolved = 999
	metrics.ByProvider["mock"] = &ProviderStats{Solved: 999}

	metrics2 := m.GetMetrics()
	if metrics2.TotalSolved != 1 {
		t.Fatalf("metrics not isolated: TotalSolved = %d, want 1", metrics2.TotalSolved)
	}
}

func TestResultCacheExpiry(t *testing.T) {
	cache := newResultCache(100 * time.Millisecond)

	result := &SolveResult{Text: "test", Provider: "mock"}
	cache.Set("key1", result)

	if cached, ok := cache.Get("key1"); !ok {
		t.Fatal("should find cached result immediately")
	} else if cached.Text != "test" {
		t.Fatalf("cached.Text = %q, want %q", cached.Text, "test")
	}

	time.Sleep(150 * time.Millisecond)

	if _, ok := cache.Get("key1"); ok {
		t.Fatal("cached result should have expired")
	}
}

func TestResultCacheDefaultTTL(t *testing.T) {
	cache := newResultCache(0)
	if cache.ttl != 60*time.Second {
		t.Fatalf("default TTL = %v, want 60s", cache.ttl)
	}
}

func TestCacheKeyFromRequest(t *testing.T) {
	req1 := &SolveRequest{
		Type:      CaptchaImage,
		SiteKey:   "sk1",
		PageURL:   "https://example.com",
		ImageData: []byte{1, 2, 3},
	}
	req2 := &SolveRequest{
		Type:      CaptchaImage,
		SiteKey:   "sk1",
		PageURL:   "https://example.com",
		ImageData: []byte{1, 2, 3},
	}
	req3 := &SolveRequest{
		Type:      CaptchaImage,
		SiteKey:   "sk2",
		PageURL:   "https://example.com",
		ImageData: []byte{1, 2, 3},
	}

	k1 := cacheKeyFromRequest(req1)
	k2 := cacheKeyFromRequest(req2)
	k3 := cacheKeyFromRequest(req3)

	if k1 != k2 {
		t.Fatal("identical requests should produce same cache key")
	}
	if k1 == k3 {
		t.Fatal("different sitekey should produce different cache key")
	}
}

func TestCapsolverSolver(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Content-Type") != "application/json" {
			t.Fatalf("unexpected Content-Type: %s", r.Header.Get("Content-Type"))
		}

		var req map[string]any
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			t.Fatalf("decode request: %v", err)
		}

		clientKey, _ := req["clientKey"].(string)
		if clientKey != "test-key" {
			t.Fatalf("clientKey = %q, want %q", clientKey, "test-key")
		}

		switch r.URL.Path {
		case "/createTask":
			task, _ := req["task"].(map[string]any)
			if task == nil {
				t.Fatal("missing task in request")
			}
			taskType, _ := task["type"].(string)
			if taskType != "ImageToTextTask" {
				t.Fatalf("task type = %q, want %q", taskType, "ImageToTextTask")
			}
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"taskId":  "capsolver-task-123",
			})

		case "/getTaskResult":
			taskId, _ := req["taskId"].(string)
			if taskId != "capsolver-task-123" {
				t.Fatalf("taskId = %q, want %q", taskId, "capsolver-task-123")
			}
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"status":  "ready",
				"solution": map[string]any{
					"text": "XYZ789",
					"cost": 0.0008,
				},
			})

		default:
			http.NotFound(w, r)
		}
	}))
	defer server.Close()

	solver := NewCapsolverSolver("test-key", 30*time.Second)
	solver.baseURL = server.URL

	result, err := solver.Solve(context.Background(), &SolveRequest{
		Type:      CaptchaImage,
		ImageData: []byte{1, 2, 3, 4, 5},
	})
	if err != nil {
		t.Fatalf("Capsolver Solve error: %v", err)
	}
	if result.Text != "XYZ789" {
		t.Fatalf("result.Text = %q, want %q", result.Text, "XYZ789")
	}
	if result.Provider != "capsolver" {
		t.Fatalf("result.Provider = %q, want %q", result.Provider, "capsolver")
	}
}

func TestCapsolverSolverPollingTimeout(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{
			"errorId": 0,
			"status":  "processing",
		})
	}))
	defer server.Close()

	solver := NewCapsolverSolver("test-key", 5*time.Second)
	solver.baseURL = server.URL

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	_, err := solver.Solve(ctx, &SolveRequest{
		Type:      CaptchaImage,
		ImageData: []byte{1, 2, 3},
	})
	if err == nil {
		t.Fatal("expected timeout error, got nil")
	}
}

func TestCapsolverSolverCreateTaskError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{
			"errorId":          1,
			"errorCode":        "ERROR_INVALID_TASK",
			"errorDescription": "Invalid task parameters",
		})
	}))
	defer server.Close()

	solver := NewCapsolverSolver("test-key", 30*time.Second)
	solver.baseURL = server.URL

	_, err := solver.Solve(context.Background(), &SolveRequest{
		Type:      CaptchaImage,
		ImageData: []byte{1, 2, 3},
	})
	if err == nil {
		t.Fatal("expected error from createTask, got nil")
	}
}

func TestCapsolverSolverImageToTextNeedsData(t *testing.T) {
	solver := NewCapsolverSolver("test-key", 30*time.Second)

	_, err := solver.Solve(context.Background(), &SolveRequest{
		Type: CaptchaImage,
	})
	if err == nil {
		t.Fatal("expected error for missing image data, got nil")
	}
}

func TestCapsolverSolverReCaptchaV2(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var req map[string]any
		json.NewDecoder(r.Body).Decode(&req)

		if r.URL.Path == "/createTask" {
			task, _ := req["task"].(map[string]any)
			if task["type"] != "ReCaptchaV2Task" {
				t.Fatalf("unexpected task type: %v", task["type"])
			}
			if task["websiteKey"] != "6Le-test-key" {
				t.Fatalf("websiteKey = %v", task["websiteKey"])
			}
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"taskId":  "recaptcha-task-1",
			})
		} else {
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"status":  "ready",
				"solution": map[string]any{
					"gRecaptchaResponse": "recaptcha-token-abc",
				},
			})
		}
	}))
	defer server.Close()

	solver := NewCapsolverSolver("test-key", 30*time.Second)
	solver.baseURL = server.URL

	result, err := solver.Solve(context.Background(), &SolveRequest{
		Type:    CaptchaReCaptcha,
		SiteKey: "6Le-test-key",
		PageURL: "https://example.com",
	})
	if err != nil {
		t.Fatalf("Solve error: %v", err)
	}
	if result.Token != "recaptcha-token-abc" {
		t.Fatalf("result.Token = %q, want %q", result.Token, "recaptcha-token-abc")
	}
}

func TestCapsolverSolverTurnstile(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var req map[string]any
		json.NewDecoder(r.Body).Decode(&req)

		if r.URL.Path == "/createTask" {
			task, _ := req["task"].(map[string]any)
			if task["type"] != "AntiTurnstileTaskProxyLess" {
				t.Fatalf("unexpected task type: %v", task["type"])
			}
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"taskId":  "turnstile-task-1",
			})
		} else {
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"status":  "ready",
				"solution": map[string]any{
					"token": "turnstile-token-xyz",
				},
			})
		}
	}))
	defer server.Close()

	solver := NewCapsolverSolver("test-key", 30*time.Second)
	solver.baseURL = server.URL

	result, err := solver.Solve(context.Background(), &SolveRequest{
		Type:    CaptchaTurnstile,
		SiteKey: "0x4AAAAAAAFnTpB",
		PageURL: "https://example.com/login",
	})
	if err != nil {
		t.Fatalf("Solve error: %v", err)
	}
	if result.Token != "turnstile-token-xyz" {
		t.Fatalf("result.Token = %q, want %q", result.Token, "turnstile-token-xyz")
	}
}

func TestCapsolverSolverHCaptcha(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var req map[string]any
		json.NewDecoder(r.Body).Decode(&req)

		if r.URL.Path == "/createTask" {
			task, _ := req["task"].(map[string]any)
			if task["type"] != "HCaptchaTask" {
				t.Fatalf("unexpected task type: %v", task["type"])
			}
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"taskId":  "hcaptcha-task-1",
			})
		} else {
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"status":  "ready",
				"solution": map[string]any{
					"gRecaptchaResponse": "hcaptcha-token-abc",
				},
			})
		}
	}))
	defer server.Close()

	solver := NewCapsolverSolver("test-key", 30*time.Second)
	solver.baseURL = server.URL

	result, err := solver.Solve(context.Background(), &SolveRequest{
		Type:    CaptchaHCaptcha,
		SiteKey: "hcaptcha-site-key",
		PageURL: "https://example.com",
	})
	if err != nil {
		t.Fatalf("Solve error: %v", err)
	}
	if result.Token != "hcaptcha-token-abc" {
		t.Fatalf("result.Token = %q, want %q", result.Token, "hcaptcha-token-abc")
	}
}

func TestCapsolverSolverGeeTest(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var req map[string]any
		json.NewDecoder(r.Body).Decode(&req)

		if r.URL.Path == "/createTask" {
			task, _ := req["task"].(map[string]any)
			if task["type"] != "GeeTestTask" {
				t.Fatalf("unexpected task type: %v", task["type"])
			}
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"taskId":  "geetest-task-1",
			})
		} else {
			json.NewEncoder(w).Encode(map[string]any{
				"errorId": 0,
				"status":  "ready",
				"solution": map[string]any{
					"token": "geetest-token",
				},
			})
		}
	}))
	defer server.Close()

	solver := NewCapsolverSolver("test-key", 30*time.Second)
	solver.baseURL = server.URL

	result, err := solver.Solve(context.Background(), &SolveRequest{
		Type:    CaptchaGeeTest,
		SiteKey: "gt-key",
		PageURL: "https://example.com",
		Options: map[string]any{
			"gt":        "gt-key",
			"challenge": "challenge-value",
		},
	})
	if err != nil {
		t.Fatalf("Solve error: %v", err)
	}
	if result.Token != "geetest-token" {
		t.Fatalf("result.Token = %q, want %q", result.Token, "geetest-token")
	}
}

func TestCapsolverSolverUnsupportedType(t *testing.T) {
	solver := NewCapsolverSolver("test-key", 30*time.Second)
	_, err := solver.Solve(context.Background(), &SolveRequest{
		Type: CaptchaFun,
	})
	if err == nil {
		t.Fatal("expected error for unsupported type, got nil")
	}
}

func TestCapsolverSolverGetBalance(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var req map[string]any
		json.NewDecoder(r.Body).Decode(&req)
		if req["clientKey"] != "test-key" {
			t.Fatalf("clientKey = %v", req["clientKey"])
		}
		json.NewEncoder(w).Encode(map[string]any{
			"errorId": 0,
			"balance": 42.5,
		})
	}))
	defer server.Close()

	solver := NewCapsolverSolver("test-key", 30*time.Second)
	solver.baseURL = server.URL

	bal, err := solver.GetBalance(context.Background())
	if err != nil {
		t.Fatalf("GetBalance error: %v", err)
	}
	if bal != 42.5 {
		t.Fatalf("balance = %f, want 42.5", bal)
	}
}

func TestTwoCaptchaSolverImage(t *testing.T) {
	var mu sync.Mutex
	var submitted bool
	captchaID := "2captcha-id-456"

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		defer mu.Unlock()

		switch r.URL.Path {
		case "/in.php":
			if err := r.ParseForm(); err != nil {
				t.Fatalf("parse form: %v", err)
			}
			if r.Form.Get("method") != "base64" {
				t.Fatalf("method = %q", r.Form.Get("method"))
			}
			if r.Form.Get("body") == "" {
				t.Fatal("body is empty")
			}
			submitted = true
			json.NewEncoder(w).Encode(map[string]any{
				"status":  1,
				"request": captchaID,
			})

		case "/res.php":
			id := r.URL.Query().Get("id")
			if id != captchaID {
				t.Fatalf("id = %q, want %q", id, captchaID)
			}
			action := r.URL.Query().Get("action")
			if action == "get" {
				json.NewEncoder(w).Encode(map[string]any{
					"status":  1,
					"request": "ABCDE",
				})
			}

		default:
			http.NotFound(w, r)
		}
	}))
	defer server.Close()

	solver := NewTwoCaptchaSolver("test-key-2c", 30*time.Second)
	solver.baseURL = server.URL

	result, err := solver.Solve(context.Background(), &SolveRequest{
		Type:      CaptchaImage,
		ImageData: []byte{10, 20, 30},
	})
	if err != nil {
		t.Fatalf("2Captcha Solve error: %v", err)
	}
	if !submitted {
		t.Fatal("captcha was not submitted")
	}
	if result.Text != "ABCDE" {
		t.Fatalf("result.Text = %q, want %q", result.Text, "ABCDE")
	}
	if result.Provider != "2captcha" {
		t.Fatalf("result.Provider = %q, want %q", result.Provider, "2captcha")
	}
}

func TestTwoCaptchaSolverReCaptcha(t *testing.T) {
	var mu sync.Mutex
	captchaID := "2captcha-rec-789"

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		defer mu.Unlock()

		switch r.URL.Path {
		case "/in.php":
			r.ParseForm()
			if r.Form.Get("method") != "userrecaptcha" {
				t.Fatalf("method = %q", r.Form.Get("method"))
			}
			if r.Form.Get("googlekey") != "6Le-test" {
				t.Fatalf("googlekey = %q", r.Form.Get("googlekey"))
			}
			json.NewEncoder(w).Encode(map[string]any{
				"status":  1,
				"request": captchaID,
			})

		case "/res.php":
			if r.URL.Query().Get("id") == captchaID {
				json.NewEncoder(w).Encode(map[string]any{
					"status":  1,
					"request": "recaptcha-token-2c",
				})
			}

		default:
			http.NotFound(w, r)
		}
	}))
	defer server.Close()

	solver := NewTwoCaptchaSolver("test-key-2c", 30*time.Second)
	solver.baseURL = server.URL

	result, err := solver.Solve(context.Background(), &SolveRequest{
		Type:    CaptchaReCaptcha,
		SiteKey: "6Le-test",
		PageURL: "https://example.com",
	})
	if err != nil {
		t.Fatalf("2Captcha Solve error: %v", err)
	}
	if result.Token != "recaptcha-token-2c" {
		t.Fatalf("result.Token = %q, want %q", result.Token, "recaptcha-token-2c")
	}
}

func TestTwoCaptchaSolverTurnstile(t *testing.T) {
	var mu sync.Mutex
	captchaID := "2captcha-ts-111"

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		defer mu.Unlock()

		switch r.URL.Path {
		case "/in.php":
			r.ParseForm()
			if r.Form.Get("method") != "turnstile" {
				t.Fatalf("method = %q", r.Form.Get("method"))
			}
			json.NewEncoder(w).Encode(map[string]any{
				"status":  1,
				"request": captchaID,
			})

		case "/res.php":
			if r.URL.Query().Get("id") == captchaID {
				json.NewEncoder(w).Encode(map[string]any{
					"status":  1,
					"request": "turnstile-token-2c",
				})
			}

		default:
			http.NotFound(w, r)
		}
	}))
	defer server.Close()

	solver := NewTwoCaptchaSolver("test-key-2c", 30*time.Second)
	solver.baseURL = server.URL

	result, err := solver.Solve(context.Background(), &SolveRequest{
		Type:    CaptchaTurnstile,
		SiteKey: "0x4AAAAAAA-test",
		PageURL: "https://example.com",
	})
	if err != nil {
		t.Fatalf("2Captcha Solve error: %v", err)
	}
	if result.Token != "turnstile-token-2c" {
		t.Fatalf("result.Token = %q, want %q", result.Token, "turnstile-token-2c")
	}
}

func TestTwoCaptchaSolverUnsupportedType(t *testing.T) {
	solver := NewTwoCaptchaSolver("test-key-2c", 30*time.Second)
	_, err := solver.Solve(context.Background(), &SolveRequest{
		Type: CaptchaFun,
	})
	if err == nil {
		t.Fatal("expected error for unsupported type, got nil")
	}
}

func TestTwoCaptchaSolverImageNeedsData(t *testing.T) {
	solver := NewTwoCaptchaSolver("test-key-2c", 30*time.Second)
	_, err := solver.Solve(context.Background(), &SolveRequest{
		Type: CaptchaImage,
	})
	if err == nil {
		t.Fatal("expected error for missing image data, got nil")
	}
}

func TestTwoCaptchaSolverGetBalance(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Query().Get("action") == "getbalance" {
			json.NewEncoder(w).Encode(map[string]any{
				"status":  1,
				"request": "99.99",
			})
		}
	}))
	defer server.Close()

	solver := NewTwoCaptchaSolver("test-key-2c", 30*time.Second)
	solver.baseURL = server.URL

	bal, err := solver.GetBalance(context.Background())
	if err != nil {
		t.Fatalf("GetBalance error: %v", err)
	}
	if bal != 99.99 {
		t.Fatalf("balance = %f, want 99.99", bal)
	}
}

func TestTwoCaptchaSolverSubmitError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{
			"status":  0,
			"request": "ERROR_INVALID_KEY",
		})
	}))
	defer server.Close()

	solver := NewTwoCaptchaSolver("invalid-key", 5*time.Second)
	solver.baseURL = server.URL

	_, err := solver.Solve(context.Background(), &SolveRequest{
		Type:      CaptchaImage,
		ImageData: []byte{1, 2, 3},
	})
	if err == nil {
		t.Fatal("expected submit error, got nil")
	}
}

func TestTwoCaptchaSolverTimeout(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/in.php":
			json.NewEncoder(w).Encode(map[string]any{
				"status":  1,
				"request": "timeout-id",
			})
		case "/res.php":
			json.NewEncoder(w).Encode(map[string]any{
				"status":  0,
				"request": "CAPCHA_NOT_READY",
			})
		}
	}))
	defer server.Close()

	solver := NewTwoCaptchaSolver("test-key", 5*time.Second)
	solver.baseURL = server.URL

	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	_, err := solver.Solve(ctx, &SolveRequest{
		Type:      CaptchaImage,
		ImageData: []byte{1, 2, 3},
	})
	if err == nil {
		t.Fatal("expected timeout error")
	}
}

func TestTwoCaptchaSolverSetCost(t *testing.T) {
	solver := NewTwoCaptchaSolver("test-key", 30*time.Second)
	solver.SetCost(CaptchaImage, 0.005)
	if c := solver.getCostForType(CaptchaImage); c != 0.005 {
		t.Fatalf("cost = %f, want 0.005", c)
	}
}

func TestManagerConcurrency(t *testing.T) {
	cfg := &Config{CacheTTL: 60 * time.Second}
	m := NewManager(cfg)

	m.AddSolver(&mockSolver{
		name: "mock",
		solveFn: func(ctx context.Context, req *SolveRequest) (*SolveResult, error) {
			time.Sleep(10 * time.Millisecond)
			return &SolveResult{Text: "OK", Provider: "mock", Cost: 0.001}, nil
		},
	})

	var wg sync.WaitGroup
	for i := 0; i < 20; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, err := m.Solve(context.Background(), &SolveRequest{
				Type:    CaptchaImage,
				SiteKey: "concurrent-test",
				PageURL: "https://example.com",
			})
			if err != nil {
				t.Errorf("concurrent Solve error: %v", err)
			}
		}()
	}
	wg.Wait()

	metrics := m.GetMetrics()
	if metrics.TotalSolved == 0 {
		t.Fatalf("TotalSolved = 0, expected at least 1")
	}
	if metrics.TotalSolved > 20 {
		t.Fatalf("TotalSolved = %d, should not exceed 20", metrics.TotalSolved)
	}
}

func TestNewManagerWithZeroCacheTTL(t *testing.T) {
	cfg := &Config{CacheTTL: 0}
	m := NewManager(cfg)
	if m.cache.ttl != 60*time.Second {
		t.Fatalf("cache TTL = %v, want 60s default", m.cache.ttl)
	}
}

func TestCapsolverDefaultTimeout(t *testing.T) {
	solver := NewCapsolverSolver("test-key", 0)
	if solver.client.Timeout != 120*time.Second {
		t.Fatalf("default timeout = %v, want 120s", solver.client.Timeout)
	}
}

func TestTwoCaptchaDefaultTimeout(t *testing.T) {
	solver := NewTwoCaptchaSolver("test-key", 0)
	if solver.client.Timeout != 120*time.Second {
		t.Fatalf("default timeout = %v, want 120s", solver.client.Timeout)
	}
}

func TestCapsolverSolverGeeTestMissingParams(t *testing.T) {
	solver := NewCapsolverSolver("test-key", 30*time.Second)
	_, err := solver.Solve(context.Background(), &SolveRequest{
		Type:    CaptchaGeeTest,
		SiteKey: "gt-key",
		PageURL: "https://example.com",
	})
	if err == nil {
		t.Fatal("expected error for missing gt/challenge, got nil")
	}
}
