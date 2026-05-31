package backend

import "testing"

func TestProfileEnvironmentInjectionClaimDeduplicatesSameLaunch(t *testing.T) {
	app := NewApp("")
	otherKey := profileEnvironmentInjectionKey(&BrowserProfile{
		ProfileId: "profile-other",
		Pid:       5678,
	}, 9333)
	key := profileEnvironmentInjectionKey(&BrowserProfile{
		ProfileId: "profile-env",
		Pid:       1234,
	}, 9222)

	if !app.claimProfileEnvironmentInjection(key) {
		t.Fatal("first injection claim should be accepted")
	}
	if app.claimProfileEnvironmentInjection(key) {
		t.Fatal("duplicate injection claim should be rejected")
	}
	app.releaseProfileEnvironmentInjection(key)
	if !app.claimProfileEnvironmentInjection(key) {
		t.Fatal("claim should be accepted after failed injection releases the key")
	}
	if !app.claimProfileEnvironmentInjection(otherKey) {
		t.Fatal("other profile claim should be accepted")
	}
	app.clearProfileEnvironmentInjections("profile-env")
	if !app.claimProfileEnvironmentInjection(key) {
		t.Fatal("claim should be accepted after profile stop clears its keys")
	}
	if app.claimProfileEnvironmentInjection(otherKey) {
		t.Fatal("clearing one profile must not clear other profile keys")
	}
}

func TestBuildEnvironmentInjectionProfileFromLaunchFlags(t *testing.T) {
	profile := &BrowserProfile{
		ProfileId:    "profile-env",
		HumanizeSeed: "seed-env",
		LaunchArgs:   []string{"--lang=en-US,en;q=0.9"},
		FingerprintArgs: []string{
			"--fingerprint-platform=windows",
			"--fingerprint-hardware-concurrency=8",
			"--fingerprint-device-memory=16",
			"--timezone=Asia/Shanghai",
			"--fingerprint-webgl-vendor=Intel Inc.",
			"--fingerprint-webgl-renderer=Intel Iris Xe",
			"--fingerprint-audio-noise=true",
			"--fingerprint-fonts=Segoe UI,Arial",
		},
	}

	injection := buildEnvironmentInjectionProfile(profile)
	if injection.Platform != "Win32" {
		t.Fatalf("platform = %q", injection.Platform)
	}
	if injection.HardwareConcurrency != 8 || injection.DeviceMemory != 16 {
		t.Fatalf("hardware/device = %d/%d", injection.HardwareConcurrency, injection.DeviceMemory)
	}
	if injection.Timezone != "Asia/Shanghai" || injection.TimezoneOffset == nil || *injection.TimezoneOffset != -480 {
		t.Fatalf("timezone/offset = %q/%v", injection.Timezone, injection.TimezoneOffset)
	}
	if injection.AcceptLanguage != "en-US,en;q=0.9" || len(injection.Languages) != 2 {
		t.Fatalf("languages = %q %v", injection.AcceptLanguage, injection.Languages)
	}
	if injection.WebGLVendor != "Intel Inc." || injection.WebGLRenderer != "Intel Iris Xe" {
		t.Fatalf("webgl = %q/%q", injection.WebGLVendor, injection.WebGLRenderer)
	}
	if injection.AudioNoise == 0 {
		t.Fatal("expected audio noise to be enabled")
	}
	if len(injection.FontAllowlist) != 2 {
		t.Fatalf("font allowlist = %v", injection.FontAllowlist)
	}
	if len(injection.MediaDevices) == 0 || len(injection.Plugins) == 0 || len(injection.WebGLExtensions) == 0 {
		t.Fatalf("expected default media/plugin/webgl support, got devices=%d plugins=%d extensions=%d", len(injection.MediaDevices), len(injection.Plugins), len(injection.WebGLExtensions))
	}
}
