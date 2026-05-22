package sms

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"
)

// ─── 5sim mock server ────────────────────────────────────────────────────────────

type fiveSimHandler struct {
	mu          sync.Mutex
	balance     float64
	buyResult   map[string]any
	checkResult map[string]any
	actionOK    bool
}

func newFiveSimMock() *fiveSimHandler {
	return &fiveSimHandler{
		balance: 50.0,
		buyResult: map[string]any{
			"id":      12345,
			"phone":   15627231715,
			"price":   400,
			"status":  "PENDING",
			"expires": time.Now().Add(10 * time.Minute).Format(time.RFC3339),
			"product": "google",
			"country": "usa",
		},
		checkResult: map[string]any{
			"status": "PENDING",
			"sms":    []any{},
		},
		actionOK: true,
	}
}

func fiveSimAuthOK(r *http.Request) bool {
	auth := r.Header.Get("Authorization")
	return strings.HasPrefix(auth, "Bearer ") && len(auth) > 7
}

func (h *fiveSimHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	h.mu.Lock()
	defer h.mu.Unlock()

	// Guest endpoint — no auth required
	if containsPath(r.URL.Path, "/v1/user/guest/prices/") {
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]map[string]map[string]any{
			"virtual8": {
				"google": {"cost": 0.01, "count": 100},
			},
		})
		return
	}

	if !fiveSimAuthOK(r) {
		http.Error(w, `{"error":"unauthorized"}`, http.StatusUnauthorized)
		return
	}

	switch {
	case r.URL.Path == "/v1/user/profile":
		json.NewEncoder(w).Encode(map[string]float64{"balance": h.balance})

	case containsPath(r.URL.Path, "/v1/user/buy/activation/"):
		if h.buyResult == nil {
			http.Error(w, `{"error":"no stock"}`, http.StatusNotFound)
			return
		}
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(h.buyResult)

	case containsPath(r.URL.Path, "/v1/user/check/"):
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(h.checkResult)

	case containsPath(r.URL.Path, "/v1/user/cancel/"):
		if h.actionOK {
			w.WriteHeader(http.StatusOK)
			json.NewEncoder(w).Encode(map[string]string{"status": "canceled"})
		} else {
			http.Error(w, `{"error":"already finished"}`, http.StatusBadGateway)
		}

	case containsPath(r.URL.Path, "/v1/user/finish/"):
		if h.actionOK {
			w.WriteHeader(http.StatusOK)
			json.NewEncoder(w).Encode(map[string]string{"status": "finished"})
		} else {
			http.Error(w, `{"error":"already finished"}`, http.StatusBadGateway)
		}

	default:
		http.Error(w, "not found", http.StatusNotFound)
	}
}

// ─── SMSPool mock server ─────────────────────────────────────────────────────────

type smspoolHandler struct {
	mu          sync.Mutex
	balance     float64
	buyResult   map[string]any
	checkResult map[string]any
	cancelOK    bool
}

func newSMSPoolMock() *smspoolHandler {
	return &smspoolHandler{
		balance: 25.0,
		buyResult: map[string]any{
			"order_id": 98765,
			"number":   "+14155551234",
			"cost":     0.05,
			"service":  "google",
			"country":  "us",
			"status":   "pending",
			"success":  true,
		},
		checkResult: map[string]any{
			"success": true,
			"status":  1,
			"sms":     "",
			"sender":  "",
		},
		cancelOK: true,
	}
}

func (h *smspoolHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	h.mu.Lock()
	defer h.mu.Unlock()

	if r.Header.Get("Authorization") == "" {
		http.Error(w, `{"success":false,"message":"unauthorized"}`, http.StatusUnauthorized)
		return
	}

	switch {
	case r.URL.Path == "/balance":
		json.NewEncoder(w).Encode(map[string]float64{"balance": h.balance})

	case containsPath(r.URL.Path, "/sms/order/"):
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(h.buyResult)

	case containsPath(r.URL.Path, "/sms/check/"):
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(h.checkResult)

	case containsPath(r.URL.Path, "/sms/cancel/"):
		if h.cancelOK {
			w.WriteHeader(http.StatusOK)
			json.NewEncoder(w).Encode(map[string]any{"success": true})
		} else {
			http.Error(w, `{"success":false}`, http.StatusBadRequest)
		}

	default:
		http.Error(w, "not found", http.StatusNotFound)
	}
}

