package transport

import "testing"

func TestBrowserLikeUserAgent(t *testing.T) {
	if got := BrowserLikeUserAgent(""); got != ProductUserAgent {
		t.Fatalf("empty profile UA = %q, want %q", got, ProductUserAgent)
	}
	if got := BrowserLikeUserAgent("  "); got != ProductUserAgent {
		t.Fatalf("whitespace profile UA = %q, want %q", got, ProductUserAgent)
	}
	profile := "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"
	if got := BrowserLikeUserAgent(profile); got != profile {
		t.Fatalf("profile UA = %q, want %q", got, profile)
	}
	if got := BrowserLikeUserAgent("  " + profile + "  "); got != profile {
		t.Fatalf("trimmed profile UA = %q, want %q", got, profile)
	}
}
