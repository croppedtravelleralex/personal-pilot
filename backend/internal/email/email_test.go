package email

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"regexp"
	"strings"
	"sync"
	"testing"
	"time"
)

// ─── helpers ──────────────────────────────────────────────────────────────────

type testTransport struct {
	base     http.RoundTripper
	rewrites map[string]string
}

func (t *testTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	if replacement, ok := t.rewrites[req.URL.Host]; ok {
		u, _ := url.Parse(replacement)
		req.URL.Host = u.Host
		req.URL.Scheme = u.Scheme
	}
	return t.base.RoundTrip(req)
}

func withHostRewrite(fromHost, toURL string) func() {
	orig := http.DefaultTransport
	http.DefaultTransport = &testTransport{
		base: orig,
		rewrites: map[string]string{
			fromHost: toURL,
		},
	}
	return func() { http.DefaultTransport = orig }
}

func newCloudflareClient(serverURL string) *CloudflareTempClient {
	return &CloudflareTempClient{
		hc:        &http.Client{},
		apiBase:   serverURL,
		address:   "test@test.com",
		jwt:       "test-jwt",
		cachedIDs: make(map[string]struct{}),
	}
}

func newMailTMMockServer(t *testing.T, msgHandler func(http.ResponseWriter, *http.Request)) *httptest.Server {
	t.Helper()
	return httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.Path == "/domains":
			json.NewEncoder(w).Encode(map[string]any{
				"hydra:member": []map[string]string{{"domain": "test.com"}},
			})
		case r.URL.Path == "/accounts":
			w.WriteHeader(http.StatusCreated)
			json.NewEncoder(w).Encode(map[string]string{
				"address": "test@test.com",
				"id":      "acct-1",
			})
		case r.URL.Path == "/token":
			json.NewEncoder(w).Encode(map[string]string{
				"token": "mock-token",
			})
		default:
			if msgHandler != nil {
				msgHandler(w, r)
			} else {
				w.WriteHeader(http.StatusNotFound)
			}
		}
	}))
}

func withFastPoll(t *testing.T) func() {
	origInterval := TempEmailPollInterval
	TempEmailPollInterval = 1 * time.Millisecond
	return func() { TempEmailPollInterval = origInterval }
}

// ─── TestNewCloudflareTempClient ──────────────────────────────────────────────

func TestNewCloudflareTempClient_Valid(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/new_address" && r.Method == http.MethodPost {
			json.NewEncoder(w).Encode(map[string]string{
				"jwt":     "test-jwt",
				"address": "test@example.com",
			})
			return
		}
		w.WriteHeader(http.StatusNotFound)
	}))
	defer server.Close()

	client, err := NewCloudflareTempClient(server.URL)
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if client.Address() != "test@example.com" {
		t.Errorf("expected address test@example.com, got %s", client.Address())
	}
}

func TestNewCloudflareTempClient_HTTPErrors(t *testing.T) {
	attempts := 0
	primaryServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		attempts++
		w.WriteHeader(http.StatusInternalServerError)
		w.Write([]byte(`{"error":"server error"}`))
	}))
	defer primaryServer.Close()

	mailtmServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusServiceUnavailable)
	}))
	defer mailtmServer.Close()

	restore := withHostRewrite("api.mail.tm", mailtmServer.URL)
	defer restore()

	_, err := NewCloudflareTempClient(primaryServer.URL)
	if err == nil {
		t.Fatal("expected error from HTTP failures, got nil")
	}
	if attempts > 3 {
		t.Errorf("expected at most 3 retry attempts, got %d", attempts)
	}
	if !strings.Contains(err.Error(), "HTTP 500") {
		t.Errorf("expected HTTP 500 in error, got: %v", err)
	}
}

func TestNewCloudflareTempClient_FallbackSuccess(t *testing.T) {
	primaryServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusServiceUnavailable)
	}))
	defer primaryServer.Close()

	mailtmServer := newMailTMMockServer(t, nil)
	defer mailtmServer.Close()

	restore := withHostRewrite("api.mail.tm", mailtmServer.URL)
	defer restore()

	client, err := NewCloudflareTempClient(primaryServer.URL)
	if err != nil {
		t.Fatalf("expected fallback success, got: %v", err)
	}
	if client.Address() != "test@test.com" {
		t.Errorf("expected address test@test.com from fallback, got %s", client.Address())
	}
}