// ─── helpers ─────────────────────────────────────────────────────────────────────

func containsPath(path, prefix string) bool {
	n := len(prefix)
	if len(path) < n {
		return false
	}
	return path[:n] == prefix
}

// Temporarily replaces a provider's baseURL for testing.
func withFiveSimProvider(apiKey, baseURL string) *FiveSimProvider {
	p := NewFiveSimProvider(apiKey)
	p.baseURL = baseURL
	return p
}

func withSMSPoolProvider(apiKey, baseURL string) *SMSPoolProvider {
	p := NewSMSPoolProvider(apiKey)
	p.baseURL = baseURL
	return p
}

// ─── Tests: 5sim provider ────────────────────────────────────────────────

func TestFiveSimProvider_BuyNumber(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withFiveSimProvider("test-key", srv.URL)
	n, err := p.BuyNumber(context.Background(), &BuyRequest{
		Country: "usa", Service: "google", Operator: "virtual8",
	})
	if err != nil {
		t.Fatalf("BuyNumber failed: %v", err)
	}
	if n.ID != "12345" {
		t.Errorf("expected ID 12345, got %s", n.ID)
	}
	if n.Phone != "+15627231715" {
		t.Errorf("expected +15627231715, got %s", n.Phone)
	}
	if n.Price != 4.0 {
		t.Errorf("expected price 4.0, got %f", n.Price)
	}
	if n.Status != StatusPending {
		t.Errorf("expected PENDING, got %s", n.Status)
	}
}

func TestFiveSimProvider_CheckSMS_Pending(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withFiveSimProvider("test-key", srv.URL)
	result, err := p.CheckSMS(context.Background(), "12345")
	if err != nil {
		t.Fatalf("CheckSMS failed: %v", err)
	}
	if result.Status != StatusPending {
		t.Errorf("expected PENDING, got %s", result.Status)
	}
	if result.SMS != nil {
		t.Errorf("expected nil SMS, got %+v", result.SMS)
	}
}

func TestFiveSimProvider_CheckSMS_Received(t *testing.T) {
	mock := newFiveSimMock()
	mock.mu.Lock()
	mock.checkResult = map[string]any{
		"status": "RECEIVED",
		"sms": []any{
			map[string]any{
				"code":   "644794",
				"text":   "Your Google verification code is 644794",
				"sender": "Google",
				"date":   time.Now().Format(time.RFC3339),
			},
		},
	}
	mock.mu.Unlock()

	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withFiveSimProvider("test-key", srv.URL)
	result, err := p.CheckSMS(context.Background(), "12345")
	if err != nil {
		t.Fatalf("CheckSMS failed: %v", err)
	}
	if result.Status != StatusReceived {
		t.Errorf("expected RECEIVED, got %s", result.Status)
	}
	if result.SMS == nil {
		t.Fatal("expected SMS data, got nil")
	}
	if result.SMS.Code != "644794" {
		t.Errorf("expected code 644794, got %s", result.SMS.Code)
	}
	if result.SMS.Text != "Your Google verification code is 644794" {
		t.Errorf("unexpected text: %s", result.SMS.Text)
	}
	if result.SMS.Sender != "Google" {
		t.Errorf("expected sender Google, got %s", result.SMS.Sender)
	}
}

func TestFiveSimProvider_Cancel(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withFiveSimProvider("test-key", srv.URL)
	err := p.Cancel(context.Background(), "12345")
	if err != nil {
		t.Fatalf("Cancel failed: %v", err)
	}
}

func TestFiveSimProvider_Finish(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withFiveSimProvider("test-key", srv.URL)
	err := p.Finish(context.Background(), "12345")
	if err != nil {
		t.Fatalf("Finish failed: %v", err)
	}
}

func TestFiveSimProvider_GetBalance(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withFiveSimProvider("test-key", srv.URL)
	bal, err := p.GetBalance(context.Background())
	if err != nil {
		t.Fatalf("GetBalance failed: %v", err)
	}
	if bal != 50.0 {
		t.Errorf("expected balance 50, got %f", bal)
	}
}

func TestFiveSimProvider_GetPrices(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withFiveSimProvider("test-key", srv.URL)
	price, err := p.GetPrices(context.Background(), "usa", "google")
	if err != nil {
		t.Fatalf("GetPrices failed: %v", err)
	}
	if price != 0.01 {
		t.Errorf("expected price 0.01, got %f", price)
	}
}

