package browser

import "strings"

// StrictAuthSiteIDs are behavior profile / tag hints for enterprise auth flows.
var StrictAuthSiteIDs = []string{"auth", "outlook", "microsoft", "claude", "oauth", "enterprise"}

// ApplyStrictAuthPreset augments materialized args for high-friction login surfaces.
func ApplyStrictAuthPreset(profile *Profile, fingerprintArgs, launchArgs []string) ([]string, []string) {
	version := ResolveChromiumVersion(nil, "", profile, fingerprintArgs, launchArgs)
	return applyStrictAuthPresetWithVersion(profile, fingerprintArgs, launchArgs, version)
}

func applyStrictAuthPresetWithVersion(profile *Profile, fingerprintArgs, launchArgs []string, version CoreVersionInfo) ([]string, []string) {
	if !profileNeedsStrictAuthPreset(profile) {
		return fingerprintArgs, launchArgs
	}
	fingerprintArgs = stripLaunchArgPrefixes(fingerprintArgs, "--lang", "--accept-lang", "--timezone", "--user-agent", "--fingerprint-brand-version")
	launchArgs = stripLaunchArgPrefixes(launchArgs, "--lang", "--accept-lang", "--timezone", "--user-agent")
	has := indexLaunchArgs(append(append([]string{}, fingerprintArgs...), launchArgs...))
	addFP := func(arg string) {
		key := arg
		if idx := strings.Index(arg, "="); idx > 0 {
			key = arg[:idx]
		}
		if has[key] {
			return
		}
		has[key] = true
		fingerprintArgs = append(fingerprintArgs, arg)
	}
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

	// Anti-automation surface reduction (Chromium).
	addLaunch("--disable-blink-features=AutomationControlled")
	addLaunch("--disable-infobars")
	addLaunch("--no-first-run")
	addLaunch("--no-default-browser-check")
	addLaunch("--disable-popup-blocking")

	// Locale/timezone coherence for OAuth (prefer en-US unless profile already set).
	if !has["--lang"] {
		addFP("--lang=en-US")
	}
	if !has["--accept-lang"] {
		addFP("--accept-lang=en-US,en;q=0.9")
	}
	if !has["--timezone"] {
		addFP("--timezone=America/New_York")
	}

	// Client hints / platform consistency.
	addFP("--fingerprint-brand=Chrome")
	addFP("--fingerprint-platform=Windows")
	addFP("--fingerprint-platform-version=15.0.0")
	addFP("--fingerprint-brand-version=" + version.Full)
	if !has["--user-agent"] {
		addFP("--user-agent=" + ChromiumUserAgent(version))
	}

	// Pointer realism for headed auth flows (opt-in via show-mouse-pointer tag).
	addFP("--fingerprint-touch-points=0")
	addFP("--fingerprint-color-depth=24")
	addFP("--fingerprint-hardware-concurrency=8")
	addFP("--fingerprint-device-memory=8")

	return fingerprintArgs, launchArgs
}

func profileNeedsStrictAuthPreset(profile *Profile) bool {
	if profile == nil {
		return false
	}
	blob := strings.ToLower(strings.Join(append([]string{
		profile.BehaviorProfileID,
		profile.ProfileName,
		strings.Join(profile.Tags, " "),
		strings.Join(profile.Keywords, " "),
	}, profile.GroupId), " "))
	for _, hint := range StrictAuthSiteIDs {
		if strings.Contains(blob, hint) {
			return true
		}
	}
	for _, arg := range append(append([]string{}, profile.LaunchArgs...), profile.FingerprintArgs...) {
		if strings.Contains(strings.ToLower(arg), "strict-auth") {
			return true
		}
	}
	return false
}

func stripLaunchArgPrefixes(args []string, prefixes ...string) []string {
	out := make([]string, 0, len(args))
	for _, arg := range args {
		keep := true
		trimmed := strings.TrimSpace(arg)
		for _, prefix := range prefixes {
			if trimmed == prefix || strings.HasPrefix(trimmed, prefix+"=") {
				keep = false
				break
			}
		}
		if keep {
			out = append(out, arg)
		}
	}
	return out
}
