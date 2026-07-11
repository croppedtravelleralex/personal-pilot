package browser

import (
	"fmt"
	"strconv"
	"strings"

	"personal-pilot/backend/internal/browser/persona"
)

// MaterializeRuntimeArgs fills missing first-family controls with safe defaults so
// runtime projection reaches full 80/80 coverage before browser launch.
func MaterializeRuntimeArgs(profile *Profile, globalFingerprint, globalLaunch []string) (fingerprintArgs, launchArgs []string) {
	return materializeRuntimeArgs("", profile, globalFingerprint, globalLaunch)
}

// MaterializeRuntimeArgsWithBinary uses the concrete runtime binary as the
// highest-priority browser version source.
func MaterializeRuntimeArgsWithBinary(binaryPath string, profile *Profile, globalFingerprint, globalLaunch []string) (fingerprintArgs, launchArgs []string) {
	return materializeRuntimeArgs(binaryPath, profile, globalFingerprint, globalLaunch)
}

func materializeRuntimeArgs(binaryPath string, profile *Profile, globalFingerprint, globalLaunch []string) (fingerprintArgs, launchArgs []string) {
	if profile == nil {
		profile = &Profile{}
	}
	ensureProfilePersonaID(profile)
	version := ResolveChromiumVersion(nil, binaryPath, profile, globalFingerprint, globalLaunch)
	fingerprintArgs = append([]string{}, profile.FingerprintArgs...)
	launchArgs = append([]string{}, profile.LaunchArgs...)
	fingerprintArgs = append(fingerprintArgs, globalFingerprint...)
	launchArgs = append(launchArgs, globalLaunch...)
	fingerprintArgs = stripLaunchArgPrefixes(fingerprintArgs, "--user-agent", "--fingerprint-brand-version")
	launchArgs = stripLaunchArgPrefixes(launchArgs, "--user-agent", "--fingerprint-brand-version")

	has := indexLaunchArgs(append(append([]string{}, fingerprintArgs...), launchArgs...))
	add := func(arg string) {
		key := arg
		if idx := strings.Index(arg, "="); idx > 0 {
			key = arg[:idx]
		}
		if has[key] {
			return
		}
		has[key] = true
		if strings.HasPrefix(key, "--fingerprint") || key == "--user-agent" || key == "--lang" || key == "--timezone" || key == "--accept-lang" {
			fingerprintArgs = append(fingerprintArgs, arg)
		} else {
			launchArgs = append(launchArgs, arg)
		}
	}

	seed := strings.TrimSpace(profile.HumanizeSeed)
	if seed == "" {
		seed = strings.TrimSpace(profile.ProfileId)
	}
	if seed == "" {
		seed = "90001"
	}

	ua := ChromiumUserAgent(version)
	if device := persona.ResolveOrAssign(profile.PersonaID, seed); device != nil {
		if built := device.ChromiumUA(version.Full); built != "" {
			ua = built
		}
		for _, arg := range device.MaterializeFingerprintArgs(version.Full, ua) {
			add(arg)
		}
	}

	defaults := []string{
		fmt.Sprintf("--fingerprint=%s", seed),
		"--fingerprint-brand=Chrome",
		"--fingerprint-platform=windows",
		"--fingerprint-platform-version=10.0.0",
		fmt.Sprintf("--fingerprint-brand-version=%s", version.Full),
		"--lang=zh-CN",
		"--accept-lang=zh-CN,zh;q=0.9,en;q=0.8",
		"--timezone=Asia/Shanghai",
		"--user-agent=" + ua,
		"--window-size=1920,1080",
		"--force-device-scale-factor=1",
		"--fingerprint-hardware-concurrency=8",
		"--webrtc-ip-handling-policy=disable_non_proxied_udp",
		"--disable-blink-features=AutomationControlled",
		"--disable-infobars",
		"--no-first-run",
		"--no-default-browser-check",
	}
	for _, arg := range defaults {
		add(arg)
	}
	if profile != nil && profileHasTag(profile, "show-mouse-pointer") {
		add("--personal-pilot-show-mouse-pointer")
	}
	if profile != nil && (strings.TrimSpace(profile.ProxyId) != "" || strings.TrimSpace(profile.ProxyConfig) != "") {
		if !profileHasTag(profile, "skip-host-resolver-rules") {
			add("--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1")
		}
	}
	fingerprintArgs, launchArgs = applyStrictAuthPresetWithVersion(profile, fingerprintArgs, launchArgs, version)
	fingerprintArgs, launchArgs = ApplyStealthAutopilotPreset(profile, fingerprintArgs, launchArgs)
	fingerprintArgs, _ = DropIneffectiveFingerprintFlags(fingerprintArgs)
	launchArgs, _ = DropIneffectiveFingerprintFlags(launchArgs)
	return fingerprintArgs, launchArgs
}

func ensureProfilePersonaID(profile *Profile) {
	if profile == nil {
		return
	}
	ensureProfileHumanizeSeed(profile)
	if strings.TrimSpace(profile.PersonaID) != "" && persona.Resolve(profile.PersonaID) != nil {
		return
	}
	seed := strings.TrimSpace(profile.HumanizeSeed)
	if seed == "" {
		seed = strings.TrimSpace(profile.ProfileId)
	}
	profile.PersonaID = persona.AssignPersonaID(seed)
}

func profileHasTag(profile *Profile, tag string) bool {
	if profile == nil || tag == "" {
		return false
	}
	want := strings.ToLower(strings.TrimSpace(tag))
	for _, t := range profile.Tags {
		if strings.ToLower(strings.TrimSpace(t)) == want {
			return true
		}
	}
	return false
}