func TestFiveSimProvider_BuyNumber_Unauthorized(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withFiveSimProvider("", srv.URL)
	_, err := p.BuyNumber(context.Background(), &BuyRequest{
		Country: "usa", Service: "google", Operator: "any",
	})
	if err == nil {
		t.Fatal("expected error for missing auth, got nil")
	}
}

// ─── Tests: SMSPool provider ─────────────────────────────────────────────

func TestSMSPoolProvider_BuyNumber(t *testing.T) {
	mock := newSMSPoolMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withSMSPoolProvider("test-key", srv.URL)
	n, err := p.BuyNumber(context.Background(), &BuyRequest{
		Country: "us", Service: "google",
	})
	if err != nil {
		t.Fatalf("BuyNumber failed: %v", err)
	}
	if n.ID != "98765" {
		t.Errorf("expected ID 98765, got %s", n.ID)
	}
	if n.Phone != "+14155551234" {
		t.Errorf("expected +14155551234, got %s", n.Phone)
	}
	if n.Price != 0.05 {
		t.Errorf("expected price 0.05, got %f", n.Price)
	}
	if n.Status != StatusPending {
		t.Errorf("expected PENDING, got %s", n.Status)
	}
}

func TestSMSPoolProvider_CheckSMS_Pending(t *testing.T) {
	mock := newSMSPoolMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withSMSPoolProvider("test-key", srv.URL)
	result, err := p.CheckSMS(context.Background(), "98765")
	if err != nil {
		t.Fatalf("CheckSMS failed: %v", err)
	}
	if result.Status != StatusPending {
		t.Errorf("expected PENDING, got %s", result.Status)
	}
}

func TestSMSPoolProvider_CheckSMS_Received(t *testing.T) {
	mock := newSMSPoolMock()
	mock.mu.Lock()
	mock.checkResult = map[string]any{
		"success":     true,
		"status":      2,
		"sms":         "Your Google verification code: 123456",
		"sender":      "Google",
		"insert_date": time.Now().Format("2006-01-02 15:04:05"),
	}
	mock.mu.Unlock()

	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withSMSPoolProvider("test-key", srv.URL)
	result, err := p.CheckSMS(context.Background(), "98765")
	if err != nil {
		t.Fatalf("CheckSMS failed: %v", err)
	}
	if result.Status != StatusReceived {
		t.Errorf("expected RECEIVED, got %s", result.Status)
	}
	if result.SMS == nil {
		t.Fatal("expected SMS data, got nil")
	}
	if result.SMS.Text != "Your Google verification code: 123456" {
		t.Errorf("unexpected text: %s", result.SMS.Text)
	}
	if result.SMS.Sender != "Google" {
		t.Errorf("expected sender Google, got %s", result.SMS.Sender)
	}
}

func TestSMSPoolProvider_Cancel(t *testing.T) {
	mock := newSMSPoolMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withSMSPoolProvider("test-key", srv.URL)
	err := p.Cancel(context.Background(), "98765")
	if err != nil {
		t.Fatalf("Cancel failed: %v", err)
	}
}

func TestSMSPoolProvider_Finish(t *testing.T) {
	mock := newSMSPoolMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withSMSPoolProvider("test-key", srv.URL)
	err := p.Finish(context.Background(), "98765")
	if err != nil {
		t.Fatalf("Finish should be a no-op, got error: %v", err)
	}
}

func TestSMSPoolProvider_GetBalance(t *testing.T) {
	mock := newSMSPoolMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withSMSPoolProvider("test-key", srv.URL)
	bal, err := p.GetBalance(context.Background())
	if err != nil {
		t.Fatalf("GetBalance failed: %v", err)
	}
	if bal != 25.0 {
		t.Errorf("expected balance 25, got %f", bal)
	}
}

func TestSMSPoolProvider_BuyNumber_Failure(t *testing.T) {
	mock := newSMSPoolMock()
	mock.mu.Lock()
	mock.buyResult = map[string]any{
		"success": false,
		"message": "insufficient balance",
	}
	mock.mu.Unlock()

	srv := httptest.NewServer(mock)
	defer srv.Close()

	p := withSMSPoolProvider("test-key", srv.URL)
	_, err := p.BuyNumber(context.Background(), &BuyRequest{
		Country: "us", Service: "google",
	})
	if err == nil {
		t.Fatal("expected error for insufficient balance, got nil")
	}
}

