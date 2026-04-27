package browser

import "strings"

// ConsistencyCheckSeverity indicates how severe a consistency mismatch is.
type ConsistencyCheckSeverity int

const (
	CheckHard ConsistencyCheckSeverity = 0 // -30 points
	CheckSoft ConsistencyCheckSeverity = 1 // -20 points
	CheckInfo ConsistencyCheckSeverity = 2 // -10 points
)

// ConsistencyCheckItem describes a single consistency check result.
type ConsistencyCheckItem struct {
	Dimension string
	Severity  ConsistencyCheckSeverity
	Passed    bool
	Detail    string
}

// FingerprintConsistencyAssessment is the result of cross-checking fingerprint parameters.
type FingerprintConsistencyAssessment struct {
	CoherenceScore  int                    // 0-100, higher = more consistent
	Status          string                 // "coherent", "suspicious", "inconsistent"
	HardFailures    int
	SoftFailures    int
	RiskReasons     []string
	CheckItems      []ConsistencyCheckItem
}

// FingerprintConsistencyInput provides all the data needed for a consistency assessment.
type FingerprintConsistencyInput struct {
	// Region / locale
	TargetRegion     string // e.g. "US", "JP", "DE"
	ProxyRegion      string // detected proxy exit region
	Timezone         string // e.g. "America/New_York"
	Locale           string // e.g. "en-US"
	AcceptLanguage   string // e.g. "en-US,en;q=0.9"
	// Display
	ScreenWidth  int
	ScreenHeight int
	AvailWidth   int
	AvailHeight  int
	// GPU / WebGL
	GPUVRAM       int
	GPUVendor     string // expected GPU vendor from fingerprint args
	WebGLVendor   string // actual GPU vendor detected in browser
	WebGLRenderer string // actual GPU renderer detected in browser
	// Touch
	SupportsTouch   bool
	MaxTouchPoints  int
	// Hardware
	HardwareConcurrency int
	DeviceMemory        int
	Platform            string // "Win32", "MacIntel", "Linux x86_64"
	// Session
	StickySessionTTLMinutes int
	AutoRotateProxy         bool
	// Automation
	AutomationPolicyEnabled bool
	BehaviorCadence         string // "human", "mixed", "bot"
}

// AssessFingerprintConsistency runs all 10 dimension checks and returns a scored assessment.
func AssessFingerprintConsistency(input *FingerprintConsistencyInput) FingerprintConsistencyAssessment {
	var checks []ConsistencyCheckItem

	checks = append(checks, checkTargetVsProxyRegion(input))
	checks = append(checks, checkProxyVsExitRegion(input))
	checks = append(checks, checkTimezoneVsRegion(input))
	checks = append(checks, checkLocaleVsAcceptLanguage(input))
	checks = append(checks, checkScreenVsViewport(input))
	checks = append(checks, checkGPUvsWebGL(input))
	checks = append(checks, checkTouchSupport(input))
	checks = append(checks, checkStickySessionVsRotation(input))
	checks = append(checks, checkHardwareTierVsPowerPlan(input))
	checks = append(checks, checkAutomationVsBehavior(input))

	score := 100
	var hardFails, softFails int
	var risks []string

	for _, ch := range checks {
		if ch.Passed {
			continue
		}
		switch ch.Severity {
		case CheckHard:
			score -= 30
			hardFails++
			risks = append(risks, "[HARD] "+ch.Detail)
		case CheckSoft:
			score -= 20
			softFails++
			risks = append(risks, "[SOFT] "+ch.Detail)
		case CheckInfo:
			score -= 10
			risks = append(risks, "[INFO] "+ch.Detail)
		}
	}

	if score < 0 {
		score = 0
	}

	var status string
	switch {
	case score >= 80:
		status = "coherent"
	case score >= 50:
		status = "suspicious"
	default:
		status = "inconsistent"
	}

	return FingerprintConsistencyAssessment{
		CoherenceScore: score,
		Status:         status,
		HardFailures:   hardFails,
		SoftFailures:   softFails,
		RiskReasons:    risks,
		CheckItems:     checks,
	}
}

// checkTargetVsProxyRegion verifies the target region matches the proxy exit region.
func checkTargetVsProxyRegion(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	if input.TargetRegion == "" || input.ProxyRegion == "" {
		return ConsistencyCheckItem{Dimension: "target_vs_proxy_region", Severity: CheckHard, Passed: true, Detail: "skipped (missing data)"}
	}
	passed := strings.EqualFold(input.TargetRegion, input.ProxyRegion)
	detail := "target=" + input.TargetRegion + " proxy=" + input.ProxyRegion
	if !passed {
		detail = "mismatch: " + detail
	}
	return ConsistencyCheckItem{Dimension: "target_vs_proxy_region", Severity: CheckHard, Passed: passed, Detail: detail}
}

