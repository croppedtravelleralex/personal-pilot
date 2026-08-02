package backend

import (
	"fmt"
	"strconv"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/events"
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

	ownershipReady := false
	for attempt := 1; attempt <= 40; attempt++ {
		if err := a.validateProfileCDPOwnership(snapshot); err == nil {
			ownershipReady = true
			break
		}
		if attempt == 40 {
			logger.New("Browser").Warn("Skip environment injection because CDP ownership is not proven after retries",
				logger.F("profile_id", profileID),
				logger.F("debug_port", debugPort),
			)
			a.markProfileInjectionReady(profileID)
			return
		}
		time.Sleep(time.Duration(150*attempt) * time.Millisecond)
		a.browserMgr.Mutex.Lock()
		profile := a.browserMgr.Profiles[profileID]
		if profile == nil || !profile.Running {
			a.browserMgr.Mutex.Unlock()
			return
		}
		copied := *profile
		snapshot = &copied
		a.browserMgr.Mutex.Unlock()
	}
	if !ownershipReady {
		return
	}
	injectionKey := profileEnvironmentInjectionKey(snapshot, debugPort)
	if !a.claimProfileEnvironmentInjection(injectionKey) {
		a.markProfileInjectionReady(profileID)
		return
	}
	succeeded := false
	defer func() {
		if !succeeded {
			a.releaseProfileEnvironmentInjection(injectionKey)
		}
	}()

	profileForInjection := buildEnvironmentInjectionProfile(snapshot)
	if exitIP := a.lookupProfileProxyExitIP(snapshot); exitIP != "" {
		profileForInjection.WebRTCPolicy.AllowHosts = []string{exitIP}
	}
	var lastErr error
	for attempt := 1; attempt <= 20; attempt++ {
		executor, err := connectCDPExecutor(debugPort)
		if err == nil {
			plan, applyErr := executor.ApplyEnvironmentInjection(profileForInjection)
			if applyErr == nil {
				country, city := a.getProxyCachedGeo(snapshot.ProxyId)
				if coord, ok := browser.LookupGeoCoordinate(country, city); ok {
					_ = executor.SetGeolocationOverride(coord.Lat, coord.Lon, coord.Accuracy)
				}
			}
			_ = executor.Close()
			if applyErr == nil {
				logger.New("Browser").Info("环境注入已接入实例启动流程",
					logger.F("profile_id", profileID),
					logger.F("debug_port", debugPort),
					logger.F("families", strings.Join(plan.AppliedFamilies, ",")),
				)
				succeeded = true
				a.markProfileInjectionReady(profileID)
				go a.probeWebRTCAfterInjection(profileID, debugPort)
				go a.injectTrustCookiesAsync(profileID, debugPort)
				return
			}
			lastErr = applyErr
		} else {
			lastErr = err
		}
		time.Sleep(time.Duration(150*attempt) * time.Millisecond)
	}
	if lastErr != nil {
		logger.New("Browser").Error("环境注入失败，实例可能缺少反泄漏 JS hook",
			logger.F("profile_id", profileID),
			logger.F("debug_port", debugPort),
			logger.F("error", lastErr.Error()),
		)
		if a.ctx != nil {
			a.emit(events.EventRiskWebRTCLeak, map[string]interface{}{
				"profileId": profileID,
				"reason":    "environment_injection_failed",
				"error":     lastErr.Error(),
			})
		}
		a.markProfileInjectionReady(profileID)
	}
}

