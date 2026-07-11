package browser

import "testing"

func TestAssessFingerprintConsistency_Coherent(t *testing.T) {
	input := &FingerprintConsistencyInput{
		TargetRegion:        "US",
		ProxyRegion:         "US",
		Timezone:            "America/New_York",
		Locale:              "en-US",
		AcceptLanguage:      "en-US,en;q=0.9",
		ScreenWidth:         1920,
		ScreenHeight:        1080,
		AvailWidth:          1920,
		AvailHeight:         1040,
		GPUVendor:           "Google",
		WebGLVendor:         "Google Inc.",
		WebGLRenderer:       "ANGLE",
		SupportsTouch:       false,
		HardwareConcurrency: 8,
		Platform:            "Win32",
	}
	result := AssessFingerprintConsistency(input)
	if result.CoherenceScore < 80 {
		t.Fatalf("CoherenceScore = %d, want >= 80 for coherent input. Risks: %v", result.CoherenceScore, result.RiskReasons)
	}
	if result.Status != "coherent" {
		t.Fatalf("Status = %q, want 'coherent'. Risks: %v", result.Status, result.RiskReasons)
	}
}

func TestAssessFingerprintConsistency_RegionMismatch(t *testing.T) {
	input := &FingerprintConsistencyInput{
		TargetRegion: "US",
		ProxyRegion:  "JP",
	}
	result := AssessFingerprintConsistency(input)
	if result.CoherenceScore >= 80 {
		t.Fatalf("CoherenceScore = %d, want < 80 for region mismatch", result.CoherenceScore)
	}
	if result.HardFailures == 0 {
		t.Fatal("Should have at least one hard failure for region mismatch")
	}
}

func TestAssessFingerprintConsistency_TimezoneMismatch(t *testing.T) {
	input := &FingerprintConsistencyInput{
		TargetRegion: "JP",
		Timezone:     "America/New_York",
	}
	result := AssessFingerprintConsistency(input)
	if result.Status == "coherent" {
		t.Fatal("Should be suspicious or inconsistent for timezone mismatch")
	}
	// Find the timezone check
	found := false
	for _, ch := range result.CheckItems {
		if ch.Dimension == "timezone_vs_region" && !ch.Passed {
			found = true
		}
	}
	if !found {
		t.Fatal("timezone_vs_region check should have failed")
	}
}

func TestAssessFingerprintConsistency_LocaleMismatch(t *testing.T) {
	input := &FingerprintConsistencyInput{
		Locale:         "ja-JP",
		AcceptLanguage: "en-US,en;q=0.9",
	}
	result := AssessFingerprintConsistency(input)
	found := false
	for _, ch := range result.CheckItems {
		if ch.Dimension == "locale_vs_accept_language" && !ch.Passed {
			found = true
		}
	}
	if !found {
		t.Fatal("locale_vs_accept_language check should have failed")
	}
}

func TestAssessFingerprintConsistency_GPUWebGLMismatch(t *testing.T) {
	input := &FingerprintConsistencyInput{
		GPUVendor:   "NVIDIA",
		WebGLVendor: "Intel Inc.",
	}
	result := AssessFingerprintConsistency(input)
	found := false
	for _, ch := range result.CheckItems {
		if ch.Dimension == "gpu_vs_webgl" && !ch.Passed {
			found = true
		}
	}
	if !found {
		t.Fatal("gpu_vs_webgl check should have failed")
	}
}

func TestAssessFingerprintConsistency_TouchMismatch(t *testing.T) {
	input := &FingerprintConsistencyInput{
		SupportsTouch:  true,
		MaxTouchPoints: 0,
	}
	result := AssessFingerprintConsistency(input)
	found := false
	for _, ch := range result.CheckItems {
		if ch.Dimension == "touch_support" && !ch.Passed {
			found = true
		}
	}
	if !found {
		t.Fatal("touch_support check should have failed (touch enabled but 0 max points)")
	}
}

func TestAssessFingerprintConsistency_MobileHighCores(t *testing.T) {
	input := &FingerprintConsistencyInput{
		Platform:            "Android",
		HardwareConcurrency: 16,
	}
	result := AssessFingerprintConsistency(input)
	found := false
	for _, ch := range result.CheckItems {
		if ch.Dimension == "hardware_tier_vs_power_plan" && !ch.Passed {
			found = true
		}
	}
	if !found {
		t.Fatal("mobile with 16 cores should be flagged")
	}
}

func TestAssessFingerprintConsistency_AutomationBot(t *testing.T) {
	input := &FingerprintConsistencyInput{
		AutomationPolicyEnabled: true,
		BehaviorCadence:         "bot",
	}
	result := AssessFingerprintConsistency(input)
	found := false
	for _, ch := range result.CheckItems {
		if ch.Dimension == "automation_vs_behavior" && !ch.Passed {
			found = true
		}
	}
	if !found {
		t.Fatal("automation + bot cadence should be flagged")
	}
}

