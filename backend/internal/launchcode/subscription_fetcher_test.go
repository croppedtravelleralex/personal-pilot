package launchcode

import (
	"context"
	"encoding/base64"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestNormalizeSubscriptionSourceURL(t *testing.T) {
	got, err := normalizeSubscriptionSourceURL(" HTTPS://Example.COM/sub/path?token=secret#ignored ")
	if err != nil {
		t.Fatalf("normalize: %v", err)
	}
	if got != "https://example.com/sub/path?token=secret" {
		t.Fatalf("normalized URL = %q", got)
	}
	for _, raw := range []string{"", "file:///tmp/sub", "http://user:pass@example.com/sub", "https:///missing-host"} {
		if _, err := normalizeSubscriptionSourceURL(raw); err == nil {
			t.Fatalf("expected %q to be rejected", raw)
		}
	}
}

func TestParseSubscriptionPayloadSupportsBase64URLListAndClash(t *testing.T) {
	plain := "socks5://127.0.0.1:1080#local%20socks\nhttp://user:secret@proxy.example:8080#http-node\n"
	encoded := base64.StdEncoding.EncodeToString([]byte(plain))
	nodes, err := parseSubscriptionPayload([]byte(encoded))
	if err != nil {
		t.Fatalf("parse base64: %v", err)
	}
	if len(nodes) != 2 || nodes[0].Name != "local socks" || strings.Contains(nodes[0].ProxyConfig, "#") {
		t.Fatalf("base64 nodes = %+v", nodes)
	}

	clash := `proxies:
  - name: clash-http
    type: http
    server: proxy.example
    port: 8080
    username: user
    password: secret
  - name: clash-vmess
    type: vmess
    server: vmess.example
    port: 443
    uuid: 00000000-0000-0000-0000-000000000001
`
	nodes, err = parseSubscriptionPayload([]byte(clash))
	if err != nil {
		t.Fatalf("parse clash: %v", err)
	}
	if len(nodes) != 2 || nodes[0].Name != "clash-http" || !strings.HasPrefix(nodes[0].ProxyConfig, "http://") || !strings.Contains(nodes[1].ProxyConfig, "type: vmess") {
		t.Fatalf("clash nodes = %+v", nodes)
	}
}

func TestHTTPSubscriptionFetcherAppliesStatusAndSizeBoundaries(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/ok":
			_, _ = w.Write([]byte("socks5://127.0.0.1:1080"))
		case "/large":
			_, _ = w.Write([]byte(strings.Repeat("x", maxSubscriptionPayloadBytes+1)))
		case "/redirect-with-credentials":
			http.Redirect(w, r, "http://user:pass@example.com/sub", http.StatusFound)
		default:
			http.Error(w, "bad", http.StatusBadGateway)
		}
	}))
	defer server.Close()

	fetcher := NewHTTPSubscriptionFetcher(server.Client())
	nodes, err := fetcher.Fetch(context.Background(), server.URL+"/ok")
	if err != nil || len(nodes) != 1 {
		t.Fatalf("Fetch ok nodes=%+v err=%v", nodes, err)
	}
	if _, err := fetcher.Fetch(context.Background(), server.URL+"/bad"); err == nil {
		t.Fatal("expected non-2xx error")
	}
	if _, err := fetcher.Fetch(context.Background(), server.URL+"/large"); err == nil {
		t.Fatal("expected payload size error")
	}
	defaultFetcher := NewHTTPSubscriptionFetcher(nil)
	if _, err := defaultFetcher.Fetch(context.Background(), server.URL+"/redirect-with-credentials"); err == nil {
		t.Fatal("expected redirect with embedded credentials to be rejected")
	}
}