func (a *App) lookupProfileProxyExitIP(profile *BrowserProfile) string {
	if a == nil || profile == nil || strings.TrimSpace(profile.ProxyId) == "" {
		return ""
	}
	return strings.TrimSpace(a.lookupProxyStoredExitIP(profile.ProxyId))
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

func profileInjectionNotReadyError(profile *BrowserProfile) error {
	if profile == nil || profile.InjectionReady {
		return nil
	}
	return fmt.Errorf("环境注入未就绪: %s", profile.ProfileName)
}

func (a *App) markProfileInjectionReady(profileID string) {
	if a == nil || a.browserMgr == nil || strings.TrimSpace(profileID) == "" {
		return
	}
	a.browserMgr.Mutex.Lock()
	profile, ok := a.browserMgr.Profiles[profileID]
	if ok && profile != nil && !profile.InjectionReady {
		profile.InjectionReady = true
		snapshot := copyBrowserProfileSnapshot(profile)
		a.browserMgr.Mutex.Unlock()
		if snapshot != nil && a.ctx != nil {
			a.emit(events.EventBrowserInstanceUpdated, browserInstanceEventPayload(snapshot, false))
		}
		return
	}
	a.browserMgr.Mutex.Unlock()
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
		case "--fingerprint-brand-version":
			injection.BrandVersion = value
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
		case "--webrtc-ip-handling-policy":
			injection.WebRTCPolicy = mapWebRTCPolicy(value)
		case "--fingerprint-touch-points":
			injection.MaxTouchPoints = parsePositiveInt(value)
		case "--fingerprint-color-depth":
			injection.ColorDepth = parsePositiveInt(value)
		case "--fingerprint-do-not-track":
			injection.DoNotTrack = strings.EqualFold(value, "true") || value == "1"
		case "--force-device-scale-factor":
			injection.DevicePixelRatio = value
		case "--window-size":
			injection.WindowSize = value
			if w, h, ok := parseWindowSizeDimensions(value); ok {
				injection.ScreenWidth = w
				injection.ScreenHeight = h
			}
		}
	}

	if len(injection.Languages) == 0 {
		injection.Languages = []string{"zh-CN", "zh", "en"}
	}
	if injection.AcceptLanguage == "" {
		injection.AcceptLanguage = "zh-CN,zh;q=0.9,en;q=0.8"
	}
	if injection.DeviceMemory <= 0 {
		injection.DeviceMemory = 8
	}
	if injection.HardwareConcurrency <= 0 {
		injection.HardwareConcurrency = 8
	}
	if strings.TrimSpace(injection.BrandVersion) == "" {
		if ver := browser.ResolveChromiumVersion(nil, "", profile); strings.TrimSpace(ver.Full) != "" {
			injection.BrandVersion = ver.Full
		}
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
		injection.Plugins = defaultEnvironmentPDFPlugins()
	}
	if len(injection.MediaDevices) == 0 {
		injection.MediaDevices = []behavior.MediaDeviceSpec{
			{Kind: "audioinput", Label: "Microphone", GroupID: stableShortID(seed, "audioinput-group"), ID: stableShortID(seed, "audioinput")},
			{Kind: "audiooutput", Label: "Speaker", GroupID: stableShortID(seed, "audiooutput-group"), ID: stableShortID(seed, "audiooutput")},
		}
	}
	return injection
}

func mapWebRTCPolicy(value string) behavior.WebRTCPolicy {
	switch strings.TrimSpace(strings.ToLower(value)) {
	case "disable_non_proxied_udp", "default_public_interface_only", "default_public_and_private_interfaces":
		return behavior.WebRTCPolicy{Mode: "block_private_candidates"}
	case "allow_all":
		return behavior.WebRTCPolicy{Mode: "allow_all"}
	default:
		return behavior.WebRTCPolicy{Mode: "block_private_candidates"}
	}
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

func parseWindowSizeDimensions(raw string) (int, int, bool) {
	parts := strings.Split(strings.TrimSpace(raw), ",")
	if len(parts) != 2 {
		return 0, 0, false
	}
	w := parsePositiveInt(parts[0])
	h := parsePositiveInt(parts[1])
	if w <= 0 || h <= 0 {
		return 0, 0, false
	}
	return w, h, true
}

func defaultEnvironmentPDFPlugins() []behavior.PluginInjection {
	return []behavior.PluginInjection{
		{Name: "PDF Viewer", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
		{Name: "Chrome PDF Viewer", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
		{Name: "Chromium PDF Viewer", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
		{Name: "Microsoft Edge PDF Viewer", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
		{Name: "WebKit built-in PDF", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
	}
}
