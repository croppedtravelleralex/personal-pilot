package proxy

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"personal-pilot/backend/internal/config"
)

func TestLiveSSHTunnelProxyIPHealthFromEnv(t *testing.T) {
	rawURL := strings.TrimSpace(os.Getenv("PERSONAL_PILOT_PROXY_SMOKE_URL"))
	if rawURL == "" {
		t.Skip("PERSONAL_PILOT_PROXY_SMOKE_URL not set")
	}
	if !HasSSHTunnelDirective(rawURL) {
		t.Fatalf("PERSONAL_PILOT_PROXY_SMOKE_URL must include pp_via_ssh")
	}

	root := strings.TrimSpace(os.Getenv("PERSONAL_PILOT_APP_ROOT"))
	if root == "" {
		var err error
		root, err = filepath.Abs("../../..")
		if err != nil {
			t.Fatalf("resolve app root: %v", err)
		}
	}

	cfg := config.DefaultConfig()
	manager := NewSingBoxManager(cfg, root)
	defer manager.StopAll()

	proxies := []config.BrowserProxy{{
		ProxyId:     "live-ssh-smoke",
		ProxyName:   "live-ssh-smoke",
		ProxyConfig: rawURL,
	}}
	result, err := FetchProxyIPInfo(context.Background(), "live-ssh-smoke", proxies, []BridgeManager{manager})
	if err != nil {
		t.Fatalf("FetchProxyIPInfo via SSH tunnel failed: %v", err)
	}
	if strings.TrimSpace(firstNonEmpty(mapStringValue(result, "ip"), mapStringValue(result, "query"))) == "" {
		t.Fatalf("proxy IP health returned no observed IP, source=%s", mapStringValue(result, "source"))
	}
}
