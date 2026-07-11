package launchcode

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func parseProxyForTest(t *testing.T, raw string, protocol string) (int, map[string]interface{}, string) {
	t.Helper()
	srv := NewLaunchServer(NewLaunchCodeService(NewMemoryLaunchCodeDAO()), nil, nil, nil, 0)
	body := map[string]string{"raw": raw}
	if strings.TrimSpace(protocol) != "" {
		body["protocol"] = protocol
	}
	payload, err := json.Marshal(body)
	if err != nil {
		t.Fatalf("marshal request: %v", err)
	}
	req := httptest.NewRequest(http.MethodPost, "/api/proxy/parse", strings.NewReader(string(payload)))
	req.Header.Set("Content-Type", "application/json")
	rr := httptest.NewRecorder()
	NewTestHandler(srv).ServeHTTP(rr, req)

	var decoded map[string]interface{}
	if err := json.Unmarshal(rr.Body.Bytes(), &decoded); err != nil {
		t.Fatalf("decode response: %v body=%s", err, rr.Body.String())
	}
	return rr.Code, decoded, rr.Body.String()
}

func TestParseProxySupportsDirectImportFormats(t *testing.T) {
	cases := []struct {
		name     string
		raw      string
		protocol string
		wantHost string
		wantPort int
		wantKind string
	}{
		{
			name:     "server port username password",
			raw:      "70.39.164.200:30000:a7zxcut:Y8sA6rtN",
			wantHost: "70.39.164.200",
			wantPort: 30000,
			wantKind: "http",
		},
		{
			name:     "server port at username password",
			raw:      "proxy.example.com:30000@user:pass",
			protocol: "socks",
			wantHost: "proxy.example.com",
			wantPort: 30000,
			wantKind: "socks5",
		},
		{
			name:     "username password server port",
			raw:      "user:pass:proxy.example.com:30000",
			wantHost: "proxy.example.com",
			wantPort: 30000,
			wantKind: "http",
		},
		{
			name:     "username numeric password at server port",
			raw:      "user:123456@proxy.example.com:30000",
			wantHost: "proxy.example.com",
			wantPort: 30000,
			wantKind: "http",
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			status, payload, rawBody := parseProxyForTest(t, tc.raw, tc.protocol)
			if status != http.StatusOK {
				t.Fatalf("status = %d, body=%s", status, rawBody)
			}
			if payload["ok"] != true {
				t.Fatalf("ok = %#v, body=%s", payload["ok"], rawBody)
			}
			if payload["format"] != "direct" {
				t.Fatalf("format = %#v, want direct", payload["format"])
			}
			if payload["host"] != tc.wantHost {
				t.Fatalf("host = %#v, want %s", payload["host"], tc.wantHost)
			}
			if int(payload["port"].(float64)) != tc.wantPort {
				t.Fatalf("port = %#v, want %d", payload["port"], tc.wantPort)
			}
			if payload["protocol"] != tc.wantKind {
				t.Fatalf("protocol = %#v, want %s", payload["protocol"], tc.wantKind)
			}
			if payload["hasAuth"] != true {
				t.Fatalf("hasAuth = %#v, want true", payload["hasAuth"])
			}
			if strings.Contains(rawBody, "Y8sA6rtN") || strings.Contains(rawBody, "pass") || strings.Contains(rawBody, "123456") {
				t.Fatalf("parse response leaked proxy password: %s", rawBody)
			}
		})
	}
}

func TestParseProxyNormalizesStandardURLAndPreservesQuery(t *testing.T) {
	status, payload, rawBody := parseProxyForTest(t, "socks://user:pass@p.webshare.io:1080?pp_via_ssh=panda", "")
	if status != http.StatusOK {
		t.Fatalf("status = %d, body=%s", status, rawBody)
	}
	if payload["format"] != "url" {
		t.Fatalf("format = %#v, want url", payload["format"])
	}
	if payload["protocol"] != "socks5" {
		t.Fatalf("protocol = %#v, want socks5", payload["protocol"])
	}
	if payload["host"] != "p.webshare.io" {
		t.Fatalf("host = %#v", payload["host"])
	}
	if int(payload["port"].(float64)) != 1080 {
		t.Fatalf("port = %#v", payload["port"])
	}
	proxyConfig, _ := payload["proxyConfig"].(string)
	if !strings.Contains(proxyConfig, "pp_via_ssh=panda") {
		t.Fatalf("proxyConfig did not preserve query: %s", proxyConfig)
	}
	if strings.Contains(rawBody, "user:pass") || strings.Contains(rawBody, "pass@") {
		t.Fatalf("parse response leaked standard URL password: %s", rawBody)
	}
}