// checkProxyVsExitRegion is a placeholder for when actual exit-IP region data is available.
func checkProxyVsExitRegion(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	// Requires external IP geolocation data — always passes unless explicitly inconsistent
	return ConsistencyCheckItem{Dimension: "proxy_vs_exit_region", Severity: CheckHard, Passed: true, Detail: "deferred (requires IP geolocation)"}
}

// checkTimezoneVsRegion verifies the timezone is consistent with the target region.
func checkTimezoneVsRegion(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	if input.Timezone == "" || input.TargetRegion == "" {
		return ConsistencyCheckItem{Dimension: "timezone_vs_region", Severity: CheckSoft, Passed: true, Detail: "skipped (missing data)"}
	}

	// Map regions to expected timezone patterns
	regionTZ := map[string]string{
		"US": "America/", "GB": "Europe/London", "DE": "Europe/Berlin",
		"FR": "Europe/Paris", "JP": "Asia/Tokyo", "KR": "Asia/Seoul",
		"CN": "Asia/Shanghai", "IN": "Asia/Kolkata", "BR": "America/Sao_Paulo",
		"AU": "Australia/", "CA": "America/",
		"RU": "Europe/Moscow", "SG": "Asia/Singapore", "HK": "Asia/Hong_Kong",
	}

	for region, tzPrefix := range regionTZ {
		if strings.EqualFold(input.TargetRegion, region) {
			passed := strings.HasPrefix(input.Timezone, tzPrefix)
			detail := "tz=" + input.Timezone + " region=" + input.TargetRegion
			if !passed {
				detail = "mismatch: " + detail + " (expected prefix: " + tzPrefix + ")"
			}
			return ConsistencyCheckItem{Dimension: "timezone_vs_region", Severity: CheckSoft, Passed: passed, Detail: detail}
		}
	}
	// Unknown region — we can't validate
	return ConsistencyCheckItem{Dimension: "timezone_vs_region", Severity: CheckSoft, Passed: true, Detail: "unknown region: " + input.TargetRegion}
}

// checkLocaleVsAcceptLanguage verifies locale prefix matches Accept-Language prefix.
func checkLocaleVsAcceptLanguage(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	if input.Locale == "" || input.AcceptLanguage == "" {
		return ConsistencyCheckItem{Dimension: "locale_vs_accept_language", Severity: CheckHard, Passed: true, Detail: "skipped (missing data)"}
	}

	// Extract primary language tag (e.g., "en" from "en-US")
	localePrefix := strings.Split(input.Locale, "-")[0]
	alPrefix := strings.Split(input.AcceptLanguage, "-")[0]
	alPrefix = strings.Split(alPrefix, ",")[0] // trim q-values

	passed := strings.EqualFold(localePrefix, alPrefix)
	detail := "locale=" + input.Locale + " accept-language=" + input.AcceptLanguage
	if !passed {
		detail = "mismatch: " + detail
	}
	return ConsistencyCheckItem{Dimension: "locale_vs_accept_language", Severity: CheckHard, Passed: passed, Detail: detail}
}

// checkScreenVsViewport verifies screen dimensions are >= viewport dimensions.
func checkScreenVsViewport(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	if input.ScreenWidth <= 0 || input.ScreenHeight <= 0 {
		return ConsistencyCheckItem{Dimension: "screen_vs_viewport", Severity: CheckHard, Passed: true, Detail: "skipped (no screen dimensions)"}
	}
	passed := input.ScreenWidth >= input.AvailWidth && input.ScreenHeight >= input.AvailHeight
	detail := "screen=" + itoa(input.ScreenWidth) + "x" + itoa(input.ScreenHeight) +
		" avail=" + itoa(input.AvailWidth) + "x" + itoa(input.AvailHeight)
	if !passed {
		detail = "viewport larger than screen: " + detail
	}
	return ConsistencyCheckItem{Dimension: "screen_vs_viewport", Severity: CheckHard, Passed: passed, Detail: detail}
}

