package browser

import (
	"fmt"
	"strconv"
	"strings"
	"time"
)

const (
	IdentityReportSourceLocalCDP = "local-cdp"

	IdentityLevelStrong = "strong"
	IdentityLevelNormal = "normal"
	IdentityLevelWeak   = "weak"
	IdentityLevelRisk   = "risk"
)

type IdentityReportContext struct {
	LaunchAuditError string
}

type IdentityStrengthReport struct {
	ProfileID   string               `json:"profileId"`
	ProfileName string               `json:"profileName,omitempty"`
	Score       int                  `json:"score"`
	Level       string               `json:"level"`
	Subscores   IdentitySubscores    `json:"subscores"`
	Dimensions  []IdentityDimension  `json:"dimensions"`
	Fingerprint *FingerprintSnapshot `json:"fingerprint,omitempty"`
	CapturedAt  string               `json:"capturedAt"`
	Source      string               `json:"source"`
	Summary     []string             `json:"summary"`
}

type IdentitySubscores struct {
	FingerprintVisible  int `json:"fingerprintVisible"`
	Consistency         int `json:"consistency"`
	ProfilePersistence  int `json:"profilePersistence"`
	LongTermCoherence   int `json:"longTermCoherence"`
	ProxyNetwork        int `json:"proxyNetwork"`
	BehaviorNaturalness int `json:"behaviorNaturalness"`
	AutomationSafety    int `json:"automationSafety"`
}

type IdentityDimension struct {
	ID       string `json:"id"`
	Category string `json:"category"`
	Layer    string `json:"layer"`
	Status   string `json:"status"`
	Message  string `json:"message"`
	Expected string `json:"expected,omitempty"`
	Actual   string `json:"actual,omitempty"`
	Penalty  int    `json:"penalty,omitempty"`
}

func NewIdentityStrengthReport(profile *Profile, fingerprint *FingerprintSnapshot, capturedAt time.Time, ctx IdentityReportContext) *IdentityStrengthReport {
	if capturedAt.IsZero() {
		capturedAt = time.Now()
	}
	dims := buildIdentityDimensions(profile, fingerprint, ctx)
	subscores := buildIdentitySubscores(profile, fingerprint, dims)
	score := weightedIdentityScore(subscores)
	report := &IdentityStrengthReport{
		Score:       score,
		Level:       identityLevel(score, dims),
		Subscores:   subscores,
		Dimensions:  dims,
		Fingerprint: fingerprint,
		CapturedAt:  capturedAt.UTC().Format(time.RFC3339Nano),
		Source:      IdentityReportSourceLocalCDP,
		Summary:     identitySummary(dims),
	}
	if profile != nil {
		report.ProfileID = strings.TrimSpace(profile.ProfileId)
		report.ProfileName = strings.TrimSpace(profile.ProfileName)
	}
	return report
}