func TestNewCloudflareTempClient_EmptyAPIBase(t *testing.T) {
	mockServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/new_address" && r.Method == http.MethodPost {
			json.NewEncoder(w).Encode(map[string]string{
				"jwt":     "default-jwt",
				"address": "default@example.com",
			})
			return
		}
		w.WriteHeader(http.StatusNotFound)
	}))
	defer mockServer.Close()

	defaultURL, _ := url.Parse(DefaultTempEmailAPIBase)
	restore1 := withHostRewrite(defaultURL.Host, mockServer.URL)
	defer restore1()

	// Block mail.tm fallback so it doesn't succeed instead
	mailtmServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusServiceUnavailable)
	}))
	defer mailtmServer.Close()

	restore2 := withHostRewrite("api.mail.tm", mailtmServer.URL)
	defer restore2()

	client, err := NewCloudflareTempClient("")
	if err != nil {
		t.Fatalf("expected no error with mocked default URL, got: %v", err)
	}
	if client.Address() != "default@example.com" {
		t.Errorf("expected address default@example.com, got %s", client.Address())
	}
}

// ─── TestWaitForCode ──────────────────────────────────────────────────────────

func TestWaitForCode_Immediate(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{
				{
					"id":      1,
					"subject": "Your verification code is 123456",
					"text":    "",
					"html":    "",
					"from":    "noreply@test.com",
				},
			},
		})
	}))
	defer server.Close()

	client := newCloudflareClient(server.URL)
	ctx := context.Background()
	code, err := client.WaitForCode(ctx, 5*time.Second)
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if code != "123456" {
		t.Errorf("expected code 123456, got %s", code)
	}
}

func TestWaitForCode_AfterNPolls(t *testing.T) {
	var mu sync.Mutex
	pollCount := 0

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		count := pollCount
		pollCount++
		mu.Unlock()

		if count < 2 {
			json.NewEncoder(w).Encode(map[string]any{"results": []any{}})
			return
		}
		json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{
				{
					"id":      999,
					"subject": "Your OTP is 654321",
					"text":    "",
					"html":    "",
					"from":    "noreply@test.com",
				},
			},
		})
	}))
	defer server.Close()

	client := newCloudflareClient(server.URL)
	restorePoll := withFastPoll(t)
	defer restorePoll()

	ctx := context.Background()
	code, err := client.WaitForCode(ctx, 30*time.Second)
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if code != "654321" {
		t.Errorf("expected code 654321, got %s", code)
	}

	mu.Lock()
	if pollCount < 3 {
		t.Errorf("expected at least 3 polls (2 empty + 1 with code), got %d", pollCount)
	}
	mu.Unlock()
}

func TestWaitForCode_Timeout(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{"results": []any{}})
	}))
	defer server.Close()

	client := newCloudflareClient(server.URL)
	ctx := context.Background()
	_, err := client.WaitForCode(ctx, 100*time.Millisecond)
	if err == nil {
		t.Fatal("expected timeout error, got nil")
	}
	if !strings.Contains(err.Error(), "verification code not received") {
		t.Errorf("unexpected error message: %v", err)
	}
}

func TestWaitForCode_ContextCancelled(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{"results": []any{}})
	}))
	defer server.Close()

	client := newCloudflareClient(server.URL)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := client.WaitForCode(ctx, 10*time.Second)
	if err == nil {
		t.Fatal("expected error from cancelled context, got nil")
	}
}

// ─── TestWaitForMail ──────────────────────────────────────────────────────────

func TestWaitForMail_FilterMatches(t *testing.T) {
	restore := withFastPoll(t)
	defer restore()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{
				{
					"id":      1,
					"subject": "Welcome to our service",
					"text":    "Thank you",
					"html":    "<p>Thank you</p>",
					"from":    "support@example.com",
				},
			},
		})
	}))
	defer server.Close()

	client := newCloudflareClient(server.URL)
	ctx := context.Background()

	mail, err := client.WaitForMail(ctx, 5*time.Second, MailFilter{
		FromSuffix:      "@example.com",
		SubjectContains: "Welcome",
	})
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if mail == nil {
		t.Fatal("expected mail, got nil")
	}
	if mail.ID != "1" {
		t.Errorf("expected mail ID 1, got %s", mail.ID)
	}
	if mail.From != "support@example.com" {
		t.Errorf("expected from support@example.com, got %s", mail.From)
	}
}

