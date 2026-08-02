package browser

import "strings"

// RuntimeControlField describes one first-family control and how it maps to runtime.
type RuntimeControlField struct {
	Name       string
	LaunchArg  string // empty if injection-only
	InjectKey  string // environment injection profile JSON key
	Layer      FingerprintFieldPriorityLayer
}

// FirstFamilyRuntimeControls returns the canonical 80 first-family control fields.
func FirstFamilyRuntimeControls() []RuntimeControlField {
	names := []string{
		"browser_family", "browser_channel", "browser_major_version", "browser_minor_version",
		"user_agent", "ua_platform", "ua_brand_list", "ua_full_version_list", "ua_mobile", "ua_architecture",
		"os_name", "os_version", "os_build_number", "os_edition", "os_branch",
		"system_locale", "ui_language", "region_format", "timezone", "daylight_saving_rule",
		"screen_width", "screen_height", "available_width", "available_height", "viewport_width",
		"viewport_height", "device_pixel_ratio", "page_zoom", "color_depth", "multi_monitor_count",
		"cpu_architecture", "hardware_concurrency", "device_memory_gb", "cpu_class", "gpu_vendor",
		"gpu_renderer", "touch_support", "max_touch_points", "battery_presence", "power_plan",
		"canvas_profile", "webgl_vendor", "webgl_renderer", "webgl_version", "audio_profile",
		"font_fingerprint_profile", "media_codec_profile", "image_decode_profile", "color_gamut_profile", "hdr_support_profile",
		"locale", "accept_language", "keyboard_layout", "input_method", "text_direction",
		"date_format", "number_format", "first_day_of_week", "typing_latency_profile", "punctuation_profile",
		"proxy_type", "proxy_provider", "proxy_host", "proxy_port", "proxy_auth_mode",
		"proxy_region", "exit_ip", "dns_mode", "sticky_session_ttl", "rotation_policy",
		"click_speed_profile", "scroll_speed_profile", "pointer_smoothing_profile", "dwell_time_profile", "tab_switch_cadence",
		"session_length_profile", "risk_tolerance", "automation_guard_level", "behavior_profile_id", "humanize_seed",
	}
	fields := make([]RuntimeControlField, 0, len(names))
	for _, name := range names {
		fields = append(fields, RuntimeControlField{
			Name:      name,
			LaunchArg: launchArgForControl(name),
			InjectKey: injectKeyForControl(name),
			Layer:     layerForControl(name),
		})
	}
	return fields
}

// RuntimeProjectionReport counts how many controls are materialized for a profile.
type RuntimeProjectionReport struct {
	DeclaredTotal   int      `json:"declaredTotal"`
	AppliedCount    int      `json:"appliedCount"`
	MissingControls []string `json:"missingControls"`
	AppliedControls []string `json:"appliedControls"`
}

// BuildRuntimeProjectionReport inspects fingerprint/launch args for applied controls.
func BuildRuntimeProjectionReport(fingerprintArgs, launchArgs []string) RuntimeProjectionReport {
	controls := FirstFamilyRuntimeControls()
	args := append(append([]string{}, fingerprintArgs...), launchArgs...)
	argIndex := indexLaunchArgs(args)
	applied := make([]string, 0, len(controls))
	missing := make([]string, 0)
	for _, ctrl := range controls {
		if ctrl.LaunchArg != "" && argIndex[ctrl.LaunchArg] {
			applied = append(applied, ctrl.Name)
			continue
		}
		if hasDerivedControl(args, ctrl.Name) {
			applied = append(applied, ctrl.Name)
			continue
		}
		missing = append(missing, ctrl.Name)
	}
	return RuntimeProjectionReport{
		DeclaredTotal:   len(controls),
		AppliedCount:    len(applied),
		MissingControls: missing,
		AppliedControls: applied,
	}
}

func indexLaunchArgs(args []string) map[string]bool {
	out := make(map[string]bool)
	for _, arg := range args {
		arg = strings.TrimSpace(arg)
		if arg == "" {
			continue
		}
		key := arg
		if idx := strings.Index(arg, "="); idx > 0 {
			key = arg[:idx]
		}
		out[key] = true
	}
	return out
}