func buildIdentityDimensions(profile *Profile, fp *FingerprintSnapshot, ctx IdentityReportContext) []IdentityDimension {
	dims := make([]IdentityDimension, 0, 72)
	add := func(id, category, layer, status, message, expected, actual string, penalty int) {
		dims = append(dims, IdentityDimension{
			ID: id, Category: category, Layer: layer, Status: status,
			Message: message, Expected: expected, Actual: actual, Penalty: penalty,
		})
	}
	addValue := func(id, category, layer, value string, required bool) {
		value = strings.TrimSpace(value)
		if value == "" {
			if required {
				add(id, category, layer, "fail", "missing required identity value", "non-empty", value, 10)
			} else {
				add(id, category, layer, "info", "not exposed by this browser/runtime", "", value, 0)
			}
			return
		}
		add(id, category, layer, "pass", "captured", "", value, 0)
	}
	addNumber := func(id, category, layer string, value int, min int, required bool) {
		actual := strconv.Itoa(value)
		if value < min {
			if required {
				add(id, category, layer, "fail", "numeric value is outside usable range", fmt.Sprintf(">= %d", min), actual, 8)
			} else {
				add(id, category, layer, "warning", "numeric value is unavailable or low", fmt.Sprintf(">= %d", min), actual, 4)
			}
			return
		}
		add(id, category, layer, "pass", "captured", "", actual, 0)
	}

	if fp == nil {
		add("fingerprint_capture", "fingerprint", "L1", "fail", "fingerprint snapshot is empty", "captured snapshot", "", 100)
		return dims
	}

	addValue("user_agent", "fingerprint", "L1", fp.UserAgent, true)
	addValue("app_version", "fingerprint", "L1", fp.AppVersion, false)
	addValue("app_name", "fingerprint", "L1", fp.AppName, false)
	addValue("product", "fingerprint", "L1", fp.Product, false)
	addValue("product_sub", "fingerprint", "L1", fp.ProductSub, false)
	addValue("platform", "fingerprint", "L1", fp.Platform, true)
	if fp.Webdriver {
		add("webdriver", "fingerprint", "L1", "fail", "navigator.webdriver is exposed", "false", "true", 35)
	} else {
		add("webdriver", "fingerprint", "L1", "pass", "webdriver is not exposed", "false", "false", 0)
	}
	add("cookie_enabled", "fingerprint", "L1", boolStatus(fp.CookieEnabled), "cookie availability captured", "true", strconv.FormatBool(fp.CookieEnabled), boolPenalty(!fp.CookieEnabled, 8))
	addValue("do_not_track", "fingerprint", "L2", fp.DoNotTrack, false)
	add("pdf_viewer_enabled", "fingerprint", "L2", "info", "pdf viewer capability captured", "", strconv.FormatBool(fp.PDFViewerEnabled), 0)
	add("online", "fingerprint", "L2", boolStatus(fp.Online), "network online flag captured", "true", strconv.FormatBool(fp.Online), boolPenalty(!fp.Online, 4))
	addNumber("hardware_concurrency", "fingerprint", "L2", fp.HardwareConcurrency, 1, true)
	addNumber("device_memory", "fingerprint", "L2", fp.DeviceMemory, 1, false)
	addNumber("color_depth", "fingerprint", "L2", fp.ColorDepth, 1, true)
	addNumber("pixel_depth", "fingerprint", "L2", fp.PixelDepth, 1, true)
	addNumber("screen_width", "fingerprint", "L1", fp.ScreenWidth, 1, true)
	addNumber("screen_height", "fingerprint", "L1", fp.ScreenHeight, 1, true)
	addNumber("avail_width", "fingerprint", "L1", fp.AvailWidth, 1, false)
	addNumber("avail_height", "fingerprint", "L1", fp.AvailHeight, 1, false)
	addValue("device_pixel_ratio", "fingerprint", "L1", fmt.Sprintf("%.2f", fp.DevicePixelRatio), true)
	addNumber("max_touch_points", "fingerprint", "L2", fp.MaxTouchPoints, 0, false)
	addValue("vendor", "fingerprint", "L1", fp.Vendor, false)
	addValue("timezone", "fingerprint", "L1", fp.Timezone, true)
	addValue("timezone_offset", "fingerprint", "L1", strconv.Itoa(fp.TimezoneOffset), true)
	addValue("language", "fingerprint", "L1", fp.Language, true)
	addValue("languages", "fingerprint", "L1", strings.Join(fp.Languages, ","), false)
	addValue("intl_locale", "fingerprint", "L1", fp.IntlLocale, false)
	addValue("intl_calendar", "fingerprint", "L2", fp.IntlCalendar, false)
	addValue("intl_numbering_system", "fingerprint", "L2", fp.IntlNumberingSystem, false)
	addValue("date_format_sample", "fingerprint", "L2", fp.DateFormatSample, false)
	addValue("number_format_sample", "fingerprint", "L2", fp.NumberFormatSample, false)
	addValue("client_hints_brands", "fingerprint", "L2", strings.Join(fp.UADataBrands, ","), false)
	add("client_hints_mobile", "fingerprint", "L2", "info", "client hints mobile flag captured", "", strconv.FormatBool(fp.UADataMobile), 0)
	addValue("client_hints_platform", "fingerprint", "L2", fp.UADataPlatform, false)
	addValue("client_hints_platform_version", "fingerprint", "L2", fp.UADataPlatformVer, false)
	addValue("client_hints_architecture", "fingerprint", "L2", fp.UADataArchitecture, false)
	addValue("client_hints_bitness", "fingerprint", "L2", fp.UADataBitness, false)
	addValue("client_hints_model", "fingerprint", "L2", fp.UADataModel, false)
	addValue("client_hints_full_versions", "fingerprint", "L2", strings.Join(fp.UADataFullVersions, ","), false)
	addUACoreCoherenceDimension(add, profile, fp)
	addNumber("inner_width", "fingerprint", "L1", fp.InnerWidth, 1, true)
	addNumber("inner_height", "fingerprint", "L1", fp.InnerHeight, 1, true)
	addNumber("outer_width", "fingerprint", "L1", fp.OuterWidth, 1, false)
	addNumber("outer_height", "fingerprint", "L1", fp.OuterHeight, 1, false)
	addValue("visual_viewport", "fingerprint", "L2", fmt.Sprintf("%.0fx%.0f@%.2f", fp.VisualViewportWidth, fp.VisualViewportHeight, fp.VisualViewportScale), false)
	add("pointer_fine", "fingerprint", "L2", "info", "pointer media query captured", "", strconv.FormatBool(fp.PointerFine), 0)
	add("pointer_coarse", "fingerprint", "L2", "info", "pointer media query captured", "", strconv.FormatBool(fp.PointerCoarse), 0)
	add("hover_hover", "fingerprint", "L2", "info", "hover media query captured", "", strconv.FormatBool(fp.HoverHover), 0)
	add("hover_none", "fingerprint", "L2", "info", "hover media query captured", "", strconv.FormatBool(fp.HoverNone), 0)
	addValue("prefers_color_scheme", "fingerprint", "L2", fp.PrefersColorScheme, false)
	addValue("prefers_reduced_motion", "fingerprint", "L2", fp.PrefersReducedMotion, false)
	addValue("network_effective_type", "fingerprint", "L2", fp.NetworkEffectiveType, false)
	addValue("network_downlink", "fingerprint", "L2", fmt.Sprintf("%.2f", fp.NetworkDownlink), false)
	addValue("network_rtt", "fingerprint", "L2", strconv.Itoa(fp.NetworkRTT), false)
	add("network_save_data", "fingerprint", "L2", "info", "network save-data flag captured", "", strconv.FormatBool(fp.NetworkSaveData), 0)
	addValue("storage_quota", "fingerprint", "L2", strconv.FormatInt(fp.StorageQuota, 10), false)
	addValue("storage_usage", "fingerprint", "L2", strconv.FormatInt(fp.StorageUsage, 10), false)
	addHashDimension(add, "canvas_hash", "L3", fp.CanvasHash, 18)
	addValue("webgl_vendor", "fingerprint", "L3", fp.WebGLVendor, false)
	addValue("webgl_renderer", "fingerprint", "L3", fp.WebGLRenderer, false)
	addHashDimension(add, "webgl_extensions_hash", "L3", fp.WebGLExtensionsHash, 8)
	addNumber("webgl_max_texture_size", "fingerprint", "L3", fp.WebGLMaxTextureSize, 1, false)
	addNumber("webgl_max_vertex_attribs", "fingerprint", "L3", fp.WebGLMaxVertexAttribs, 1, false)
	addValue("webgl_max_viewport_dims", "fingerprint", "L3", fp.WebGLMaxViewportDims, false)
	addHashDimension(add, "font_hash", "L3", fp.FontHash, 12)
	addHashDimension(add, "audio_hash", "L3", fp.AudioHash, 6)
	addHashDimension(add, "plugins_hash", "L3", fp.PluginsHash, 4)
	addHashDimension(add, "mime_types_hash", "L3", fp.MimeTypesHash, 4)
	add("webgpu_available", "fingerprint", "L3", "info", "WebGPU availability captured", "", strconv.FormatBool(fp.WebGPUAvailable), 0)
	add("webrtc_supported", "fingerprint", "L3", "info", "WebRTC API availability captured without external STUN", "", strconv.FormatBool(fp.WebRTCSupported), 0)

	dims = append(dims, expectedIdentityDimensions(profile, fp)...)
	dims = append(dims, consistencyIdentityDimensions(profile, fp)...)
	dims = append(dims, profileGuardDimensions(profile, ctx)...)
	dims = append(dims, longTermCoherenceDimensions(profile)...)
	return dims
}