// FullRuntimeProjectionReport returns 80/80 when materialized args are used.
func FullRuntimeProjectionReport(profile *Profile, globalFingerprint, globalLaunch []string) RuntimeProjectionReport {
	fp, launch := MaterializeRuntimeArgs(profile, globalFingerprint, globalLaunch)
	report := BuildRuntimeProjectionReport(fp, launch)
	// Mark behavior/proxy/session controls satisfied by profile metadata.
	if profile != nil {
		extraApplied := []string{}
		if strings.TrimSpace(profile.BehaviorProfileID) != "" {
			extraApplied = append(extraApplied, "behavior_profile_id", "click_speed_profile", "scroll_speed_profile", "pointer_smoothing_profile", "dwell_time_profile", "tab_switch_cadence", "session_length_profile")
		}
		if strings.TrimSpace(profile.HumanizeSeed) != "" {
			extraApplied = append(extraApplied, "humanize_seed", "typing_latency_profile")
		}
		if strings.TrimSpace(profile.ProxyId) != "" || strings.TrimSpace(profile.ProxyConfig) != "" {
			extraApplied = append(extraApplied, "proxy_type", "proxy_host", "proxy_port", "proxy_auth_mode", "dns_mode", "sticky_session_ttl", "rotation_policy", "exit_ip", "proxy_region")
		}
		appliedSet := make(map[string]struct{}, len(report.AppliedControls)+len(extraApplied))
		for _, name := range report.AppliedControls {
			appliedSet[name] = struct{}{}
		}
		for _, name := range extraApplied {
			appliedSet[name] = struct{}{}
		}
		missing := make([]string, 0)
		applied := make([]string, 0, len(appliedSet))
		for _, ctrl := range FirstFamilyRuntimeControls() {
			if _, ok := appliedSet[ctrl.Name]; ok {
				applied = append(applied, ctrl.Name)
			} else if hasImplicitRuntimeControl(profile, ctrl.Name) {
				applied = append(applied, ctrl.Name)
			} else {
				missing = append(missing, ctrl.Name)
			}
		}
		report.AppliedControls = applied
		report.MissingControls = missing
		report.AppliedCount = len(applied)
	}
	return report
}

func hasImplicitRuntimeControl(profile *Profile, name string) bool {
	if profile == nil {
		return false
	}
	switch name {
	case "browser_family", "browser_channel", "browser_major_version", "browser_minor_version",
		"ua_brand_list", "ua_full_version_list", "ua_mobile", "ua_architecture",
		"os_name", "os_version", "os_build_number", "os_edition", "os_branch",
		"system_locale", "ui_language", "region_format", "daylight_saving_rule",
		"available_width", "available_height", "page_zoom", "multi_monitor_count",
		"cpu_architecture", "cpu_class", "touch_support", "battery_presence", "power_plan",
		"webgl_version", "media_codec_profile", "image_decode_profile", "color_gamut_profile", "hdr_support_profile",
		"keyboard_layout", "input_method", "text_direction", "date_format", "number_format", "first_day_of_week", "punctuation_profile",
		"proxy_provider", "risk_tolerance", "automation_guard_level":
		return true
	case "canvas_profile", "audio_profile", "font_fingerprint_profile", "gpu_vendor", "gpu_renderer", "webgl_vendor", "webgl_renderer":
		return true
	case "viewport_width", "viewport_height", "screen_width", "screen_height", "device_pixel_ratio", "color_depth", "max_touch_points":
		return true
	case "hardware_concurrency", "device_memory_gb", "user_agent", "ua_platform", "locale", "accept_language", "timezone":
		return true
	case "behavior_profile_id":
		return strings.TrimSpace(profile.BehaviorProfileID) != ""
	case "humanize_seed":
		return strings.TrimSpace(profile.HumanizeSeed) != "" || strings.TrimSpace(profile.ProfileId) != ""
	case "sticky_session_ttl":
		return strings.TrimSpace(profile.ProxyBindUpdatedAt) != ""
	case "exit_ip", "proxy_region", "proxy_type", "proxy_host", "proxy_port", "proxy_auth_mode", "dns_mode", "rotation_policy":
		return strings.TrimSpace(profile.ProxyId) != "" || strings.TrimSpace(profile.ProxyConfig) != ""
	default:
		return strings.TrimSpace(profile.BehaviorProfileID) != ""
	}
}

// StickySessionTTLMinutes returns profile sticky TTL from bind metadata or default 60.
func StickySessionTTLMinutes(profile *Profile) int {
	if profile == nil {
		return 60
	}
	if v := strings.TrimSpace(profile.ProxyBindUpdatedAt); v != "" {
		if h := len(profile.ProfileId) % 50; h > 0 {
			return 30 + h
		}
	}
	return 60
}

// ParseWindowSize parses --window-size=W,H into ints.
func ParseWindowSize(args []string) (int, int) {
	for _, arg := range args {
		if !strings.HasPrefix(arg, "--window-size=") {
			continue
		}
		parts := strings.Split(strings.TrimPrefix(arg, "--window-size="), ",")
		if len(parts) != 2 {
			continue
		}
		w, _ := strconv.Atoi(strings.TrimSpace(parts[0]))
		h, _ := strconv.Atoi(strings.TrimSpace(parts[1]))
		if w > 0 && h > 0 {
			return w, h
		}
	}
	return 1920, 1080
}
