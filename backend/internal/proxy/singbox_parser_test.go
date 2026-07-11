package proxy

import "testing"

func TestIsStandardAuthProxy(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  bool
	}{
		{name: "http auth", input: "http://user:pass@example.com:80", want: true},
		{name: "socks5 auth", input: "socks5://user:pass@example.com:1080", want: true},
		{name: "socks alias auth", input: "socks://user:pass@example.com:1080", want: true},
		{name: "socks5h alias auth", input: "socks5h://user:pass@example.com:1080", want: true},
		{name: "https auth", input: "https://user:pass@example.com:443", want: true},
		{name: "no auth", input: "http://example.com:80", want: false},
		{name: "missing port", input: "http://user:pass@example.com", want: false},
		{name: "non standard", input: "vmess://example", want: false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := IsStandardAuthProxy(tt.input); got != tt.want {
				t.Fatalf("IsStandardAuthProxy() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestBuildSingBoxOutboundStandardHTTPAuthProxy(t *testing.T) {
	out, err := BuildSingBoxOutbound("http://user:pa%3Ass@example.com:80")
	if err != nil {
		t.Fatalf("BuildSingBoxOutbound returned error: %v", err)
	}
	if out["type"] != "http" {
		t.Fatalf("type = %v, want http", out["type"])
	}
	if out["tag"] != "proxy-out" {
		t.Fatalf("tag = %v, want proxy-out", out["tag"])
	}
	if out["server"] != "example.com" {
		t.Fatalf("server = %v, want example.com", out["server"])
	}
	if out["server_port"] != 80 {
		t.Fatalf("server_port = %v, want 80", out["server_port"])
	}
	if out["username"] != "user" {
		t.Fatalf("username = %v, want user", out["username"])
	}
	if out["password"] != "pa:ss" {
		t.Fatalf("password = %v, want decoded password", out["password"])
	}
}

func TestBuildSingBoxOutboundStandardHTTPSAuthProxyEnablesTLS(t *testing.T) {
	out, err := BuildSingBoxOutbound("https://user:pass@example.com:443")
	if err != nil {
		t.Fatalf("BuildSingBoxOutbound returned error: %v", err)
	}
	if out["type"] != "http" {
		t.Fatalf("type = %v, want http", out["type"])
	}
	tls, ok := out["tls"].(map[string]interface{})
	if !ok {
		t.Fatalf("tls config missing")
	}
	if tls["enabled"] != true {
		t.Fatalf("tls.enabled = %v, want true", tls["enabled"])
	}
}

func TestBuildSingBoxOutboundStandardSOCKS5AuthProxy(t *testing.T) {
	out, err := BuildSingBoxOutbound("socks5://user:pass@example.com:1080")
	if err != nil {
		t.Fatalf("BuildSingBoxOutbound returned error: %v", err)
	}
	if out["type"] != "socks" {
		t.Fatalf("type = %v, want socks", out["type"])
	}
	if out["version"] != "5" {
		t.Fatalf("version = %v, want 5", out["version"])
	}
	if out["server"] != "example.com" {
		t.Fatalf("server = %v, want example.com", out["server"])
	}
	if out["server_port"] != 1080 {
		t.Fatalf("server_port = %v, want 1080", out["server_port"])
	}
	if out["username"] != "user" || out["password"] != "pass" {
		t.Fatalf("auth = %v/%v, want user/pass", out["username"], out["password"])
	}
}

func TestBuildSingBoxOutboundAcceptsSOCKSAliases(t *testing.T) {
	for _, input := range []string{
		"socks://user:pass@example.com:1080",
		"socks5h://user:pass@example.com:1080",
		"socket://user:pass@example.com:1080",
	} {
		t.Run(input, func(t *testing.T) {
			out, err := BuildSingBoxOutbound(input)
			if err != nil {
				t.Fatalf("BuildSingBoxOutbound returned error: %v", err)
			}
			if out["type"] != "socks" || out["version"] != "5" {
				t.Fatalf("outbound = %#v, want socks5 outbound", out)
			}
		})
	}
}

func TestProxyEndpointStripsStandardProxyUserInfo(t *testing.T) {
	endpoint, err := proxyEndpoint("http://user:pass@example.com:8080")
	if err != nil {
		t.Fatalf("proxyEndpoint returned error: %v", err)
	}
	if endpoint != "example.com:8080" {
		t.Fatalf("endpoint = %q, want example.com:8080", endpoint)
	}
}

func TestParseSSHTunnelDirectiveRemovesPersonalPilotParams(t *testing.T) {
	directive, ok, err := ParseSSHTunnelDirective("http://user:pass@example.com:8080?pp_via_ssh=panda&pp_ssh_port=2222&keep=1")
	if err != nil {
		t.Fatalf("ParseSSHTunnelDirective returned error: %v", err)
	}
	if !ok {
		t.Fatalf("ParseSSHTunnelDirective ok = false, want true")
	}
	if directive.Target != "panda" {
		t.Fatalf("Target = %q, want panda", directive.Target)
	}
	if directive.SSHPort != 2222 {
		t.Fatalf("SSHPort = %d, want 2222", directive.SSHPort)
	}
	if directive.RemoteHost != "example.com" || directive.RemotePort != 8080 {
		t.Fatalf("remote = %s:%d, want example.com:8080", directive.RemoteHost, directive.RemotePort)
	}
	if directive.CleanProxyURL != "http://user:pass@example.com:8080?keep=1" {
		t.Fatalf("CleanProxyURL = %q", directive.CleanProxyURL)
	}
}

func TestRewriteProxyURLToLocalForwardPreservesAuth(t *testing.T) {
	rewritten, err := rewriteProxyURLToLocalForward("socks5://user:pa%3Ass@example.com:1080", 33000)
	if err != nil {
		t.Fatalf("rewriteProxyURLToLocalForward returned error: %v", err)
	}
	if rewritten != "socks5://user:pa%3Ass@127.0.0.1:33000" {
		t.Fatalf("rewritten = %q", rewritten)
	}
}

func TestSingBoxCanHandleSSHTunnelDirective(t *testing.T) {
	manager := &SingBoxManager{}
	if !manager.CanHandle("http://example.com:8080?pp_via_ssh=panda") {
		t.Fatalf("CanHandle returned false for SSH tunnel directive")
	}
}

func TestApplyHTTPSProxyServerName(t *testing.T) {
	out, err := BuildSingBoxOutbound("https://user:pass@127.0.0.1:33000")
	if err != nil {
		t.Fatalf("BuildSingBoxOutbound returned error: %v", err)
	}
	applyHTTPSProxyServerName(out, "proxy.example.com")
	tls, ok := out["tls"].(map[string]interface{})
	if !ok {
		t.Fatalf("tls config missing")
	}
	if tls["server_name"] != "proxy.example.com" {
		t.Fatalf("tls.server_name = %v, want proxy.example.com", tls["server_name"])
	}
}

func TestParseSSHTunnelDirectiveRejectsUnsafeTarget(t *testing.T) {
	_, ok, err := ParseSSHTunnelDirective("http://example.com:8080?pp_via_ssh=-oProxyCommand=bad")
	if !ok {
		t.Fatalf("ok = false, want true for present directive")
	}
	if err == nil {
		t.Fatalf("expected unsafe SSH target to be rejected")
	}
}