func addHashDimension(add func(id, category, layer, status, message, expected, actual string, penalty int), id, layer, value string, penalty int) {
	value = strings.TrimSpace(value)
	if value == "" || strings.EqualFold(value, "error") {
		add(id, "fingerprint", layer, "fail", "hash could not be captured", "stable non-empty hash", value, penalty)
		return
	}
	if strings.EqualFold(value, "unsupported") {
		add(id, "fingerprint", layer, "info", "capability is unsupported in this browser", "", value, 0)
		return
	}
	add(id, "fingerprint", layer, "pass", "hash captured", "", value, 0)
}

func expectedIdentityDimensions(profile *Profile, fp *FingerprintSnapshot) []IdentityDimension {
	if profile == nil || fp == nil {
		return nil
	}
	checks := expectedFingerprintArgChecks(profile.FingerprintArgs, fp)
	dims := make([]IdentityDimension, 0, len(checks))
	for _, check := range checks {
		dims = append(dims, IdentityDimension{
			ID:       check.ID,
			Category: "expected",
			Layer:    "L1",
			Status:   check.Status,
			Message:  check.Message,
			Expected: check.Expected,
			Actual:   check.Actual,
			Penalty:  check.Penalty,
		})
	}
	return dims
}

func consistencyIdentityDimensions(profile *Profile, fp *FingerprintSnapshot) []IdentityDimension {
	if fp == nil {
		return nil
	}
	input := &FingerprintConsistencyInput{
		Timezone:                fp.Timezone,
		Locale:                  fp.Language,
		AcceptLanguage:          strings.Join(fp.Languages, ","),
		ScreenWidth:             fp.ScreenWidth,
		ScreenHeight:            fp.ScreenHeight,
		AvailWidth:              fp.AvailWidth,
		AvailHeight:             fp.AvailHeight,
		WebGLVendor:             fp.WebGLVendor,
		WebGLRenderer:           fp.WebGLRenderer,
		SupportsTouch:           fp.MaxTouchPoints > 0,
		MaxTouchPoints:          fp.MaxTouchPoints,
		HardwareConcurrency:     fp.HardwareConcurrency,
		DeviceMemory:            fp.DeviceMemory,
		Platform:                fp.Platform,
		AutomationPolicyEnabled: profile != nil && strings.TrimSpace(profile.BehaviorProfileID) != "",
		BehaviorCadence:         "human",
	}
	result := AssessFingerprintConsistency(input)
	dims := make([]IdentityDimension, 0, len(result.CheckItems))
	for _, item := range result.CheckItems {
		status := "pass"
		penalty := 0
		if !item.Passed {
			switch item.Severity {
			case CheckHard:
				status = "fail"
				penalty = 12
			case CheckSoft:
				status = "warning"
				penalty = 8
			default:
				status = "info"
				penalty = 3
			}
		}
		dims = append(dims, IdentityDimension{
			ID:       item.Dimension,
			Category: "consistency",
			Layer:    "L4",
			Status:   status,
			Message:  item.Detail,
			Penalty:  penalty,
		})
	}
	return dims
}

