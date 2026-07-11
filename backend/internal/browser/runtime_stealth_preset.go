package browser

import "strings"

// ApplyStealthAutopilotPreset adds anti-automation launch args for auto-99 tagged profiles.
func ApplyStealthAutopilotPreset(profile *Profile, fingerprintArgs, launchArgs []string) ([]string, []string) {
	if !profileHasTag(profile, "auto-99") {
		return fingerprintArgs, launchArgs
	}
	has := indexLaunchArgs(append(append([]string{}, fingerprintArgs...), launchArgs...))
	addLaunch := func(arg string) {
		key := arg
		if idx := strings.Index(arg, "="); idx > 0 {
			key = arg[:idx]
		}
		if has[key] {
			return
		}
		has[key] = true
		launchArgs = append(launchArgs, arg)
	}
	addLaunch("--disable-blink-features=AutomationControlled")
	addLaunch("--disable-infobars")
	addLaunch("--disable-popup-blocking")
	return fingerprintArgs, launchArgs
}