func TestAssessFingerprintConsistency_ScreenViewportMismatch(t *testing.T) {
	input := &FingerprintConsistencyInput{
		ScreenWidth:  1024,
		ScreenHeight: 768,
		AvailWidth:   1920,
		AvailHeight:  1040,
	}
	result := AssessFingerprintConsistency(input)
	found := false
	for _, ch := range result.CheckItems {
		if ch.Dimension == "screen_vs_viewport" && !ch.Passed {
			found = true
		}
	}
	if !found {
		t.Fatal("viewport larger than screen should be flagged")
	}
}

func TestAssessFingerprintConsistency_StatusInconsistent(t *testing.T) {
	input := &FingerprintConsistencyInput{
		TargetRegion:   "US",
		ProxyRegion:    "JP",
		Locale:         "ja-JP",
		AcceptLanguage: "en-US",
		GPUVendor:      "NVIDIA",
		WebGLVendor:    "Intel",
		SupportsTouch:  true,
		MaxTouchPoints: 0,
	}
	result := AssessFingerprintConsistency(input)
	if result.Status != "inconsistent" {
		t.Fatalf("Status = %q, want 'inconsistent' with multiple hard mismatches. Score=%d", result.Status, result.CoherenceScore)
	}
}

func TestEnforceFingerprintConsistency_WarnNeverBlocks(t *testing.T) {
	assessment := FingerprintConsistencyAssessment{
		Status:       "inconsistent",
		HardFailures: 3,
		RiskReasons:  []string{"[HARD] mismatch"},
	}
	if err := EnforceFingerprintConsistency(assessment, CoherenceWarn); err != nil {
		t.Fatalf("warn mode should not block: %v", err)
	}
}

func TestEnforceFingerprintConsistency_BlockOnInconsistent(t *testing.T) {
	assessment := FingerprintConsistencyAssessment{
		Status:       "inconsistent",
		HardFailures: 2,
		RiskReasons:  []string{"[HARD] locale mismatch"},
	}
	if err := EnforceFingerprintConsistency(assessment, CoherenceBlock); err == nil {
		t.Fatal("block mode should reject inconsistent assessment")
	}
}

func TestEnforceFingerprintConsistency_BlockOnHardFailures(t *testing.T) {
	assessment := FingerprintConsistencyAssessment{
		Status:       "suspicious",
		HardFailures: 1,
		RiskReasons:  []string{"[HARD] proxy region mismatch"},
	}
	if err := EnforceFingerprintConsistency(assessment, CoherenceBlock); err == nil {
		t.Fatal("block mode should reject hard failures even when status is suspicious")
	}
}

func TestEnforceFingerprintConsistency_BlockAllowsCoherent(t *testing.T) {
	assessment := FingerprintConsistencyAssessment{Status: "coherent", HardFailures: 0}
	if err := EnforceFingerprintConsistency(assessment, CoherenceBlock); err != nil {
		t.Fatalf("coherent assessment should pass block mode: %v", err)
	}
}

func TestBuildConsistencyInputFromArgs(t *testing.T) {
	fp := []string{
		"--timezone=America/New_York",
		"--lang=en-US",
		"--accept-lang=en-US,en;q=0.9",
		"--fingerprint-platform=windows",
		"--fingerprint-webgl-vendor=Intel Inc.",
		"--fingerprint-webgl-renderer=Intel Iris",
		"--fingerprint-hardware-concurrency=8",
		"--window-size=1920,1080",
	}
	launch := []string{"--disable-sync"}
	input := BuildConsistencyInputFromArgs(fp, launch, "US")
	if input.Timezone != "America/New_York" || input.Locale != "en-US" {
		t.Fatalf("locale/tz not parsed: %+v", input)
	}
	if input.AcceptLanguage != "en-US,en;q=0.9" || input.Platform != "Win32" {
		t.Fatalf("accept/platform not parsed: %+v", input)
	}
	if input.GPUVendor != "Intel Inc." || input.HardwareConcurrency != 8 {
		t.Fatalf("gpu/concurrency not parsed: %+v", input)
	}
	if input.ScreenWidth != 1920 || input.ScreenHeight != 1080 {
		t.Fatalf("window size not parsed: %+v", input)
	}
	if input.ProxyRegion != "US" {
		t.Fatalf("proxy region=%q want US", input.ProxyRegion)
	}
}

func TestParseCoherenceEnforceMode(t *testing.T) {
	if ParseCoherenceEnforceMode("block") != CoherenceBlock {
		t.Fatal("expected block mode")
	}
	if ParseCoherenceEnforceMode("BLOCK") != CoherenceBlock {
		t.Fatal("expected case-insensitive block")
	}
	if ParseCoherenceEnforceMode("") != CoherenceWarn {
		t.Fatal("expected default warn")
	}
}