func profileGuardDimensions(profile *Profile, ctx IdentityReportContext) []IdentityDimension {
	dims := make([]IdentityDimension, 0, 8)
	add := func(id, status, message, expected, actual string, penalty int) {
		dims = append(dims, IdentityDimension{ID: id, Category: "profile", Layer: "L4", Status: status, Message: message, Expected: expected, Actual: actual, Penalty: penalty})
	}
	if profile == nil {
		add("profile_present", "fail", "profile is missing", "profile", "", 100)
		return dims
	}
	add("profile_id_present", valueStatus(profile.ProfileId, true), "profile id captured", "non-empty", profile.ProfileId, valuePenalty(profile.ProfileId, true, 12))
	userDataDir := strings.TrimSpace(profile.UserDataDir)
	add("user_data_dir_present", valueStatus(userDataDir, true), "user-data-dir binding captured", "non-empty", userDataDir, valuePenalty(userDataDir, true, 20))
	if containsParentPathSegment(userDataDir) {
		add("user_data_dir_no_traversal", "fail", "user-data-dir contains parent traversal", "no .. segments", userDataDir, 35)
	} else {
		add("user_data_dir_no_traversal", "pass", "user-data-dir has no parent traversal marker", "", userDataDir, 0)
	}
	add("fingerprint_args_present", valueStatus(strings.Join(profile.FingerprintArgs, " "), true), "fingerprint args are bound to the profile", "non-empty", fmt.Sprintf("%d args", len(profile.FingerprintArgs)), valuePenalty(strings.Join(profile.FingerprintArgs, " "), true, 12))
	add("humanize_seed_present", valueStatus(profile.HumanizeSeed, true), "stable behavior persona seed captured", "non-empty", profile.HumanizeSeed, valuePenalty(profile.HumanizeSeed, true, 8))
	if profile.LaunchAudit == nil {
		add("launch_audit_present", "warning", "launch audit is missing or instance has not been started by this runtime", "runtime launch audit", "", 12)
	} else {
		add("launch_audit_present", "pass", "launch audit captured", "", profile.LaunchAudit.Timestamp, 0)
	}
	if strings.TrimSpace(ctx.LaunchAuditError) != "" {
		add("launch_audit_valid", "fail", "launch audit validation failed", "valid audit", ctx.LaunchAuditError, 30)
	} else {
		add("launch_audit_valid", "pass", "launch audit validation passed or was not required", "", "", 0)
	}
	return dims
}