func TestWaitForMail_FilterDoesNotMatch(t *testing.T) {
	restore := withFastPoll(t)
	defer restore()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{
				{
					"id":      1,
					"subject": "Spam",
					"text":    "",
					"html":    "",
					"from":    "spammer@bad.com",
				},
			},
		})
	}))
	defer server.Close()

	client := newCloudflareClient(server.URL)
	ctx := context.Background()

	_, err := client.WaitForMail(ctx, 50*time.Millisecond, MailFilter{
		FromSuffix:      "@example.com",
		SubjectContains: "Welcome",
	})
	if err == nil {
		t.Fatal("expected timeout error, got nil")
	}
	if !strings.Contains(err.Error(), "not received within") {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestWaitForMail_MultipleMessagesOneMatches(t *testing.T) {
	restore := withFastPoll(t)
	defer restore()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{
				{
					"id":      1,
					"subject": "Spam",
					"text":    "",
					"html":    "",
					"from":    "spammer@bad.com",
				},
				{
					"id":      2,
					"subject": "Your account is ready",
					"text":    "Welcome to Example!",
					"html":    "",
					"from":    "noreply@example.com",
				},
				{
					"id":      3,
					"subject": "Another email",
					"text":    "irrelevant",
					"html":    "",
					"from":    "other@test.com",
				},
			},
		})
	}))
	defer server.Close()

	client := newCloudflareClient(server.URL)
	ctx := context.Background()

	mail, err := client.WaitForMail(ctx, 5*time.Second, MailFilter{
		FromSuffix:      "@example.com",
		SubjectContains: "account",
	})
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if mail == nil {
		t.Fatal("expected mail, got nil")
	}
	if mail.ID != "2" {
		t.Errorf("expected mail ID 2 (matching), got %s", mail.ID)
	}
}

// ─── TestFetchRawMails ────────────────────────────────────────────────────────

func TestFetchRawMails_PrimaryFormat(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/api/mails" {
			t.Errorf("unexpected path: %s", r.URL.Path)
		}
		json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{
				{
					"id":      42,
					"subject": "Hello",
					"text":    "World",
					"html":    "<p>World</p>",
					"from":    "alice@test.com",
				},
			},
		})
	}))
	defer server.Close()

	client := newCloudflareClient(server.URL)
	mails, err := client.FetchRawMails()
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if len(mails) != 1 {
		t.Fatalf("expected 1 mail, got %d", len(mails))
	}
	if mails[0].ID != "42" {
		t.Errorf("expected ID 42, got %s", mails[0].ID)
	}
	if mails[0].Subject != "Hello" {
		t.Errorf("expected Subject Hello, got %s", mails[0].Subject)
	}
	if mails[0].From != "alice@test.com" {
		t.Errorf("expected From alice@test.com, got %s", mails[0].From)
	}
	if mails[0].Text != "World" {
		t.Errorf("expected Text World, got %s", mails[0].Text)
	}
	if mails[0].HTML != "<p>World</p>" {
		t.Errorf("expected HTML <p>World</p>, got %s", mails[0].HTML)
	}
}

