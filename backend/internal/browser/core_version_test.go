package browser

import (
	"strings"
	"testing"
)

func TestResolveChromiumVersionPrefersCoreIdentityOverStaleArgs(t *testing.T) {
	core := &Core{
		CoreId:   "core-fingerprint-chromium-139-0-7258-154",
		CoreName: "Fingerprint Chromium 139",
		CorePath: `chrome/fingerprint-chromium-139.0.7258.154`,
	}
	profile := &Profile{
		CoreId:          core.CoreId,
		FingerprintArgs: []string{"--fingerprint-brand-version=131.0.0.0"},
		LaunchArgs:      []string{"--user-agent=Mozilla/5.0 Chrome/131.0.0.0 Safari/537.36"},
	}
	got := ResolveChromiumVersion(core, `D:\SelfMadeTool\personal-pilot\chrome\fingerprint-chromium-139.0.7258.154\chrome.exe`, profile)
	if got.Full != "139.0.7258.154" || got.Major != 139 {
		t.Fatalf("version=%+v", got)
	}
	if got.Source != "binary_path" {
		t.Fatalf("source=%q", got.Source)
	}
}

func TestResolveChromiumVersionFallsBackToRuntimeArgs(t *testing.T) {
	profile := &Profile{
		FingerprintArgs: []string{"--fingerprint-brand-version=126.0.6478.271"},
	}
	got := ResolveChromiumVersion(nil, "", profile)
	if got.Full != "126.0.6478.271" || got.Major != 126 || got.Source != "runtime_args" {
		t.Fatalf("version=%+v", got)
	}
}

func TestChromiumUserAgentUsesResolvedMajor(t *testing.T) {
	got := ChromiumUserAgent(CoreVersionInfo{Full: "139.0.7258.154", Major: 139})
	if want := "Chrome/139.0.7258.154"; !strings.Contains(got, want) {
		t.Fatalf("ua=%q missing %q", got, want)
	}
}
