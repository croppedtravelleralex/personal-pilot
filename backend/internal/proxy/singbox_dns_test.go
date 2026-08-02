package proxy

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	"personal-pilot/backend/internal/config"
)

func TestBuildSingBoxDNSOmitsLegacyDefaultServers(t *testing.T) {
	t.Parallel()
	if block := buildSingBoxDNS(""); block != nil {
		t.Fatalf("expected nil DNS block for empty input, got %#v", block)
	}
}

func TestBuildSingBoxDNSUsesNewServerFormat(t *testing.T) {
	t.Parallel()
	block := buildSingBoxDNS("https://1.1.1.1/dns-query,8.8.8.8")
	if block == nil {
		t.Fatal("expected DNS block")
	}
	servers, ok := block["servers"].([]interface{})
	if !ok || len(servers) != 2 {
		t.Fatalf("unexpected servers: %#v", block["servers"])
	}
	first, ok := servers[0].(map[string]interface{})
	if !ok || first["type"] != "https" || first["server"] != "1.1.1.1" || first["tag"] != "remote" {
		t.Fatalf("unexpected first server: %#v", first)
	}
	second, ok := servers[1].(map[string]interface{})
	if !ok || second["type"] != "udp" || second["server"] != "8.8.8.8" {
		t.Fatalf("unexpected second server: %#v", second)
	}
}

func TestSingBoxGeneratedConfigPassesCheck(t *testing.T) {
	t.Parallel()

	outbound := map[string]interface{}{
		"type":        "socks",
		"tag":         "proxy-out",
		"server":      "127.0.0.1",
		"server_port": 1080,
		"version":     "5",
	}
	manager := &SingBoxManager{
		AppRoot: t.TempDir(),
		Config:  &config.Config{Browser: config.BrowserConfig{UserDataRoot: t.TempDir()}},
	}
	cfgPath, err := manager.buildConfig("test-key", outbound, 21080, "")
	if err != nil {
		t.Fatalf("buildConfig failed: %v", err)
	}

	binary := filepath.Join("..", "..", "..", "bin", "sing-box.exe")
	if _, statErr := os.Stat(binary); statErr != nil {
		t.Skip("sing-box.exe not available for config check")
	}
	cmd := exec.Command(binary, "check", "-c", cfgPath)
	out, runErr := cmd.CombinedOutput()
	if runErr != nil {
		t.Fatalf("sing-box check failed: %v\n%s", runErr, string(out))
	}

	raw, err := os.ReadFile(cfgPath)
	if err != nil {
		t.Fatalf("read config: %v", err)
	}
	var parsed map[string]interface{}
	if err := json.Unmarshal(raw, &parsed); err != nil {
		t.Fatalf("parse config json: %v", err)
	}
	if _, hasDNS := parsed["dns"]; hasDNS {
		t.Fatalf("bridge config should omit default dns block, got %#v", parsed["dns"])
	}
}