// ─── Tests: SMS Manager ─────────────────────────────────────────────────

func TestManager_AcquireNumber_PrefersPool(t *testing.T) {
	mock5sim := newFiveSimMock()
	mockSmspool := newSMSPoolMock()
	srv5 := httptest.NewServer(mock5sim)
	srvSp := httptest.NewServer(mockSmspool)
	defer srv5.Close()
	defer srvSp.Close()

	mgr := NewManager(&Config{PrefetchCount: 5})
	mgr.AddProvider(withFiveSimProvider("key5", srv5.URL))
	mgr.AddProvider(withSMSPoolProvider("keySp", srvSp.URL))

	prefetched := &Number{
		ID: "pool-1", Phone: "+19999999999",
		Service: "google", Status: StatusPending,
	}
	mgr.pool.Push("google", prefetched)

	n, err := mgr.AcquireNumber(context.Background(), &BuyRequest{
		Country: "usa", Service: "google",
	})
	if err != nil {
		t.Fatalf("AcquireNumber failed: %v", err)
	}
	if n.ID != "pool-1" {
		t.Errorf("expected pool number, got %s", n.ID)
	}
}

func TestManager_AcquireNumber_FallsBack(t *testing.T) {
	mock5sim := newFiveSimMock()
	srv5 := httptest.NewServer(mock5sim)
	defer srv5.Close()

	mgr := NewManager(&Config{PrefetchCount: 0})
	mgr.AddProvider(withFiveSimProvider("key5", srv5.URL))

	n, err := mgr.AcquireNumber(context.Background(), &BuyRequest{
		Country: "usa", Service: "google", Operator: "virtual8",
	})
	if err != nil {
		t.Fatalf("AcquireNumber failed: %v", err)
	}
	if n.ID != "12345" {
		t.Errorf("expected ID 12345, got %s", n.ID)
	}
}

func TestManager_AcquireNumber_BlacklistSkips(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	mgr := NewManager(&Config{PrefetchCount: 0})
	mgr.AddProvider(withFiveSimProvider("key5", srv.URL))
	mgr.blacklist.Add("+15627231715")

	_, err := mgr.AcquireNumber(context.Background(), &BuyRequest{
		Country: "usa", Service: "google", Operator: "virtual8",
	})
	if err == nil {
		t.Fatal("expected error when blacklisted number is the only option")
	}
}

func TestManager_ReleaseNumber(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	mgr := NewManager(&Config{})
	mgr.AddProvider(withFiveSimProvider("key5", srv.URL))

	err := mgr.ReleaseNumber(context.Background(), &Number{ID: "12345"})
	if err != nil {
		t.Fatalf("ReleaseNumber failed: %v", err)
	}
}

func TestManager_FinishNumber(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	mgr := NewManager(&Config{})
	mgr.AddProvider(withFiveSimProvider("key5", srv.URL))

	err := mgr.FinishNumber(context.Background(), &Number{ID: "12345"})
	if err != nil {
		t.Fatalf("FinishNumber failed: %v", err)
	}
}

func TestManager_WaitForCode_Timeout(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	mgr := NewManager(&Config{PollInterval: 50 * time.Millisecond})
	mgr.AddProvider(withFiveSimProvider("key5", srv.URL))

	_, err := mgr.WaitForCode(context.Background(), &Number{
		ID: "12345", Status: StatusPending,
	}, 100*time.Millisecond)

	if err == nil {
		t.Fatal("expected timeout error, got nil")
	}
}

func TestManager_WaitForCode_Success(t *testing.T) {
	mock := newFiveSimMock()
	mock.mu.Lock()
	mock.checkResult = map[string]any{
		"status": "RECEIVED",
		"sms": []any{
			map[string]any{
				"code": "888888",
				"text": "Your code is 888888",
			},
		},
	}
	mock.mu.Unlock()

	srv := httptest.NewServer(mock)
	defer srv.Close()

	mgr := NewManager(&Config{PollInterval: 50 * time.Millisecond})
	mgr.AddProvider(withFiveSimProvider("key5", srv.URL))

	result, err := mgr.WaitForCode(context.Background(), &Number{
		ID: "12345", Status: StatusPending,
	}, 5*time.Second)

	if err != nil {
		t.Fatalf("WaitForCode failed: %v", err)
	}
	if result.Status != StatusReceived {
		t.Errorf("expected RECEIVED, got %s", result.Status)
	}
	if result.SMS == nil || result.SMS.Code != "888888" {
		t.Errorf("expected code 888888, got %+v", result.SMS)
	}
}

