package backend

import (
	"fmt"
	"strconv"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/logger"
)

func (a *App) applyProfileEnvironmentInjectionAsync(profileID string, debugPort int) {
	if a == nil || a.browserMgr == nil || debugPort <= 0 {
		return
	}

	a.browserMgr.Mutex.Lock()
	profile := a.browserMgr.Profiles[profileID]
	var snapshot *BrowserProfile
	if profile != nil {
		copied := *profile
		snapshot = &copied
	}
	a.browserMgr.Mutex.Unlock()

	if snapshot == nil {
		return
	}
	if err := a.validateProfileCDPOwnership(snapshot); err != nil {
		logger.New("Browser").Warn("Skip environment injection because CDP ownership is not proven",
			logger.F("profile_id", profileID),
			logger.F("debug_port", debugPort),
			logger.F("error", err.Error()),
		)
		return
	}
	injectionKey := profileEnvironmentInjectionKey(snapshot, debugPort)
	if !a.claimProfileEnvironmentInjection(injectionKey) {
		return
	}
	succeeded := false
	defer func() {
		if !succeeded {
			a.releaseProfileEnvironmentInjection(injectionKey)
		}
	}()

	profileForInjection := buildEnvironmentInjectionProfile(snapshot)
	var lastErr error
	for attempt := 1; attempt <= 8; attempt++ {
		executor, err := connectCDPExecutor(debugPort)
		if err == nil {
			plan, applyErr := executor.ApplyEnvironmentInjection(profileForInjection)
			_ = executor.Close()
			if applyErr == nil {
				logger.New("Browser").Info("环境注入已接入实例启动流程",
					logger.F("profile_id", profileID),
					logger.F("debug_port", debugPort),
					logger.F("families", strings.Join(plan.AppliedFamilies, ",")),
				)
				succeeded = true
				return
			}
			lastErr = applyErr
		} else {
			lastErr = err
		}
		time.Sleep(time.Duration(150*attempt) * time.Millisecond)
	}
	if lastErr != nil {
		logger.New("Browser").Warn("环境注入失败，实例继续运行但当前页面可能缺少 JS hook",
			logger.F("profile_id", profileID),
			logger.F("debug_port", debugPort),
			logger.F("error", lastErr.Error()),
		)
	}
}

func profileEnvironmentInjectionKey(profile *BrowserProfile, debugPort int) string {
	if profile == nil {
		return ""
	}
	timestamp := ""
	if profile.LaunchAudit != nil {
		timestamp = profile.LaunchAudit.Timestamp
	}
	return fmt.Sprintf("%s:%d:%d:%s", strings.TrimSpace(profile.ProfileId), debugPort, profile.Pid, timestamp)
}

func (a *App) claimProfileEnvironmentInjection(key string) bool {
	if a == nil || strings.TrimSpace(key) == "" {
		return false
	}
	a.envInjectionMu.Lock()
	defer a.envInjectionMu.Unlock()
	if a.envInjectionKeys == nil {
		a.envInjectionKeys = make(map[string]struct{})
	}
	if _, exists := a.envInjectionKeys[key]; exists {
		return false
	}
	a.envInjectionKeys[key] = struct{}{}
	return true
}

func (a *App) releaseProfileEnvironmentInjection(key string) {
	if a == nil || strings.TrimSpace(key) == "" {
		return
	}
	a.envInjectionMu.Lock()
	defer a.envInjectionMu.Unlock()
	delete(a.envInjectionKeys, key)
}

func (a *App) clearProfileEnvironmentInjections(profileID string) {
	if a == nil || strings.TrimSpace(profileID) == "" {
		return
	}
	prefix := strings.TrimSpace(profileID) + ":"
	a.envInjectionMu.Lock()
	defer a.envInjectionMu.Unlock()
	for key := range a.envInjectionKeys {
		if strings.HasPrefix(key, prefix) {
			delete(a.envInjectionKeys, key)
		}
	}
}