func TestFetchRawMails_MailTMFormat(t *testing.T) {
	callCount := 0
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callCount++
		switch r.URL.Path {
		case "/messages":
			json.NewEncoder(w).Encode(map[string]any{
				"hydra:member": []map[string]any{
					{
						"id":      "msg-1",
						"subject": "Hi there",
						"from": map[string]string{
							"address": "bob@test.com",
							"name":    "Bob",
						},
						"intro": "Short intro",
					},
				},
			})
		default:
			if strings.HasPrefix(r.URL.Path, "/messages/") {
				json.NewEncoder(w).Encode(map[string]string{
					"text": "Full body text",
					"html": "<p>Full body</p>",
				})
				return
			}
			w.WriteHeader(http.StatusNotFound)
		}
	}))
	defer server.Close()

	client := &CloudflareTempClient{
		hc:        &http.Client{},
		apiBase:   mailtmAPIBase,
		address:   "test@test.com",
		jwt:       "mock-token",
		cachedIDs: make(map[string]struct{}),
	}

	restore := withHostRewrite("api.mail.tm", server.URL)
	defer restore()

	mails, err := client.FetchRawMails()
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if len(mails) != 1 {
		t.Fatalf("expected 1 mail, got %d", len(mails))
	}
	if mails[0].ID != "msg-1" {
		t.Errorf("expected ID msg-1, got %s", mails[0].ID)
	}
	if mails[0].Subject != "Hi there" {
		t.Errorf("expected Subject 'Hi there', got %s", mails[0].Subject)
	}
	if mails[0].From != "bob@test.com" {
		t.Errorf("expected From bob@test.com, got %s", mails[0].From)
	}
	if mails[0].Text != "Full body text" {
		t.Errorf("expected full body text, got %s", mails[0].Text)
	}
}

func TestFetchRawMails_EmptyInbox(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{
			"results": []any{},
		})
	}))
	defer server.Close()

	client := newCloudflareClient(server.URL)
	mails, err := client.FetchRawMails()
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if len(mails) != 0 {
		t.Errorf("expected 0 mails, got %d", len(mails))
	}
}

// ─── TestSmartExtractCode ─────────────────────────────────────────────────────

func TestSmartExtractCode(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"6-digit with verification label", "Your verification code is 123456", "123456"},
		{"6-digit with code label", "code: 987654", "987654"},
		{"6-digit with OTP label", "OTP: 456789", "456789"},
		{"6-digit with pin label", "pin: 112233", "112233"},
		{"4-digit with Chinese 验证码 label", "您的验证码是1234", "1234"},
		{"4-digit with Chinese 确认码 label", "确认码: 5678", "5678"},
		{"4-digit as verification label", "Your verification code is 3344", "3344"},
		{"8-digit with code label", "code: 12345678", "12345678"},
		{"6-digit bare number", "Your number is 123456", "123456"},
		{"5-digit bare number", "your pin is 12345", "12345"},
		{"7-digit bare number", "number 7654321", "7654321"},
		{"8-digit bare number", "long code 87654321", "87654321"},
		{"bare 4-digit number", "Your code is 1234", "1234"},
		{"3-digit only", "Number 123 is small", ""},
		{"empty string", "", ""},
		{"no digits at all", "hello world", ""},
		{"code in middle of longer number", "value 1234567 is here", "1234567"},
		{"keyword: is your code", "1234 is your verification code", "1234"},
		{"keyword: code was", "567890 was your code", "567890"},
		{"keyword - Chinese 是验证码", "123456是您的验证码", "123456"},
		{"keyword - Chinese 为验证码", "123456为你的验证码", "123456"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := smartExtractCode(tt.input)
			if got != tt.expected {
				t.Errorf("smartExtractCode(%q) = %q; want %q", tt.input, got, tt.expected)
			}
		})
	}
}

// ─── TestExtractCodeFromMail ──────────────────────────────────────────────────

func TestExtractCodeFromMail_SubjectFirst(t *testing.T) {
	mail := &MailMessage{
		Subject: "Your code is 555666",
		Text:    "No code here",
		HTML:    "<p>irrelevant</p>",
	}
	code := ExtractCodeFromMail(mail, nil)
	if code != "555666" {
		t.Errorf("expected 555666 from subject, got %s", code)
	}
}

func TestExtractCodeFromMail_TextWhenSubjectEmpty(t *testing.T) {
	mail := &MailMessage{
		Subject: "",
		Text:    "Your verification code is 777888",
		HTML:    "",
	}
	code := ExtractCodeFromMail(mail, nil)
	if code != "777888" {
		t.Errorf("expected 777888 from text, got %s", code)
	}
}

func TestExtractCodeFromMail_HTMLOnly(t *testing.T) {
	mail := &MailMessage{
		Subject: "",
		Text:    "",
		HTML:    "<html><body><p>Your otp is 998877</p></body></html>",
	}
	code := ExtractCodeFromMail(mail, nil)
	if code != "998877" {
		t.Errorf("expected 998877 from stripped HTML, got %s", code)
	}
}

