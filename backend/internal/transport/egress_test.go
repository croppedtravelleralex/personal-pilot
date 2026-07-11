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

func TestEgressFromUserAgent(t *testing.T) {
	id := EgressFromUserAgent("Mozilla/5.0 Chrome/131.0.0.0", "en-US")
	if id.UAMajor != 131 || id.ClientHello != "Chrome_131" || id.AcceptLang != "en-US" {
		t.Fatalf("%+v", id)
	}
}
