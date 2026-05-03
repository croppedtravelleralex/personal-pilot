package proxy

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"personal-pilot/backend/internal/config"
)

func TestFetchProxyIPInfoFallsBackToIPAPI(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/ippure":
			http.Error(w, "temporary failure", http.StatusBadGateway)
		case "/ip-api":
			w.Header().Set("Content-Type", "application/json")
			_, _ = w.Write([]byte(`{"status":"success","query":"203.0.113.10","country":"United States","countryCode":"US","regionName":"California","city":"Los Angeles","as":"AS64500 Example","isp":"Example ISP","org":"Example Org","hosting":false,"proxy":false}`))
		default:
			http.NotFound(w, r)
		}
	}))
	defer server.Close()

	originalEndpoints := proxyIPInfoEndpoints
	proxyIPInfoEndpoints = []proxyIPInfoEndpoint{
		{source: "ippure", url: server.URL + "/ippure"},
		{source: "ip-api", url: server.URL + "/ip-api"},
	}
	defer func() {
		proxyIPInfoEndpoints = originalEndpoints
	}()

	result, err := FetchProxyIPInfo("p1", []config.BrowserProxy{
		{ProxyId: "p1", ProxyConfig: "direct://"},
	}, nil, nil)
	if err != nil {
		t.Fatalf("FetchProxyIPInfo returned error: %v", err)
	}
	if got := mapStringValue(result, "source"); got != "ip-api" {
		t.Fatalf("source = %q, want ip-api", got)
	}
	if got := mapStringValue(result, "query"); got != "203.0.113.10" {
		t.Fatalf("query = %q, want 203.0.113.10", got)
	}
	if got := mapBoolValue(result, "hosting"); got {
		t.Fatalf("hosting = true, want false")
	}
}
