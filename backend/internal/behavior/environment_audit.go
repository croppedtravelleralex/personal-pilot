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
