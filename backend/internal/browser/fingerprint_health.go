package browser

import (
	"fmt"
	"math"
	"strconv"
	"strings"
	"time"
)

const (
	FingerprintHealthSourceLocalCDP = "local-cdp"

	FingerprintHealthLevelGood    = "good"
	FingerprintHealthLevelWarning = "warning"
	FingerprintHealthLevelRisk    = "risk"

	FingerprintHealthGoodMinScore    = 90
	FingerprintHealthWarningMinScore = 70
)

type FingerprintHealthProfile struct {
	ProfileID   string                   `json:"profileId,omitempty"`
	Score       int                      `json:"score"`
	Level       string                   `json:"level"`
	Checks      []FingerprintHealthCheck `json:"checks"`
	Fingerprint *FingerprintSnapshot     `json:"fingerprint"`
	CapturedAt  string                   `json:"capturedAt"`
	Source      string                   `json:"source"`
}

type FingerprintHealthCheck struct {
	ID       string `json:"id"`
	Status   string `json:"status"`
	Message  string `json:"message"`
	Expected string `json:"expected,omitempty"`
	Actual   string `json:"actual,omitempty"`
	Penalty  int    `json:"penalty,omitempty"`
}

func NewFingerprintHealthProfile(profileID string, expectedArgs []string, fingerprint *FingerprintSnapshot, capturedAt time.Time) *FingerprintHealthProfile {
	if capturedAt.IsZero() {
		capturedAt = time.Now()
	}
	checks := AssessFingerprintHealth(expectedArgs, fingerprint)
	score := fingerprintHealthScore(checks)
	return &FingerprintHealthProfile{
		ProfileID:   strings.TrimSpace(profileID),
		Score:       score,
		Level:       fingerprintHealthLevel(score),
		Checks:      checks,
		Fingerprint: fingerprint,
		CapturedAt:  capturedAt.UTC().Format(time.RFC3339Nano),
		Source:      FingerprintHealthSourceLocalCDP,
	}
}

