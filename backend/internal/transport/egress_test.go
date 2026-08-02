package transport

import "testing"

func TestParseUAMajor(t *testing.T) {
	if got := ParseUAMajor("Mozilla/5.0 Chrome/139.0.0.0 Safari/537.36"); got != 139 {
		t.Fatalf("major=%d", got)
	}
	if got := ParseUAMajor(""); got != 0 {
		t.Fatalf("empty major=%d", got)
	}
}

func TestChromeHelloPreset(t *testing.T) {
	if got := ChromeHelloPreset(139); got != "Chrome_133" {
		t.Fatalf("got %q", got)
	}
	if got := ChromeHelloPreset(100); got != "Chrome_Auto" {
		t.Fatalf("got %q", got)
	}
}

func TestChromeMajorTLSBaseline(t *testing.T) {
	cfg := ChromeMajorTLSBaseline(139)
	if cfg.TLS.JA3 != "Chrome_133" {
		t.Fatalf("JA3=%q", cfg.TLS.JA3)
	}
	if len(cfg.TLS.ALPN) == 0 || cfg.TLS.ALPN[0] != "h2" {
		t.Fatalf("ALPN=%v", cfg.TLS.ALPN)
	}
	if cfg.RouteTag != "chrome-139" {
		t.Fatalf("RouteTag=%q", cfg.RouteTag)
	}
}

func TestTLSUACoherent(t *testing.T) {
	ua := "Mozilla/5.0 Chrome/139.0.0.0 Safari/537.36"
	if !TLSUACoherent(ua, "Chrome_133") {
		t.Fatal("expected coherent")
	}
	if TLSUACoherent(ua, "Chrome_120") {
		t.Fatal("expected incoherent for mismatched template")
	}
}