func buildEnvironmentInjectionProfile(profile *BrowserProfile) behavior.EnvironmentInjectionProfile {
	seed := strings.TrimSpace(profile.HumanizeSeed)
	if seed == "" {
		seed = strings.TrimSpace(profile.ProfileId)
	}
	injection := behavior.EnvironmentInjectionProfile{
		Seed:            seed,
		Platform:        "Win32",
		Vendor:          "Google Inc.",
		WebRTCPolicy:    behavior.WebRTCPolicy{Mode: "block_private_candidates"},
		MediaPermission: "prompt",
	}

	for _, arg := range append(append([]string{}, profile.FingerprintArgs...), profile.LaunchArgs...) {
		key, value, ok := splitLaunchFlag(arg)
		if !ok {
			continue
		}
		switch key {
		case "--fingerprint-platform":
			injection.Platform = platformToNavigatorValue(value)
		case "--user-agent":
			injection.UserAgent = value
		case "--fingerprint-hardware-concurrency":
			injection.HardwareConcurrency = parsePositiveInt(value)
		case "--fingerprint-device-memory":
			injection.DeviceMemory = parsePositiveInt(value)
		case "--timezone", "--timezone-for-testing":
			injection.Timezone = value
			if offset, ok := timezoneOffsetMinutes(value); ok {
				injection.TimezoneOffset = &offset
			}
		case "--lang", "--accept-lang", "--accept-language":
			injection.AcceptLanguage = value
			injection.Languages = parseAcceptLanguages(value)
		case "--fingerprint-webgl-vendor":
			injection.WebGLVendor = value
		case "--fingerprint-webgl-renderer":
			injection.WebGLRenderer = value
		case "--fingerprint-audio-noise":
			injection.AudioNoise = parseAudioNoise(value)
		case "--fingerprint-fonts":
			injection.FontAllowlist = splitCSV(value)
		}
	}

	if len(injection.Languages) == 0 {
		injection.Languages = []string{"zh-CN", "zh", "en"}
	}
	if injection.AcceptLanguage == "" {
		injection.AcceptLanguage = "zh-CN,zh;q=0.9,en;q=0.8"
	}
	if injection.WebGLVendor == "" {
		injection.WebGLVendor = "Google Inc."
	}
	if injection.WebGLRenderer == "" {
		injection.WebGLRenderer = "ANGLE (Intel, Intel(R) UHD Graphics Direct3D11 vs_5_0 ps_5_0)"
	}
	if len(injection.WebGLExtensions) == 0 {
		injection.WebGLExtensions = []string{"ANGLE_instanced_arrays", "EXT_blend_minmax", "EXT_color_buffer_half_float", "OES_element_index_uint", "OES_standard_derivatives", "WEBGL_debug_renderer_info"}
	}
	if len(injection.Plugins) == 0 {
		injection.Plugins = []behavior.PluginInjection{{
			Name:        "PDF Viewer",
			Filename:    "internal-pdf-viewer",
			Description: "Portable Document Format",
			MimeType:    "application/pdf",
		}}
	}
	if len(injection.MediaDevices) == 0 {
		injection.MediaDevices = []behavior.MediaDeviceSpec{
			{Kind: "audioinput", Label: "Microphone", GroupID: stableShortID(seed, "audioinput-group"), ID: stableShortID(seed, "audioinput")},
			{Kind: "audiooutput", Label: "Speaker", GroupID: stableShortID(seed, "audiooutput-group"), ID: stableShortID(seed, "audiooutput")},
		}
	}
	return injection
}

func splitLaunchFlag(arg string) (string, string, bool) {
	arg = strings.TrimSpace(arg)
	idx := strings.Index(arg, "=")
	if idx <= 0 {
		return "", "", false
	}
	return strings.TrimSpace(arg[:idx]), strings.TrimSpace(arg[idx+1:]), true
}

func parsePositiveInt(value string) int {
	n, err := strconv.Atoi(strings.TrimSpace(value))
	if err != nil || n < 0 {
		return 0
	}
	return n
}

func parseAudioNoise(value string) float64 {
	value = strings.TrimSpace(strings.ToLower(value))
	if value == "true" || value == "1" || value == "yes" || value == "on" {
		return 0.001
	}
	parsed, err := strconv.ParseFloat(value, 64)
	if err != nil {
		return 0
	}
	return parsed
}

func parseAcceptLanguages(value string) []string {
	items := strings.Split(value, ",")
	out := make([]string, 0, len(items))
	seen := make(map[string]struct{})
	for _, item := range items {
		lang := strings.TrimSpace(strings.Split(item, ";")[0])
		if lang == "" {
			continue
		}
		if _, ok := seen[lang]; ok {
			continue
		}
		seen[lang] = struct{}{}
		out = append(out, lang)
	}
	return out
}

func splitCSV(value string) []string {
	parts := strings.FieldsFunc(value, func(r rune) bool { return r == ',' || r == ';' || r == '|' })
	out := make([]string, 0, len(parts))
	for _, part := range parts {
		if v := strings.TrimSpace(part); v != "" {
			out = append(out, v)
		}
	}
	return out
}

func platformToNavigatorValue(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "windows", "win":
		return "Win32"
	case "mac", "macos", "darwin":
		return "MacIntel"
	case "linux":
		return "Linux x86_64"
	case "android":
		return "Linux armv8l"
	default:
		if strings.TrimSpace(value) == "" {
			return "Win32"
		}
		return value
	}
}

func timezoneOffsetMinutes(timezone string) (int, bool) {
	switch strings.TrimSpace(timezone) {
	case "Asia/Shanghai", "Asia/Chongqing", "Asia/Hong_Kong", "Asia/Singapore", "Asia/Taipei":
		return -480, true
	case "Asia/Tokyo", "Asia/Seoul":
		return -540, true
	case "UTC", "Etc/UTC":
		return 0, true
	case "America/New_York":
		return 300, true
	case "America/Los_Angeles":
		return 480, true
	case "Europe/London":
		return 0, true
	case "Europe/Berlin", "Europe/Paris":
		return -60, true
	default:
		return 0, false
	}
}

func stableShortID(seed string, label string) string {
	return fmt.Sprintf("pp-%x", browser.HashLaunchAuditValue(seed+":"+label))[:18]
}
