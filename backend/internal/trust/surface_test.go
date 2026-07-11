package trust

import (
	"testing"
	"time"
)

func TestFromBundleMapsCookiesAndLocalStorage(t *testing.T) {
	updated := time.Date(2026, 3, 31, 12, 0, 0, 0, time.UTC)
	b := Bundle{
		RefreshToken: "rt-abc",
		CookiesJSON:  `[{"name":"sid","value":"v1","domain":".example.com"}]`,
		LocalStorage: map[string]string{"msal.token": "x"},
		UpdatedAt:    updated,
	}
	s := FromBundle(b)
	if s.Source != "bundle" {
		t.Fatalf("source=%q want bundle", s.Source)
	}
	if s.RefreshToken != "rt-abc" {
		t.Fatalf("refreshToken=%q", s.RefreshToken)
	}
	if len(s.Cookies) != 1 || s.Cookies[0]["name"] != "sid" {
		t.Fatalf("cookies=%v", s.Cookies)
	}
	if s.LocalStorage["msal.token"] != "x" {
		t.Fatalf("localStorage=%v", s.LocalStorage)
	}
	if !s.HarvestedAt.Equal(updated) {
		t.Fatalf("harvestedAt=%v want %v", s.HarvestedAt, updated)
	}
}

func TestTrustSurfaceHasUsableSession(t *testing.T) {
	if (TrustSurface{}).HasUsableSession() {
		t.Fatal("empty surface should not be usable")
	}
	withToken := TrustSurface{RefreshToken: "rt"}
	if !withToken.HasUsableSession() {
		t.Fatal("refresh token should be usable")
	}
	withCookies := TrustSurface{Cookies: []map[string]interface{}{{"name": "a"}}}
	if !withCookies.HasUsableSession() {
		t.Fatal("cookies should be usable")
	}
}