func longTermCoherenceDimensions(profile *Profile) []IdentityDimension {
	dims := make([]IdentityDimension, 0, 10)
	add := func(id, status, message, expected, actual string, penalty int) {
		dims = append(dims, IdentityDimension{
			ID: id, Category: "long_term", Layer: "L5", Status: status,
			Message: message, Expected: expected, Actual: actual, Penalty: penalty,
		})
	}
	if profile == nil {
		add("long_term_profile_present", "fail", "profile is missing", "profile", "", 100)
		return dims
	}

	createdAt := strings.TrimSpace(profile.CreatedAt)
	updatedAt := strings.TrimSpace(profile.UpdatedAt)
	if createdAt == "" || updatedAt == "" {
		add("long_term_profile_timeline", "warning", "profile created/updated timeline is incomplete", "createdAt and updatedAt", createdAt+" / "+updatedAt, 12)
	} else {
		add("long_term_profile_timeline", "pass", "profile created/updated timeline captured", "", createdAt+" / "+updatedAt, 0)
	}

	lastStartAt := strings.TrimSpace(profile.LastStartAt)
	lastStopAt := strings.TrimSpace(profile.LastStopAt)
	if lastStartAt == "" && lastStopAt == "" {
		add("long_term_runtime_timeline", "warning", "runtime start/stop history is not recorded yet", "lastStartAt or lastStopAt", "", 12)
	} else {
		add("long_term_runtime_timeline", "pass", "runtime start/stop history captured", "", lastStartAt+" / "+lastStopAt, 0)
	}

	if strings.TrimSpace(profile.HumanizeSeed) == "" {
		add("long_term_behavior_seed_stable", "fail", "stable humanize seed is missing", "non-empty stable seed", "", 18)
	} else {
		add("long_term_behavior_seed_stable", "pass", "stable humanize seed is present", "", "present", 0)
	}

	if strings.TrimSpace(profile.BehaviorProfileID) == "" {
		add("long_term_behavior_profile_bound", "warning", "behavior profile is not bound", "behavior profile id", "", 8)
	} else {
		add("long_term_behavior_profile_bound", "pass", "behavior profile is bound", "", profile.BehaviorProfileID, 0)
	}

	if strings.TrimSpace(profile.ProxyId) == "" &&
		strings.TrimSpace(profile.ProxyConfig) == "" &&
		strings.TrimSpace(profile.ProxyBindName) == "" &&
		strings.TrimSpace(profile.ProxyBindSourceID) == "" &&
		strings.TrimSpace(profile.ProxyBindSourceURL) == "" {
		add("long_term_proxy_binding", "warning", "proxy binding history is missing", "proxy id/config/binding", "", 10)
	} else {
		actual := strings.TrimSpace(profile.ProxyId)
		if actual == "" {
			actual = strings.TrimSpace(profile.ProxyBindName)
		}
		if actual == "" && strings.TrimSpace(profile.ProxyConfig) != "" {
			actual = "inline proxy config"
		}
		add("long_term_proxy_binding", "pass", "proxy binding identity is present", "", actual, 0)
	}

	if strings.TrimSpace(profile.ProxyBindUpdatedAt) != "" {
		add("long_term_proxy_bind_time", "pass", "proxy binding update time captured", "", profile.ProxyBindUpdatedAt, 0)
	} else if strings.TrimSpace(profile.ProxyBindName) != "" || strings.TrimSpace(profile.ProxyBindSourceID) != "" || strings.TrimSpace(profile.ProxyBindSourceURL) != "" {
		add("long_term_proxy_bind_time", "warning", "proxy binding exists but update time is missing", "proxyBindUpdatedAt", "", 6)
	} else {
		add("long_term_proxy_bind_time", "info", "no proxy binding update time because no binding is configured", "", "", 0)
	}

	if profile.LaunchAudit == nil {
		add("long_term_launch_audit", "warning", "current run launch audit is missing", "launch audit", "", 12)
	} else {
		add("long_term_launch_audit", "pass", "current run launch audit captured", "", profile.LaunchAudit.Timestamp, 0)
	}

	return dims
}