func TestExtractCodeFromMail_CustomPattern(t *testing.T) {
	mail := &MailMessage{
		Subject: "Order #ABC-12345-XYZ",
		Text:    "Your tracking number is XYZ-99999",
		HTML:    "",
	}
	customPat := regexp.MustCompile(`([A-Z]+-\d+)`)
	code := ExtractCodeFromMail(mail, customPat)
	// The custom pattern would match "ABC-12345" in the subject (first hit)
	if code != "ABC-12345" {
		t.Errorf("expected ABC-12345 from custom pattern, got %s", code)
	}
}

func TestExtractCodeFromMail_CustomPatternPriority(t *testing.T) {
	mail := &MailMessage{
		Subject: "Your code is 123456 but custom says ABC-999",
		Text:    "",
		HTML:    "",
	}
	// Custom pattern matched before built-in.
	customPat := regexp.MustCompile(`([A-Z]+-\d+)`)
	code := ExtractCodeFromMail(mail, customPat)
	if code != "ABC-999" {
		t.Errorf("expected ABC-999 from custom pattern, got %s", code)
	}
}

func TestExtractCodeFromMail_NoCode(t *testing.T) {
	mail := &MailMessage{
		Subject: "Welcome!",
		Text:    "Thank you for signing up.",
		HTML:    "",
	}
	code := ExtractCodeFromMail(mail, nil)
	if code != "" {
		t.Errorf("expected empty code, got %s", code)
	}
}

// ─── TestAllCodesFromMail ─────────────────────────────────────────────────────

func TestAllCodesFromMail_MultipleCodes(t *testing.T) {
	mail := &MailMessage{
		Subject: "Codes: 111111 and 222222",
		Text:    "Also verify with 333333 and 444444",
		HTML:    "",
	}
	codes := AllCodesFromMail(mail)
	if len(codes) != 4 {
		t.Errorf("expected 4 codes, got %d: %v", len(codes), codes)
	}
}

func TestAllCodesFromMail_Deduplication(t *testing.T) {
	mail := &MailMessage{
		Subject: "Your code is 123456",
		Text:    "Repeat: 123456",
		HTML:    "",
	}
	codes := AllCodesFromMail(mail)
	if len(codes) != 1 {
		t.Errorf("expected 1 unique code, got %d: %v", len(codes), codes)
	}
	if codes[0] != "123456" {
		t.Errorf("expected 123456, got %s", codes[0])
	}
}

func TestAllCodesFromMail_OrderPreserved(t *testing.T) {
	mail := &MailMessage{
		Subject: "First code: 111111",
		Text:    "Second: 222222 and third: 333333",
		HTML:    "",
	}
	codes := AllCodesFromMail(mail)
	if len(codes) < 3 {
		t.Fatalf("expected at least 3 codes, got %d", len(codes))
	}
	if codes[0] != "111111" {
		t.Errorf("expected first=111111, got %s", codes[0])
	}
	if codes[1] != "222222" {
		t.Errorf("expected second=222222, got %s", codes[1])
	}
	if codes[2] != "333333" {
		t.Errorf("expected third=333333, got %s", codes[2])
	}
}

func TestAllCodesFromMail_SubjectAndTextAndHTML(t *testing.T) {
	mail := &MailMessage{
		Subject: "Subject: 111111",
		Text:    "Text: 222222",
		HTML:    "<p>HTML: 333333</p>",
	}
	codes := AllCodesFromMail(mail)
	if len(codes) != 3 {
		t.Errorf("expected 3 codes from all sources, got %d: %v", len(codes), codes)
	}
}

func TestAllCodesFromMail_NoCodes(t *testing.T) {
	mail := &MailMessage{
		Subject: "No numbers here",
		Text:    "Just text",
		HTML:    "",
	}
	codes := AllCodesFromMail(mail)
	if len(codes) != 0 {
		t.Errorf("expected 0 codes, got %d: %v", len(codes), codes)
	}
}

// ─── TestStripHTML ────────────────────────────────────────────────────────────

