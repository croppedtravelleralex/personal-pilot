package proxy

import (
	"os/exec"
	"testing"

	"personal-pilot/backend/internal/config"
)

func TestSingBoxAcquireBridgePinsRegisteredBridge(t *testing.T) {
	t.Parallel()

	src := "socks5://user:pass@127.0.0.1:1080"
	manager := &SingBoxManager{
		Bridges: make(map[string]*SingBoxBridge),
	}
	key := manager.singBoxBridgeKey(src, nil, "")
	cmd := exec.Command("go", "version")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start helper process: %v", err)
	}
	t.Cleanup(func() {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
	})
	bridge := &SingBoxBridge{
		NodeKey: key,
		Port:    21001,
		Cmd:     cmd,
		Running: true,
	}
	manager.Bridges[key] = bridge

	if !manager.pinBridge(key) {
		t.Fatal("expected pinBridge to succeed for registered bridge key")
	}
	if bridge.RefCount != 1 {
		t.Fatalf("expected ref count 1, got %d", bridge.RefCount)
	}
}

func TestSingBoxBridgeKeyMatchesEnsureBridgeLookup(t *testing.T) {
	t.Parallel()

	proxies := []config.BrowserProxy{{
		ProxyId:     "node-1",
		ProxyConfig: "socks5://user:pass@127.0.0.1:1080",
		DnsServers:  "8.8.8.8",
	}}
	src := normalizeNodeScheme(NormalizeStandardProxyScheme(proxies[0].ProxyConfig))
	manager := &SingBoxManager{}
	key := manager.singBoxBridgeKey(src, proxies, "node-1")
	want := computeNodeKey(src + "\x00" + "8.8.8.8")
	if key != want {
		t.Fatalf("bridge key mismatch: got %q want %q", key, want)
	}
}