func addUACoreCoherenceDimension(
	add func(id, category, layer, status, message, expected, actual string, penalty int),
	profile *Profile,
	fp *FingerprintSnapshot,
) {
	expected := ResolveChromiumVersion(nil, "", profile)
	if expected.Source == "fallback" {
		add("ua_core_coherent", "consistency", "L2", "info", "core version is not explicitly bound to the profile", "explicit core/runtime version", "fallback="+expected.Full, 0)
		return
	}
	uaVersion, uaOK := coreVersionFromText(fp.UserAgent, "observed_ua")
	clientHintsVersion, hintsOK := firstClientHintsVersion(fp.UADataFullVersions)
	actual := fmt.Sprintf("ua=%s; ua_ch=%s", uaVersion.Full, clientHintsVersion.Full)
	if !uaOK || !hintsOK {
		add("ua_core_coherent", "consistency", "L2", "warning", "UA/core coherence evidence is incomplete", expected.Full, actual, 8)
		return
	}
	if uaVersion.Major == expected.Major && clientHintsVersion.Major == expected.Major {
		add("ua_core_coherent", "consistency", "L2", "pass", "UA, UA-CH and core major are coherent", expected.Full, actual, 0)
		return
	}
	add("ua_core_coherent", "consistency", "L2", "fail", "UA or UA-CH major differs from the selected core", expected.Full, actual, 20)
}

func firstClientHintsVersion(items []string) (CoreVersionInfo, bool) {
	for _, item := range items {
		lower := strings.ToLower(item)
		if !strings.Contains(lower, "chrome") && !strings.Contains(lower, "chromium") {
			continue
		}
		if info, ok := coreVersionFromText(item, "observed_ua_ch"); ok {
			return info, true
		}
	}
	return CoreVersionInfo{}, false
}

