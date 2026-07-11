package browser

import "testing"

func TestNormalizeProxyServerForBrowser(t *testing.T) {
	tests := []struct {
		in   string
		want string
	}{
		{"socks5://127.0.0.1:1080", "socks5://127.0.0.1:1080"},
		{"socks5://localhost:2080", "socks5://127.0.0.1:2080"},
		{"socks5h://127.0.0.1:1080", "socks5://127.0.0.1:1080"},
		{"direct://", "direct://"},
		{"", ""},
	}
	for _, tc := range tests {
		if got := NormalizeProxyServerForBrowser(tc.in); got != tc.want {
			t.Fatalf("NormalizeProxyServerForBrowser(%q) = %q, want %q", tc.in, got, tc.want)
		}
	}
}

func TestAppendProxyHardeningArgs(t *testing.T) {
	args := AppendProxyHardeningArgs(nil, "socks5h://127.0.0.1:1080")
	if len(args) != 2 {
		t.Fatalf("expected 2 hardening args, got %d", len(args))
	}
	dup := AppendProxyHardeningArgs(args, "socks5h://127.0.0.1:1080")
	if len(dup) != 2 {
		t.Fatalf("expected no duplicate hardening args, got %d", len(dup))
	}
	if got := AppendProxyHardeningArgs(nil, "direct://"); len(got) != 0 {
		t.Fatalf("direct:// should not add hardening args")
	}
}
