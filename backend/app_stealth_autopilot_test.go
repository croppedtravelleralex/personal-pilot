package backend

import (
	"os"
	"testing"
)

func TestAutopilotEnvRefreshToken(t *testing.T) {
	profileID := "abc-def-123"
	key := "PERSONAL_PILOT_MS_REFRESH_TOKEN_ABC_DEF_123"
	t.Setenv(key, "token-for-profile")
	if got := autopilotEnvRefreshToken(profileID); got != "token-for-profile" {
		t.Fatalf("profile token: got %q", got)
	}
	t.Setenv(key, "")
	t.Setenv("PERSONAL_PILOT_MS_REFRESH_TOKEN", "global-token")
	if got := autopilotEnvRefreshToken("other"); got != "global-token" {
		t.Fatalf("global token: got %q", got)
	}
	_ = os.Getenv
}

func TestProfileWantsStealthAutopilot(t *testing.T) {
	if profileWantsStealthAutopilot(&BrowserProfile{Tags: []string{"auto-99"}}) != true {
		t.Fatal("expected auto-99 tag")
	}
	if profileWantsStealthAutopilot(&BrowserProfile{Tags: []string{"work"}}) {
		t.Fatal("expected false")
	}
}