func AssessFingerprintHealth(expectedArgs []string, actual *FingerprintSnapshot) []FingerprintHealthCheck {
	if actual == nil {
		return []FingerprintHealthCheck{{
			ID:      "fingerprint_capture",
			Status:  "fail",
			Message: "fingerprint snapshot is empty",
			Penalty: 100,
		}}
	}

	checks := make([]FingerprintHealthCheck, 0, 16)
	add := func(id, status, message string, penalty int, expected string, actualValue string) {
		checks = append(checks, FingerprintHealthCheck{
			ID:       id,
			Status:   status,
			Message:  message,
			Expected: expected,
			Actual:   actualValue,
			Penalty:  penalty,
		})
	}

	if strings.TrimSpace(actual.UserAgent) == "" {
		add("user_agent_present", "fail", "userAgent is empty", 25, "non-empty", actual.UserAgent)
	} else {
		add("user_agent_present", "pass", "userAgent captured", 0, "", "")
	}

	if strings.TrimSpace(actual.Platform) == "" {
		add("platform_present", "warning", "platform is empty", 8, "non-empty", actual.Platform)
	} else {
		add("platform_present", "pass", "platform captured", 0, "", "")
	}

	switch {
	case actual.HardwareConcurrency <= 0:
		add("hardware_concurrency_range", "fail", "hardwareConcurrency is not usable", 12, "> 0", strconv.Itoa(actual.HardwareConcurrency))
	case actual.HardwareConcurrency > 64:
		add("hardware_concurrency_range", "warning", "hardwareConcurrency is unusually high", 6, "<= 64", strconv.Itoa(actual.HardwareConcurrency))
	default:
		add("hardware_concurrency_range", "pass", "hardwareConcurrency looks usable", 0, "", "")
	}

	if strings.TrimSpace(actual.Timezone) == "" {
		add("timezone_present", "warning", "timezone is empty", 8, "non-empty", actual.Timezone)
	} else {
		add("timezone_present", "pass", "timezone captured", 0, "", "")
	}

	if strings.TrimSpace(actual.Language) == "" {
		add("language_present", "warning", "language is empty", 8, "non-empty", actual.Language)
	} else {
		add("language_present", "pass", "language captured", 0, "", "")
	}

	if actual.ScreenWidth <= 0 || actual.ScreenHeight <= 0 {
		add("screen_size_present", "fail", "screen size is not usable", 15, "positive width and height", fmt.Sprintf("%dx%d", actual.ScreenWidth, actual.ScreenHeight))
	} else if actual.AvailWidth > actual.ScreenWidth+32 || actual.AvailHeight > actual.ScreenHeight+32 {
		add("screen_size_present", "warning", "available screen is larger than screen size", 6, fmt.Sprintf("<= %dx%d", actual.ScreenWidth, actual.ScreenHeight), fmt.Sprintf("%dx%d", actual.AvailWidth, actual.AvailHeight))
	} else {
		add("screen_size_present", "pass", "screen size looks usable", 0, "", "")
	}

	canvas := strings.TrimSpace(actual.CanvasHash)
	if canvas == "" || strings.EqualFold(canvas, "error") {
		add("canvas_hash_present", "fail", "canvas hash could not be captured", 18, "non-empty", actual.CanvasHash)
	} else if isKnownVanillaCanvasHash(canvas) {
		add("canvas_hash_present", "warning", "canvas hash matches a known vanilla browser hash", 12, "non-vanilla hash", actual.CanvasHash)
	} else {
		add("canvas_hash_present", "pass", "canvas hash captured", 0, "", "")
	}

	if strings.TrimSpace(actual.WebGLVendor) == "" && strings.TrimSpace(actual.WebGLRenderer) == "" {
		add("webgl_present", "warning", "WebGL vendor and renderer are empty", 10, "vendor or renderer", "")
	} else {
		add("webgl_present", "pass", "WebGL details captured", 0, "", "")
	}

	fontHash := strings.TrimSpace(actual.FontHash)
	if fontHash == "" || strings.EqualFold(fontHash, "error") {
		add("font_hash_present", "fail", "font hash could not be captured", 12, "non-empty", actual.FontHash)
	} else {
		add("font_hash_present", "pass", "font hash captured", 0, "", "")
	}

	checks = append(checks, expectedFingerprintArgChecks(expectedArgs, actual)...)
	return checks
}

func expectedFingerprintArgChecks(expectedArgs []string, actual *FingerprintSnapshot) []FingerprintHealthCheck {
	checks := make([]FingerprintHealthCheck, 0, len(expectedArgs))
	add := func(id, status, message string, penalty int, expected string, actualValue string) {
		checks = append(checks, FingerprintHealthCheck{
			ID:       id,
			Status:   status,
			Message:  message,
			Expected: expected,
			Actual:   actualValue,
			Penalty:  penalty,
		})
	}

	for _, arg := range expectedArgs {
		key, val, ok := splitFlag(arg)
		if !ok || val == "" {
			continue
		}
		switch key {
		case "--fingerprint-brand":
			if containsFold(actual.UserAgent, val) {
				add("expected_brand", "pass", "expected browser brand is reflected in userAgent", 0, "", "")
			} else {
				add("expected_brand", "fail", "expected browser brand is missing from userAgent", 12, val, actual.UserAgent)
			}
		case "--user-agent":
			if containsFold(actual.UserAgent, val) {
				add("expected_user_agent", "pass", "expected userAgent substring is present", 0, "", "")
			} else {
				add("expected_user_agent", "fail", "expected userAgent substring is missing", 12, val, actual.UserAgent)
			}
		case "--fingerprint-platform":
			if platformMatches(val, actual.Platform) {
				add("expected_platform", "pass", "expected platform matches", 0, "", "")
			} else {
				add("expected_platform", "fail", "expected platform does not match", 15, val, actual.Platform)
			}
		case "--fingerprint-hardware-concurrency":
			if strconv.Itoa(actual.HardwareConcurrency) == val {
				add("expected_hardware_concurrency", "pass", "expected hardwareConcurrency matches", 0, "", "")
			} else {
				add("expected_hardware_concurrency", "fail", "expected hardwareConcurrency does not match", 10, val, strconv.Itoa(actual.HardwareConcurrency))
			}
		case "--timezone":
			if actual.Timezone == val {
				add("expected_timezone", "pass", "expected timezone matches", 0, "", "")
			} else {
				add("expected_timezone", "fail", "expected timezone does not match", 12, val, actual.Timezone)
			}
		case "--lang", "--accept-lang":
			expectedLang := primaryLanguage(val)
			if languageMatches(expectedLang, actual) {
				add("expected_language", "pass", "expected language matches", 0, "", "")
			} else {
				add("expected_language", "fail", "expected language does not match", 8, expectedLang, actual.Language)
			}
		case "--force-device-scale-factor":
			expectedVal, err := parseFloat(val)
			if err == nil && math.Abs(actual.DevicePixelRatio-expectedVal) <= 0.015 {
				add("expected_device_scale_factor", "pass", "expected device scale factor matches", 0, "", "")
			} else {
				add("expected_device_scale_factor", "fail", "expected device scale factor does not match", 8, val, fmt.Sprintf("%.2f", actual.DevicePixelRatio))
			}
		case "--window-size":
			actualValue := fmt.Sprintf("%d,%d", actual.ScreenWidth, actual.ScreenHeight)
			if val == actualValue {
				add("expected_window_size", "pass", "expected window size matches screen size", 0, "", "")
			} else {
				add("expected_window_size", "fail", "expected window size does not match screen size", 8, val, actualValue)
			}
		}
	}
	return checks
}