func TestManager_Metrics(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	mgr := NewManager(&Config{})
	mgr.AddProvider(withFiveSimProvider("key5", srv.URL))

	_, err := mgr.AcquireNumber(context.Background(), &BuyRequest{
		Country: "usa", Service: "google", Operator: "virtual8",
	})
	if err != nil {
		t.Fatalf("AcquireNumber failed: %v", err)
	}

	total, success, fail, cost := mgr.Metrics().Snapshot()
	if total != 1 {
		t.Errorf("expected total 1, got %d", total)
	}
	if cost <= 0 {
		t.Errorf("expected positive cost, got %f", cost)
	}
	if success != 0 {
		t.Errorf("expected 0 success, got %d", success)
	}
	if fail != 0 {
		t.Errorf("expected 0 fail, got %d", fail)
	}
}

func TestManager_GetBalance(t *testing.T) {
	mock := newFiveSimMock()
	srv := httptest.NewServer(mock)
	defer srv.Close()

	mgr := NewManager(&Config{})
	mgr.AddProvider(withFiveSimProvider("key5", srv.URL))

	bal, err := mgr.GetBalance(context.Background())
	if err != nil {
		t.Fatalf("GetBalance failed: %v", err)
	}
	if bal != 50.0 {
		t.Errorf("expected balance 50, got %f", bal)
	}
}

// ─── Tests: NumberPool ───────────────────────────────────────────────────

func TestNumberPool_PushPop(t *testing.T) {
	pool := NewNumberPool(5)

	n := &Number{ID: "n1", Service: "google"}
	pool.Push("google", n)

	got := pool.Pop("google")
	if got == nil {
		t.Fatal("Pop returned nil")
	}
	if got.ID != "n1" {
		t.Errorf("expected n1, got %s", got.ID)
	}

	got2 := pool.Pop("google")
	if got2 != nil {
		t.Errorf("expected nil from empty pool, got %+v", got2)
	}
}

func TestNumberPool_Size(t *testing.T) {
	pool := NewNumberPool(3)
	if s := pool.Size("google"); s != 0 {
		t.Errorf("expected 0, got %d", s)
	}
	pool.Push("google", &Number{ID: "1"})
	pool.Push("google", &Number{ID: "2"})
	if s := pool.Size("google"); s != 2 {
		t.Errorf("expected 2, got %d", s)
	}
}

func TestNumberPool_AllSizes(t *testing.T) {
	pool := NewNumberPool(3)
	pool.Push("google", &Number{ID: "1"})
	pool.Push("telegram", &Number{ID: "2"})
	sizes := pool.AllSizes()
	if sizes["google"] != 1 {
		t.Errorf("expected google:1, got %d", sizes["google"])
	}
	if sizes["telegram"] != 1 {
		t.Errorf("expected telegram:1, got %d", sizes["telegram"])
	}
}

// ─── Tests: Blacklist ────────────────────────────────────────────────────

func TestBlacklist(t *testing.T) {
	b := NewBlacklist()
	if b.Contains("+123") {
		t.Error("expected false for empty blacklist")
	}
	b.Add("+123")
	if !b.Contains("+123") {
		t.Error("expected true after add")
	}
	b.Remove("+123")
	if b.Contains("+123") {
		t.Error("expected false after remove")
	}
}

func TestBlacklist_List(t *testing.T) {
	b := NewBlacklist()
	b.Add("+111")
	b.Add("+222")
	list := b.List()
	if len(list) != 2 {
		t.Errorf("expected 2 items, got %d", len(list))
	}
}

// ─── Tests: OTP Extractor ───────────────────────────────────────────────

func TestOTPExtractor_Extract(t *testing.T) {
	e := NewOTPExtractor()

	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"label+6digit", "Your verification code is 123456", "123456"},
		{"label+4digit", "Your PIN: 7890", "7890"},
		{"chinese", "您的验证码是 654321", "654321"},
		{"japanese", "認証コード　　987654", "987654"},
		{"russian", "Ваш код: 123456", "123456"},
		{"bare 6 digits", "Some text 445566 end", "445566"},
		{"bare 4 digits", "Code 1122", "1122"},
		{"alphanumeric", "Token ABC123", "ABC123"},
		{"no code", "Hello world", ""},
		{"empty input", "", ""},
		{"code after colon", "Your OTP: 332211", "332211"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := e.Extract(tt.input)
			if got != tt.expected {
				t.Errorf("Extract(%q) = %q, want %q", tt.input, got, tt.expected)
			}
		})
	}
}