func TestStripHTML(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"removes script tags", "<script>alert('x')</script>hello", "hello"},
		{"removes style tags", "<style>body{}</style>content", "content"},
		{"replaces block tags with newlines", "<p>hello</p><div>world</div>", "hello world"},
		{"decodes &amp;", "a &amp; b", "a & b"},
		{"decodes &lt; &gt;", "&lt;tag&gt;", "<tag>"},
		{"decodes &quot;", `&quot;quote&quot;`, `"quote"`},
		{"decodes &nbsp;", "hello&nbsp;world", "hello world"},
		{"decodes &ndash; &mdash;", "a&ndash;b&mdash;c", "a-b-c"},
		{"collapses whitespace", "hello   world", "hello world"},
		{"trims outer whitespace", "  hello world  ", "hello world"},
		{"empty string", "", ""},
		{"block tags add newlines then collapse", "<p>line1</p><p>line2</p>", "line1 line2"},
		{"nested block and inline", "<div><span>text</span></div>", "text"},
		{"self-closing br", "line1<br>line2", "line1 line2"},
		{"script with newlines inside", "<script>\n var x = 1;\n</script>out", "out"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := stripHTML(tt.input)
			if got != tt.expected {
				t.Errorf("stripHTML(%q) = %q; want %q", tt.input, got, tt.expected)
			}
		})
	}
}

// ─── TestMailTMClient ─────────────────────────────────────────────────────────

func TestMailTMClient_Create(t *testing.T) {
	server := newMailTMMockServer(t, nil)
	defer server.Close()

	restore := withHostRewrite("api.mail.tm", server.URL)
	defer restore()

	client, err := NewMailTMClientWithAddress("testuser")
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if !strings.Contains(client.Address(), "testuser@") {
		t.Errorf("expected address containing testuser@, got %s", client.Address())
	}
}

func TestMailTMClient_WaitForCode_6Digit(t *testing.T) {
	msgHandler := func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/messages":
			json.NewEncoder(w).Encode(map[string]any{
				"hydra:member": []map[string]any{
					{
						"id":      "m1",
						"subject": "Your verification code is 654321",
						"from":    map[string]string{"address": "noreply@test.com", "name": "Test"},
						"intro":   "",
					},
				},
			})
		default:
			if strings.HasPrefix(r.URL.Path, "/messages/") {
				json.NewEncoder(w).Encode(map[string]string{
					"text": "",
					"html": "",
				})
				return
			}
			w.WriteHeader(http.StatusNotFound)
		}
	}

	server := newMailTMMockServer(t, msgHandler)
	defer server.Close()

	restore := withHostRewrite("api.mail.tm", server.URL)
	defer restore()

	client, err := NewMailTMClientWithAddress("code6")
	if err != nil {
		t.Fatalf("create client: %v", err)
	}

	code, err := client.WaitForCode()
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if code != "654321" {
		t.Errorf("expected code 654321, got %s", code)
	}
}

func TestMailTMClient_WaitForCode_4DigitWithLabel(t *testing.T) {
	msgHandler := func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/messages":
			json.NewEncoder(w).Encode(map[string]any{
				"hydra:member": []map[string]any{
					{
						"id":      "m4",
						"subject": "Your pin: 1234",
						"from":    map[string]string{"address": "noreply@test.com", "name": "Test"},
						"intro":   "",
					},
				},
			})
		default:
			if strings.HasPrefix(r.URL.Path, "/messages/") {
				json.NewEncoder(w).Encode(map[string]string{
					"text": "",
					"html": "",
				})
			}
		}
	}

	server := newMailTMMockServer(t, msgHandler)
	defer server.Close()

	restore := withHostRewrite("api.mail.tm", server.URL)
	defer restore()

	client, err := NewMailTMClientWithAddress("code4")
	if err != nil {
		t.Fatalf("create client: %v", err)
	}

	code, err := client.WaitForCode()
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if code != "1234" {
		t.Errorf("expected code 1234, got %s", code)
	}
}