func hasDerivedControl(args []string, name string) bool {
	switch name {
	case "platform", "ua_platform":
		return indexLaunchArgs(args)["--fingerprint-platform"]
	case "locale", "system_locale", "ui_language":
		return indexLaunchArgs(args)["--lang"]
	case "accept_language":
		return indexLaunchArgs(args)["--accept-lang"] || indexLaunchArgs(args)["--accept-language"]
	case "hardware_concurrency":
		return indexLaunchArgs(args)["--fingerprint-hardware-concurrency"]
	case "device_memory_gb":
		return indexLaunchArgs(args)["--fingerprint-device-memory"]
	case "device_pixel_ratio":
		return indexLaunchArgs(args)["--force-device-scale-factor"]
	case "viewport_width", "viewport_height", "screen_width", "screen_height":
		return indexLaunchArgs(args)["--window-size"]
	case "webgl_vendor", "webgl_renderer", "gpu_vendor", "gpu_renderer":
		return indexLaunchArgs(args)["--fingerprint-webgl-vendor"] || indexLaunchArgs(args)["--fingerprint-webgl-renderer"]
	case "canvas_profile":
		return indexLaunchArgs(args)["--fingerprint-canvas-noise"] || indexLaunchArgs(args)["--fingerprint"]
	case "audio_profile":
		return indexLaunchArgs(args)["--fingerprint-audio-noise"]
	case "font_fingerprint_profile":
		return indexLaunchArgs(args)["--fingerprint-fonts"]
	case "max_touch_points", "touch_support":
		return indexLaunchArgs(args)["--fingerprint-touch-points"]
	case "dns_mode":
		return indexLaunchArgs(args)["--host-resolver-rules"]
	default:
		return false
	}
}

func launchArgForControl(name string) string {
	switch name {
	case "user_agent":
		return "--user-agent"
	case "timezone", "daylight_saving_rule":
		return "--timezone"
	case "locale", "system_locale", "ui_language":
		return "--lang"
	case "accept_language":
		return "--accept-lang"
	case "hardware_concurrency":
		return "--fingerprint-hardware-concurrency"
	case "device_memory_gb":
		return "--fingerprint-device-memory"
	case "device_pixel_ratio":
		return "--force-device-scale-factor"
	case "viewport_width", "viewport_height", "screen_width", "screen_height":
		return "--window-size"
	case "webgl_vendor", "gpu_vendor":
		return "--fingerprint-webgl-vendor"
	case "webgl_renderer", "gpu_renderer":
		return "--fingerprint-webgl-renderer"
	case "canvas_profile":
		return "--fingerprint-canvas-noise"
	case "audio_profile":
		return "--fingerprint-audio-noise"
	case "font_fingerprint_profile":
		return "--fingerprint-fonts"
	case "max_touch_points":
		return "--fingerprint-touch-points"
	case "browser_family", "browser_major_version":
		return "--fingerprint-brand"
	case "ua_platform", "cpu_architecture":
		return "--fingerprint-platform"
	case "ua_full_version_list", "browser_minor_version":
		return "--fingerprint-brand-version"
	case "os_version", "os_build_number":
		return "--fingerprint-platform-version"
	case "dns_mode":
		return "--host-resolver-rules"
	default:
		return ""
	}
}

func injectKeyForControl(name string) string {
	switch name {
	case "user_agent":
		return "userAgent"
	case "hardware_concurrency":
		return "hardwareConcurrency"
	case "device_memory_gb":
		return "deviceMemory"
	case "timezone":
		return "timezone"
	case "accept_language", "locale":
		return "acceptLanguage"
	case "webgl_vendor", "gpu_vendor":
		return "webglVendor"
	case "webgl_renderer", "gpu_renderer":
		return "webglRenderer"
	default:
		return ""
	}
}

func layerForControl(name string) FingerprintFieldPriorityLayer {
	switch name {
	case "user_agent", "accept_language", "locale", "timezone", "viewport_width", "viewport_height", "ua_platform":
		return FPLayerL1
	case "hardware_concurrency", "device_memory_gb", "device_pixel_ratio", "browser_major_version", "os_version":
		return FPLayerL2
	default:
		return FPLayerL3
	}
}
