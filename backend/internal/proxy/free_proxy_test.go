package proxy

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

func TestParseFreeProxyCandidatesInfersSchemeAndDeduplicates(t *testing.T) {
	raw := strings.Join([]string{
		"1.1.1.1:8080",
		"http://2.2.2.2:3128",
		"socks5://3.3.3.3:1080",
		"invalid",
		"1.1.1.1:8080",
	}, "\n")

	parsed := ParseFreeProxyCandidates(raw, "https://example.test/socks5.txt")
	unique := DeduplicateFreeProxyCandidates(parsed)

	if len(unique) != 3 {
		t.Fatalf("expected 3 unique candidates, got %d: %#v", len(unique), unique)
	}
	if unique[0].Protocol != "socks5" || unique[0].ProxyConfig != "socks5://1.1.1.1:8080" {
		t.Fatalf("expected source scheme inference for host:port, got %#v", unique[0])
	}
	if unique[1].Protocol != "http" || unique[1].ProxyConfig != "http://2.2.2.2:3128" {
		t.Fatalf("expected explicit http URL, got %#v", unique[1])
	}
}

func TestFetchFreeProxyCandidatesKeepsSourceErrors(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/bad" {
			http.Error(w, "nope", http.StatusInternalServerError)
			return
		}
		_, _ = w.Write([]byte("4.4.4.4:8080\n4.4.4.4:8080\n5.5.5.5:3128\n"))
	}))
	defer server.Close()

	candidates, sourceErrors := FetchFreeProxyCandidates(
		context.Background(),
		[]string{server.URL + "/ok", server.URL + "/bad"},
		10,
		time.Second,
	)

	if len(candidates) != 2 {
		t.Fatalf("expected 2 deduplicated candidates, got %d", len(candidates))
	}
	if len(sourceErrors) != 1 || !strings.Contains(sourceErrors[0], "HTTP 500") {
		t.Fatalf("expected one source error with HTTP 500, got %#v", sourceErrors)
	}
}

func TestNormalizeFreeProxyBounds(t *testing.T) {
	if got := NormalizeFreeProxyConcurrency(0); got != 20 {
		t.Fatalf("expected default concurrency 20, got %d", got)
	}
	if got := NormalizeFreeProxyConcurrency(1000); got != 100 {
		t.Fatalf("expected max concurrency 100, got %d", got)
	}
	if got := NormalizeFreeProxyLimit(0); got != defaultFreeProxyLimit {
		t.Fatalf("expected default limit, got %d", got)
	}
	if got := NormalizeFreeProxyLimit(99999); got != 5000 {
		t.Fatalf("expected max limit 5000, got %d", got)
	}
}
