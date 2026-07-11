package browser

import (
	"strings"
	"testing"
)

func TestApplyStrictAuthPresetTriggeredByTag(t *testing.T) {
	profile := &Profile{
		ProfileId: "p1",
		Tags:      []string{"outlook-auth"},
	}
	fp, launch := MaterializeRuntimeArgs(profile, nil, nil)
	joined := append(append([]string{}, fp...), launch...)
	text := stringsJoin(joined)
	if !contains(text, "--disable-blink-features=AutomationControlled") {
		t.Fatalf("missing anti-automation flag: %v", joined)
	}
	if !contains(text, "--lang=en-US") {
		t.Fatalf("expected en-US locale for strict auth preset")
	}
}

func stringsJoin(items []string) string {
	out := ""
	for _, item := range items {
		out += item + " "
	}
	return out
}

func contains(haystack, needle string) bool {
	return len(needle) == 0 || (len(haystack) >= len(needle) && indexOf(haystack, needle) >= 0)
}

func indexOf(s, sub string) int {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return i
		}
	}
	return -1
}

func TestApplyStrictAuthPresetPreservesResolvedCoreVersion(t *testing.T) {
	profile := &Profile{
		ProfileId: "auth-139",
		CoreId:    "core-fingerprint-chromium-139-0-7258-154",
		Tags:      []string{"outlook-auth"},
	}
	fingerprintArgs, launchArgs := MaterializeRuntimeArgs(profile, nil, nil)
	joined := strings.Join(append(fingerprintArgs, launchArgs...), "\n")
	if strings.Contains(joined, "Chrome/131") || strings.Contains(joined, "brand-version=131") {
		t.Fatalf("strict auth reintroduced stale browser version:\n%s", joined)
	}
	if !strings.Contains(joined, "Chrome/139.0.7258.154") || !strings.Contains(joined, "brand-version=139.0.7258.154") {
		t.Fatalf("strict auth lost resolved browser version:\n%s", joined)
	}
}