func TestMailTMClient_WaitForCode_8Digit(t *testing.T) {
	msgHandler := func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/messages":
			json.NewEncoder(w).Encode(map[string]any{
				"hydra:member": []map[string]any{
					{
						"id":      "m8",
						"subject": "Your code: 87654321",
						"from":    map[string]string{"address": "noreply@test.com", "name": "Test"},
						"intro":   "",
					},
				},
			})
		default:
			if strings.HasPrefix(r.URL.Path, "/messages/") {
				json.NewEncoder(w).Encode(map[string]string{
					"text": "",
					"html": "",
				})
			}
		}
	}

	server := newMailTMMockServer(t, msgHandler)
	defer server.Close()

	restore := withHostRewrite("api.mail.tm", server.URL)
	defer restore()

	client, err := NewMailTMClientWithAddress("code8")
	if err != nil {
		t.Fatalf("create client: %v", err)
	}

	code, err := client.WaitForCode()
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if code != "87654321" {
		t.Errorf("expected code 87654321, got %s", code)
	}
}

// ─── TestNamesGeneration ──────────────────────────────────────────────────────

func TestGenerateHumanEmail(t *testing.T) {
	email := GenerateHumanEmail("example.com")
	if !strings.HasSuffix(email, "@example.com") {
		t.Errorf("expected suffix @example.com, got %s", email)
	}

	parts := strings.Split(strings.TrimSuffix(email, "@example.com"), ".")
	if len(parts) != 2 {
		t.Errorf("expected format firstname.lastnameNN, got %s", strings.TrimSuffix(email, "@example.com"))
	}

	if len(parts) == 2 {
		namePart := parts[1]
		if len(namePart) < 2 {
			t.Errorf("expected lastName part to include suffix digits, got %s", namePart)
		}
		// last digits should be numbers
		suffixStr := namePart
		var i int
		for i = len(suffixStr) - 1; i >= 0 && suffixStr[i] >= '0' && suffixStr[i] <= '9'; i-- {
		}
		if i == len(suffixStr)-1 {
			t.Errorf("expected numeric suffix after last name, got %s", suffixStr)
		}
	}
}

func TestGenerateHumanEmail_Multiple(t *testing.T) {
	seen := make(map[string]bool)
	for range 20 {
		email := GenerateHumanEmail("test.com")
		if seen[email] {
			t.Errorf("duplicate email generated: %s", email)
		}
		seen[email] = true
	}
}

func TestGenerateHumanPassword(t *testing.T) {
	for range 10 {
		pw := GenerateHumanPassword()
		if len(pw) < 8 {
			t.Errorf("password too short: %s", pw)
		}
		if !strings.HasSuffix(pw, "!") {
			t.Errorf("expected password ending with !, got %s", pw)
		}
		if !strings.ContainsAny(pw, "0123456789") {
			t.Errorf("expected password containing digits, got %s", pw)
		}
	}
}

func TestGenerateHumanPassword_Multiple(t *testing.T) {
	seen := make(map[string]bool)
	for range 20 {
		pw := GenerateHumanPassword()
		if seen[pw] {
			t.Errorf("duplicate password generated: %s", pw)
		}
		seen[pw] = true
	}
}

func TestSecureHex_Length(t *testing.T) {
	for _, n := range []int{0, 1, 4, 8, 16, 32, 64} {
		hex := SecureHex(n)
		if len(hex) != n {
			t.Errorf("SecureHex(%d) returned length %d", n, len(hex))
		}
	}
}

func TestSecureHex_ValidHex(t *testing.T) {
	hex := SecureHex(100)
	for _, c := range hex {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
			t.Errorf("invalid hex character: %c", c)
		}
	}
}

func TestSecureHex_Zero(t *testing.T) {
	if got := SecureHex(0); got != "" {
		t.Errorf("SecureHex(0) expected empty, got %s", got)
	}
}

func TestSecureHex_Negative(t *testing.T) {
	if got := SecureHex(-5); got != "" {
		t.Errorf("SecureHex(-5) expected empty, got %s", got)
	}
}

func TestSecureHex_MultipleUnique(t *testing.T) {
	seen := make(map[string]bool)
	for range 50 {
		hex := SecureHex(16)
		if seen[hex] {
			t.Errorf("duplicate SecureHex(16): %s", hex)
		}
		seen[hex] = true
	}
}