// checkGPUvsWebGL checks that the expected GPU vendor matches the actual WebGL vendor.
func checkGPUvsWebGL(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	if input.GPUVendor == "" && input.WebGLVendor == "" {
		return ConsistencyCheckItem{Dimension: "gpu_vs_webgl", Severity: CheckHard, Passed: true, Detail: "skipped (no GPU data)"}
	}
	if input.GPUVendor == "" || input.WebGLVendor == "" {
		return ConsistencyCheckItem{Dimension: "gpu_vs_webgl", Severity: CheckHard, Passed: true, Detail: "skipped (missing one side)"}
	}

	gpuLower := strings.ToLower(input.GPUVendor)
	webglLower := strings.ToLower(input.WebGLVendor)
	passed := strings.Contains(webglLower, gpuLower) || strings.Contains(gpuLower, webglLower)
	detail := "gpu=" + input.GPUVendor + " webgl=" + input.WebGLVendor
	if !passed {
		detail = "mismatch: " + detail
	}
	return ConsistencyCheckItem{Dimension: "gpu_vs_webgl", Severity: CheckHard, Passed: passed, Detail: detail}
}

// checkTouchSupport verifies touch configuration is self-consistent.
func checkTouchSupport(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	if !input.SupportsTouch {
		return ConsistencyCheckItem{Dimension: "touch_support", Severity: CheckHard, Passed: true, Detail: "no touch support configured"}
	}
	passed := input.MaxTouchPoints >= 1
	detail := "maxTouchPoints=" + itoa(input.MaxTouchPoints)
	if !passed {
		detail = "touch enabled but maxTouchPoints=0"
	}
	return ConsistencyCheckItem{Dimension: "touch_support", Severity: CheckHard, Passed: passed, Detail: detail}
}

// checkStickySessionVsRotation verifies session TTL is compatible with auto-rotation.
func checkStickySessionVsRotation(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	if !input.AutoRotateProxy {
		return ConsistencyCheckItem{Dimension: "sticky_session_vs_rotation", Severity: CheckHard, Passed: true, Detail: "auto-rotation disabled"}
	}
	if input.StickySessionTTLMinutes <= 0 {
		return ConsistencyCheckItem{Dimension: "sticky_session_vs_rotation", Severity: CheckHard, Passed: false, Detail: "auto-rotation enabled but no sticky session TTL set"}
	}
	passed := input.StickySessionTTLMinutes >= 10
	detail := "ttl=" + itoa(input.StickySessionTTLMinutes) + "min"
	if !passed {
		detail = "sticky session TTL too short for auto-rotation: " + detail
	}
	return ConsistencyCheckItem{Dimension: "sticky_session_vs_rotation", Severity: CheckHard, Passed: passed, Detail: detail}
}

// checkHardwareTierVsPowerPlan cross-references CPU/memory tier against platform.
func checkHardwareTierVsPowerPlan(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	// Detect inconsistencies between hardware tier and platform
	isMobile := strings.Contains(strings.ToLower(input.Platform), "android") ||
		strings.Contains(strings.ToLower(input.Platform), "ios")

	if isMobile && input.HardwareConcurrency > 8 {
		return ConsistencyCheckItem{
			Dimension: "hardware_tier_vs_power_plan",
			Severity:  CheckSoft,
			Passed:    false,
			Detail:    "mobile platform with high hardwareConcurrency=" + itoa(input.HardwareConcurrency),
		}
	}
	if !isMobile && input.HardwareConcurrency <= 1 {
		return ConsistencyCheckItem{
			Dimension: "hardware_tier_vs_power_plan",
			Severity:  CheckSoft,
			Passed:    false,
			Detail:    "desktop platform with very low hardwareConcurrency=" + itoa(input.HardwareConcurrency),
		}
	}
	return ConsistencyCheckItem{Dimension: "hardware_tier_vs_power_plan", Severity: CheckSoft, Passed: true, Detail: "consistent"}
}

// checkAutomationVsBehavior checks that automation policy aligns with behavior cadence.
func checkAutomationVsBehavior(input *FingerprintConsistencyInput) ConsistencyCheckItem {
	if !input.AutomationPolicyEnabled {
		return ConsistencyCheckItem{Dimension: "automation_vs_behavior", Severity: CheckInfo, Passed: true, Detail: "automation policy disabled"}
	}
	if input.BehaviorCadence == "bot" {
		return ConsistencyCheckItem{Dimension: "automation_vs_behavior", Severity: CheckInfo, Passed: false, Detail: "automation enabled but behavior cadence is 'bot' — may trigger detection"}
	}
	return ConsistencyCheckItem{Dimension: "automation_vs_behavior", Severity: CheckInfo, Passed: true, Detail: "cadence=" + input.BehaviorCadence}
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	neg := n < 0
	if neg {
		n = -n
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	if neg {
		i--
		buf[i] = '-'
	}
	return string(buf[i:])
}