func fingerprintHealthScore(checks []FingerprintHealthCheck) int {
	score := 100
	for _, check := range checks {
		score -= check.Penalty
	}
	if score < 0 {
		return 0
	}
	if score > 100 {
		return 100
	}
	return score
}

func fingerprintHealthLevel(score int) string {
	switch {
	case score >= FingerprintHealthGoodMinScore:
		return FingerprintHealthLevelGood
	case score >= FingerprintHealthWarningMinScore:
		return FingerprintHealthLevelWarning
	default:
		return FingerprintHealthLevelRisk
	}
}

func splitFlag(arg string) (string, string, bool) {
	arg = strings.TrimSpace(arg)
	idx := strings.Index(arg, "=")
	if idx <= 0 {
		return "", "", false
	}
	return strings.TrimSpace(arg[:idx]), strings.TrimSpace(arg[idx+1:]), true
}

func containsFold(haystack string, needle string) bool {
	return strings.Contains(strings.ToLower(haystack), strings.ToLower(needle))
}

func platformMatches(expected string, actual string) bool {
	expected = strings.ToLower(strings.TrimSpace(expected))
	actual = strings.ToLower(strings.TrimSpace(actual))
	switch expected {
	case "windows", "win":
		return strings.Contains(actual, "win")
	case "mac", "macos", "darwin":
		return strings.Contains(actual, "mac")
	case "linux":
		return strings.Contains(actual, "linux")
	case "android":
		return strings.Contains(actual, "android") || strings.Contains(actual, "linux")
	default:
		return actual == expected || strings.Contains(actual, expected)
	}
}

func primaryLanguage(raw string) string {
	raw = strings.TrimSpace(raw)
	if comma := strings.Index(raw, ","); comma >= 0 {
		raw = raw[:comma]
	}
	if semi := strings.Index(raw, ";"); semi >= 0 {
		raw = raw[:semi]
	}
	return strings.TrimSpace(raw)
}

func languageMatches(expected string, actual *FingerprintSnapshot) bool {
	expected = strings.ToLower(strings.TrimSpace(expected))
	if expected == "" || actual == nil {
		return false
	}
	if strings.EqualFold(actual.Language, expected) {
		return true
	}
	for _, lang := range actual.Languages {
		if strings.EqualFold(lang, expected) {
			return true
		}
	}
	return false
}

func isKnownVanillaCanvasHash(hash string) bool {
	vanillaHashes := map[string]struct{}{
		"-49ff125c": {},
		"3f8a1b2c":  {},
		"7d2e9a1f":  {},
	}
	_, ok := vanillaHashes[strings.ToLower(strings.TrimSpace(hash))]
	return ok
}
