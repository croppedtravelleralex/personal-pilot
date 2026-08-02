package proxy

import (
	"strings"
	"testing"

	"personal-pilot/backend/internal/config"
)

func TestValidateDirectFallbackSwitchRejectsBridgedToDirect(t *testing.T) {
	t.Parallel()

	proxies := []config.BrowserProxy{
		{ProxyId: "socks-node", ProxyConfig: "socks5://user:pass@1.2.3.4:1080"},
	}
	err := ValidateDirectFallbackSwitch(
		"socks5://user:pass@1.2.3.4:1080",
		proxies,
		"socks-node",
		"__direct__",
		"direct://",
		false,
	)
	if err == nil {
		t.Fatal("expected bridged-to-direct fallback to be rejected")
	}
	if !strings.Contains(err.Error(), DirectProxyTagAllowFallback) {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestValidateDirectFallbackSwitchAllowsWithTag(t *testing.T) {
	t.Parallel()

	proxies := []config.BrowserProxy{
		{ProxyId: "socks-node", ProxyConfig: "socks5://user:pass@1.2.3.4:1080"},
	}
	if err := ValidateDirectFallbackSwitch(
		"socks5://user:pass@1.2.3.4:1080",
		proxies,
		"socks-node",
		"__direct__",
		"direct://",
		true,
	); err != nil {
		t.Fatalf("expected allow fallback: %v", err)
	}
}
