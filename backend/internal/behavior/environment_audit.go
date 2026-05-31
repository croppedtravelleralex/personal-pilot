package behavior

import "sort"

type EnvironmentFamilyAudit struct {
	Family         string   `json:"family"`
	TargetCount    int      `json:"targetCount"`
	SupportedCount int      `json:"supportedCount"`
	ObservedCount  int      `json:"observedCount"`
	SupportedKeys  []string `json:"supportedKeys"`
	ObservedKeys   []string `json:"observedKeys"`
	Status         string   `json:"status"`
}

type EnvironmentInjectionAudit struct {
	Families []EnvironmentFamilyAudit `json:"families"`
	Status   string                   `json:"status"`
}

func AuditEnvironmentInjection(profile EnvironmentInjectionProfile, observed map[string]bool) EnvironmentInjectionAudit {
	families := []EnvironmentFamilyAudit{
		auditFamily("browser_api_surface", 44, browserAPISupportedKeys(profile), observed),
		auditFamily("canvas_rendering", 38, []string{"toDataURL", "getImageData", "fillText", "seeded_noise"}, observed),
		auditFamily("timezone_locale", 34, timezoneLocaleSupportedKeys(profile), observed),
		auditFamily("webgl_gpu", 42, webGLSupportedKeys(profile), observed),
		auditFamily("audio_stack", 34, audioSupportedKeys(profile), observed),
		auditFamily("fonts_text_metrics", 34, fontSupportedKeys(profile), observed),
		auditFamily("media_devices", 30, mediaDeviceSupportedKeys(profile), observed),
		auditFamily("webrtc_ip_leak", 36, webRTCSupportedKeys(profile), observed),
	}
	status := "supported_pending_observed"
	allObserved := true
	for _, family := range families {
		if family.Status != "observed" {
			allObserved = false
		}
	}
	if allObserved {
		status = "observed"
	}
	return EnvironmentInjectionAudit{Families: families, Status: status}
}

func auditFamily(name string, targetCount int, supported []string, observed map[string]bool) EnvironmentFamilyAudit {
	sort.Strings(supported)
	var observedKeys []string
	for _, key := range supported {
		if observed[name+":"+key] || observed[key] {
			observedKeys = append(observedKeys, key)
		}
	}
	status := "partial_supported"
	if len(supported) >= targetCount {
		status = "supported"
	}
	if len(observedKeys) >= targetCount {
		status = "observed"
	} else if len(observedKeys) > 0 {
		status = "partially_observed"
	}
	return EnvironmentFamilyAudit{Family: name, TargetCount: targetCount, SupportedCount: len(supported), ObservedCount: len(observedKeys), SupportedKeys: supported, ObservedKeys: observedKeys, Status: status}
}

func browserAPISupportedKeys(profile EnvironmentInjectionProfile) []string {
	keys := []string{"navigator.webdriver", "navigator.languages", "navigator.platform", "navigator.vendor", "navigator.userAgent", "navigator.hardwareConcurrency", "navigator.deviceMemory", "navigator.plugins", "navigator.mimeTypes"}
	if len(profile.MediaDevices) > 0 {
		keys = append(keys, "navigator.mediaDevices.enumerateDevices")
	}
	if profile.MediaPermission != "" {
		keys = append(keys, "navigator.mediaDevices.getUserMedia")
	}
	return keys
}

func timezoneLocaleSupportedKeys(profile EnvironmentInjectionProfile) []string {
	keys := []string{"Intl.DateTimeFormat.resolvedOptions", "Date.getTimezoneOffset", "Accept-Language"}
	if len(profile.Languages) > 0 {
		keys = append(keys, "navigator.languages.locale")
	}
	if profile.Timezone != "" {
		keys = append(keys, "timezone.id")
	}
	return keys
}

func webGLSupportedKeys(profile EnvironmentInjectionProfile) []string {
	keys := []string{"getParameter.UNMASKED_VENDOR_WEBGL", "getParameter.UNMASKED_RENDERER_WEBGL", "getSupportedExtensions.filter"}
	if profile.WebGLVendor != "" {
		keys = append(keys, "webgl.vendor")
	}
	if profile.WebGLRenderer != "" {
		keys = append(keys, "webgl.renderer")
	}
	if len(profile.WebGLExtensions) > 0 {
		keys = append(keys, "webgl.extensions")
	}
	return keys
}

func audioSupportedKeys(profile EnvironmentInjectionProfile) []string {
	keys := []string{"AnalyserNode.getFloatFrequencyData", "AudioBuffer.copyFromChannel"}
	if profile.AudioNoise != 0 {
		keys = append(keys, "audio.noise")
	}
	return keys
}

func fontSupportedKeys(profile EnvironmentInjectionProfile) []string {
	keys := []string{"Document.fonts.check", "Document.fonts.iterator", "window.__personaPilotAllowedFonts"}
	if len(profile.FontAllowlist) > 0 {
		keys = append(keys, "font.allowlist")
	}
	return keys
}

func mediaDeviceSupportedKeys(profile EnvironmentInjectionProfile) []string {
	keys := []string{"navigator.mediaDevices.enumerateDevices", "navigator.mediaDevices.getUserMedia"}
	if len(profile.MediaDevices) > 0 {
		keys = append(keys, "media.devices")
	}
	if profile.MediaPermission != "" {
		keys = append(keys, "media.permission")
	}
	return keys
}

func webRTCSupportedKeys(profile EnvironmentInjectionProfile) []string {
	keys := []string{"RTCPeerConnection.createOffer", "RTCPeerConnection.setLocalDescription", "RTCPeerConnection.icecandidate.filter"}
	if profile.WebRTCPolicy.Mode != "" {
		keys = append(keys, "webrtc.policy")
	}
	if len(profile.WebRTCPolicy.AllowHosts) > 0 {
		keys = append(keys, "webrtc.allowHosts")
	}
	return keys
}