func TestOTPExtractor_Priority(t *testing.T) {
	e := NewOTPExtractor()

	result := e.Extract("Your verification code: 1234 and some random 999999")
	if result != "1234" {
		t.Errorf("expected labeled code 1234 to take priority over 999999, got %s", result)
	}
}

// ─── Tests: Multi-provider Manager ───────────────────────────────────────

func TestManager_MultiProvider_Fallback(t *testing.T) {
	fail5sim := newFiveSimMock()
	fail5sim.mu.Lock()
	fail5sim.buyResult = nil
	fail5sim.mu.Unlock()

	mockSp := newSMSPoolMock()
	srvFail := httptest.NewServer(fail5sim)
	srvSp := httptest.NewServer(mockSp)
	defer srvFail.Close()
	defer srvSp.Close()

	mgr := NewManager(&Config{})
	mgr.AddProvider(withFiveSimProvider("key5", srvFail.URL))
	mgr.AddProvider(withSMSPoolProvider("keySp", srvSp.URL))

	n, err := mgr.AcquireNumber(context.Background(), &BuyRequest{
		Country: "us", Service: "google",
	})
	if err != nil {
		t.Fatalf("AcquireNumber with fallback failed: %v", err)
	}
	if n.ID != "98765" {
		t.Errorf("expected fallback to smspool (ID 98765), got %s", n.ID)
	}
}

// ─── Tests: Manager edge cases ───────────────────────────────────────────

func TestManager_EmptyProviders(t *testing.T) {
	mgr := NewManager(&Config{PrefetchCount: 0})
	_, err := mgr.AcquireNumber(context.Background(), &BuyRequest{
		Country: "usa", Service: "google",
	})
	if err == nil {
		t.Fatal("expected error with no providers")
	}
}

func TestManager_ReleaseNumber_NoProviders(t *testing.T) {
	mgr := NewManager(&Config{})
	err := mgr.ReleaseNumber(context.Background(), &Number{ID: "123"})
	if err != nil {
		t.Errorf("expected nil error with no providers, got %v", err)
	}
}

func TestManager_FinishNumber_NoProviders(t *testing.T) {
	mgr := NewManager(&Config{})
	err := mgr.FinishNumber(context.Background(), &Number{ID: "123"})
	if err != nil {
		t.Errorf("expected nil error with no providers, got %v", err)
	}
}

func TestManager_GetBalance_NoProviders(t *testing.T) {
	mgr := NewManager(&Config{})
	_, err := mgr.GetBalance(context.Background())
	if err == nil {
		t.Fatal("expected error with no providers")
	}
}

// ─── Tests: Rate limiting / edge cases ───────────────────────────────────

func TestFiveSimProvider_RateLimited(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Retry-After", "30")
		w.WriteHeader(http.StatusTooManyRequests)
		fmt.Fprintln(w, `{"error":"rate limit"}`)
	})
	srv := httptest.NewServer(handler)
	defer srv.Close()

	p := withFiveSimProvider("key", srv.URL)
	_, err := p.BuyNumber(context.Background(), &BuyRequest{
		Country: "usa", Service: "google",
	})
	if err == nil {
		t.Fatal("expected rate limit error")
	}
	if !strings.Contains(err.Error(), "rate limited") {
		t.Errorf("expected 'rate limited' in error, got: %v", err)
	}
}

func TestSMSPoolProvider_CheckSMS_StatusCodes(t *testing.T) {
	tests := []struct {
		code     int
		expected NumberStatus
	}{
		{1, StatusPending},
		{2, StatusReceived},
		{3, StatusReceived},
		{4, StatusCanceled},
		{5, StatusTimeout},
		{99, StatusPending},
	}

	for _, tt := range tests {
		t.Run(fmt.Sprintf("status_%d", tt.code), func(t *testing.T) {
			got := parseSMSPoolCheckStatus(tt.code)
			if got != tt.expected {
				t.Errorf("parseSMSPoolCheckStatus(%d) = %s, want %s", tt.code, got, tt.expected)
			}
		})
	}
}
