package trust

import (
	"testing"
	"time"
)

func TestHasMicrosoftSessionCookies(t *testing.T) {
	cookies := []CookieEntry{
		{Name: "ESTSAUTHPERSISTENT", Value: "abc", Domain: ".login.live.com"},
	}
	if !HasMicrosoftSessionCookies(cookies) {
		t.Fatal("expected session cookies valid")
	}
	if HasMicrosoftSessionCookies([]CookieEntry{{Name: "foo", Value: "bar", Domain: ".example.com"}}) {
		t.Fatal("expected false for unrelated cookies")
	}
}

func TestParseTokenHarvestPayload(t *testing.T) {
	raw := []byte(`{"refreshToken":"rt-123","accessToken":"at-456","loggedIn":true}`)
	h := ParseTokenHarvestPayload(raw)
	if h.RefreshToken != "rt-123" || h.AccessToken != "at-456" {
		t.Fatalf("unexpected harvest: %+v", h)
	}
}

func TestBundleHasValidTrust(t *testing.T) {
	b := Bundle{RefreshToken: "x"}
	if !b.HasValidTrust(time.Now()) {
		t.Fatal("refresh token should be valid trust")
	}
	b2 := Bundle{CookiesJSON: `[{"name":"ESTSAUTHPERSISTENT","value":"v","domain":".live.com"}]`}
	if !b2.HasValidTrust(time.Now()) {
		t.Fatal("session cookies should be valid trust")
	}
	b3 := Bundle{
		Provider:     ProviderLocalSession,
		LocalStorage: map[string]string{"pp.session.continuity": "p1"},
		CookiesJSON:  `[{"name":"pp_local_session","value":"p1","domain":"localhost"}]`,
	}
	if !b3.HasValidTrust(time.Now()) {
		t.Fatal("local session continuity should be valid trust")
	}
	if !b3.HasLocalContinuity() {
		t.Fatal("expected HasLocalContinuity")
	}
}
