package behavior

import (
	"strings"
	"testing"
)

func TestCompileEnvironmentInjectionScriptBuildsSingleBatchScript(t *testing.T) {
	offset := -480
	plan, err := CompileEnvironmentInjectionScript(EnvironmentInjectionProfile{
		Seed:                "profile-1",
		Languages:           []string{"en-US", "en"},
		Platform:            "Win32",
		Vendor:              "Google Inc.",
		UserAgent:           "Mozilla/5.0 test",
		HardwareConcurrency: 8,
		DeviceMemory:        8,
		Timezone:            "America/Los_Angeles",
		TimezoneOffset:      &offset,
		WebGLVendor:         "Intel Inc.",
		WebGLRenderer:       "Intel Iris",
		WebGLExtensions:     []string{"WEBGL_debug_renderer_info"},
		AudioNoise:          0.001,
		FontAllowlist:       []string{"Segoe UI", "Arial"},
		MediaPermission:     "allow",
		WebRTCPolicy:        WebRTCPolicy{Mode: "filtered", AllowHosts: []string{"203.0.113.1"}},
		Plugins: []PluginInjection{{
			Name: "Chrome PDF Plugin", Filename: "internal-pdf-viewer", MimeType: "application/pdf",
		}},
		MediaDevices: []MediaDeviceSpec{{Kind: "audioinput", Label: "Microphone", ID: "mic-1", GroupID: "grp"}},
	})
	if err != nil {
		t.Fatalf("compile environment injection: %v", err)
	}
	needles := []string{
		"Navigator.prototype, 'webdriver'",
		"Navigator.prototype, 'languages'",
		"HTMLCanvasElement.prototype.toDataURL",
		"CanvasRenderingContext2D.prototype.getImageData",
		"Intl.DateTimeFormat.prototype.resolvedOptions",
		"Date.prototype.getTimezoneOffset",
		"getSupportedExtensions",
		"AnalyserNode.prototype.getFloatFrequencyData",
		"AudioBuffer.prototype.copyFromChannel",
		"Document.prototype, 'fonts'",
		"enumerateDevices",
		"getUserMedia",
		"RTCPeerConnection",
		"a=candidate:",
	}
	for _, needle := range needles {
		if !strings.Contains(plan.Script, needle) {
			t.Fatalf("script missing %q", needle)
		}
	}
	if plan.HeaderOverrides["Accept-Language"] != "en-US,en;q=0.9" {
		t.Fatalf("Accept-Language = %q", plan.HeaderOverrides["Accept-Language"])
	}
	if len(plan.AppliedFamilies) < 8 {
		t.Fatalf("applied families = %v", plan.AppliedFamilies)
	}
	if len(plan.Warnings) != 0 {
		t.Fatalf("warnings = %v", plan.Warnings)
	}
}

func TestCompileEnvironmentInjectionScriptWarnsOnIncompleteIdentity(t *testing.T) {
	plan, err := CompileEnvironmentInjectionScript(EnvironmentInjectionProfile{AcceptLanguage: "ja-JP,ja;q=0.9"})
	if err != nil {
		t.Fatalf("compile environment injection: %v", err)
	}
	if plan.HeaderOverrides["Accept-Language"] != "ja-JP,ja;q=0.9" {
		t.Fatalf("Accept-Language header not preserved: %q", plan.HeaderOverrides["Accept-Language"])
	}
	want := map[string]bool{
		"timezone_not_configured":                     false,
		"user_agent_platform_vendor_chain_incomplete": false,
		"webgl_identity_incomplete":                   false,
	}
	for _, warning := range plan.Warnings {
		if _, ok := want[warning]; ok {
			want[warning] = true
		}
	}
	for warning, seen := range want {
		if !seen {
			t.Fatalf("missing warning %s in %v", warning, plan.Warnings)
		}
	}
}

func TestStableProfileSeedDeterministic(t *testing.T) {
	if stableProfileSeed("profile") != stableProfileSeed("profile") {
		t.Fatal("stableProfileSeed should be deterministic")
	}
	if stableProfileSeed("profile") == stableProfileSeed("other") {
		t.Fatal("stableProfileSeed should differ for distinct input")
	}
}
