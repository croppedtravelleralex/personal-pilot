package browser

import "strings"

// chromiumOnlyLaunchPrefixes are Chrome/fingerprint-chromium flags that must not
// be passed to Camoufox/Firefox.
var chromiumOnlyLaunchPrefixes = []string{
	"--fingerprint",
	"--disable-blink-features",
	"--host-resolver-rules",
	"--personal-pilot-",
	"--force-device-scale-factor",
	"--disable-infobars",
	"--webrtc-ip-handling-policy",
	"--disable-features=IsolateOrigins",
	"--disable-site-isolation-trials",
}

// FilterLaunchArgsForCamoufox removes Chromium-only launch flags.
func FilterLaunchArgsForCamoufox(args []string) []string {
	if len(args) == 0 {
		return nil
	}
	out := make([]string, 0, len(args))
	for _, arg := range args {
		trimmed := strings.TrimSpace(arg)
		if trimmed == "" {
			continue
		}
		if isChromiumOnlyLaunchArg(trimmed) {
			continue
		}
		out = append(out, trimmed)
	}
	return out
}

func isChromiumOnlyLaunchArg(arg string) bool {
	lower := strings.ToLower(arg)
	for _, prefix := range chromiumOnlyLaunchPrefixes {
		if strings.HasPrefix(lower, prefix) {
			return true
		}
	}
	// Chromium-style user-data-dir; Camoufox uses --profile instead (BuildCamoufoxLaunchArgs).
	if strings.HasPrefix(lower, "--user-data-dir=") {
		return true
	}
	return false
}

// MaterializeRuntimeArgsForCore applies MaterializeRuntimeArgs for Chromium-family
// cores and returns Camoufox-safe launch args only for Camoufox.
func MaterializeRuntimeArgsForCore(coreKind string, profile *Profile, globalFingerprint, globalLaunch []string) (fingerprintArgs, launchArgs []string) {
	return MaterializeRuntimeArgsForCoreBinary(coreKind, "", profile, globalFingerprint, globalLaunch)
}

// MaterializeRuntimeArgsForCoreBinary keeps the selected binary version aligned
// with the UA and UA-CH related launch controls.
func MaterializeRuntimeArgsForCoreBinary(coreKind, binaryPath string, profile *Profile, globalFingerprint, globalLaunch []string) (fingerprintArgs, launchArgs []string) {
	if IsCamoufoxKind(coreKind) {
		launchArgs = append([]string{}, profile.LaunchArgs...)
		launchArgs = append(launchArgs, globalLaunch...)
		launchArgs = FilterLaunchArgsForCamoufox(launchArgs)
		has := indexLaunchArgs(launchArgs)
		add := func(arg string) {
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
		add("--no-first-run")
		add("--no-default-browser-check")
		return nil, launchArgs
	}
	return MaterializeRuntimeArgsWithBinary(binaryPath, profile, globalFingerprint, globalLaunch)
}

// AppendProxyHardeningArgsForCore adds proxy hardening flags appropriate for the core kind.
func AppendProxyHardeningArgsForCore(coreKind string, args []string, proxyServer string) []string {
	if IsCamoufoxKind(coreKind) {
		return args
	}
	return AppendProxyHardeningArgs(args, proxyServer)
}

// MergeCoreLaunchArgs appends profile launch/fingerprint args for the given core kind.
func MergeCoreLaunchArgs(coreKind string, baseArgs, fingerprintArgs, profileLaunchArgs, extraLaunchArgs []string) []string {
	args := append([]string{}, baseArgs...)
	if IsCamoufoxKind(coreKind) {
		args = append(args, FilterLaunchArgsForCamoufox(profileLaunchArgs)...)
		args = append(args, FilterLaunchArgsForCamoufox(extraLaunchArgs)...)
		return args
	}
	args = append(args, fingerprintArgs...)
	args = append(args, profileLaunchArgs...)
	args = append(args, extraLaunchArgs...)
	return args
}