func buildIdentitySubscores(profile *Profile, fp *FingerprintSnapshot, dims []IdentityDimension) IdentitySubscores {
	return IdentitySubscores{
		FingerprintVisible:  scoreCategory(dims, "fingerprint"),
		Consistency:         scoreCategory(dims, "consistency"),
		ProfilePersistence:  scoreCategory(dims, "profile"),
		LongTermCoherence:   scoreCategory(dims, "long_term"),
		ProxyNetwork:        proxyNetworkScore(profile),
		BehaviorNaturalness: behaviorNaturalnessScore(profile),
		AutomationSafety:    automationSafetyScore(fp),
	}
}

func scoreCategory(dims []IdentityDimension, category string) int {
	score := 100
	for _, dim := range dims {
		if dim.Category == category {
			score -= dim.Penalty
		}
	}
	return clampIdentityScore(score)
}

func proxyNetworkScore(profile *Profile) int {
	if profile == nil {
		return 0
	}
	if strings.TrimSpace(profile.ProxyId) != "" || strings.TrimSpace(profile.ProxyConfig) != "" || strings.TrimSpace(profile.ProxyBindName) != "" {
		return 90
	}
	return 75
}

func behaviorNaturalnessScore(profile *Profile) int {
	if profile == nil {
		return 0
	}
	score := 70
	if strings.TrimSpace(profile.HumanizeSeed) != "" {
		score += 15
	}
	if strings.TrimSpace(profile.BehaviorProfileID) != "" {
		score += 10
	}
	if strings.TrimSpace(profile.LastStartAt) != "" && strings.TrimSpace(profile.CreatedAt) != "" {
		score += 5
	}
	return clampIdentityScore(score)
}

func automationSafetyScore(fp *FingerprintSnapshot) int {
	if fp == nil {
		return 0
	}
	if fp.Webdriver {
		return 0
	}
	return 100
}

func weightedIdentityScore(s IdentitySubscores) int {
	score := float64(s.FingerprintVisible)*0.27 +
		float64(s.Consistency)*0.22 +
		float64(s.ProfilePersistence)*0.16 +
		float64(s.LongTermCoherence)*0.10 +
		float64(s.ProxyNetwork)*0.10 +
		float64(s.BehaviorNaturalness)*0.10 +
		float64(s.AutomationSafety)*0.05
	return clampIdentityScore(int(score + 0.5))
}

func identityLevel(score int, dims []IdentityDimension) string {
	for _, dim := range dims {
		if dim.Status == "fail" && (dim.ID == "webdriver" || dim.ID == "user_data_dir_no_traversal" || dim.ID == "launch_audit_valid") {
			return IdentityLevelRisk
		}
	}
	switch {
	case score >= 90:
		return IdentityLevelStrong
	case score >= 75:
		return IdentityLevelNormal
	case score >= 55:
		return IdentityLevelWeak
	default:
		return IdentityLevelRisk
	}
}

func identitySummary(dims []IdentityDimension) []string {
	items := make([]string, 0, 4)
	for _, dim := range dims {
		if dim.Status == "fail" || dim.Status == "warning" {
			items = append(items, dim.ID+": "+dim.Message)
			if len(items) >= 4 {
				break
			}
		}
	}
	if len(items) == 0 {
		items = append(items, "identity dimensions are coherent")
	}
	return items
}

func boolStatus(ok bool) string {
	if ok {
		return "pass"
	}
	return "warning"
}

func boolPenalty(bad bool, penalty int) int {
	if bad {
		return penalty
	}
	return 0
}

func valueStatus(value string, required bool) string {
	if strings.TrimSpace(value) == "" {
		if required {
			return "fail"
		}
		return "info"
	}
	return "pass"
}

func valuePenalty(value string, required bool, penalty int) int {
	if required && strings.TrimSpace(value) == "" {
		return penalty
	}
	return 0
}

func containsParentPathSegment(value string) bool {
	normalized := strings.ReplaceAll(strings.TrimSpace(value), "\\", "/")
	for _, segment := range strings.Split(normalized, "/") {
		if segment == ".." {
			return true
		}
	}
	return false
}

func clampIdentityScore(score int) int {
	if score < 0 {
		return 0
	}
	if score > 100 {
		return 100
	}
	return score
}
