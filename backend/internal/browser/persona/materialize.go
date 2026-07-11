package persona

import (
	"fmt"
	"strings"
)

// MaterializeFingerprintArgs returns fingerprint/launch defaults derived from a persona.
// Caller should only add args that are not already present.
func (p *DevicePersona) MaterializeFingerprintArgs(chromeFullVersion, userAgent string) []string {
	if p == nil {
		return nil
	}
	platform := strings.ToLower(strings.TrimSpace(p.Platform))
	if platform == "" {
		platform = "windows"
	}
	osVer := strings.TrimSpace(p.OSVersion)
	if osVer == "" {
		osVer = "10.0.0"
	}
	args := []string{
		fmt.Sprintf("--fingerprint-platform=%s", platform),
		fmt.Sprintf("--fingerprint-platform-version=%s", osVer),
		fmt.Sprintf("--window-size=%d,%d", p.ScreenWidth, p.ScreenHeight),
		fmt.Sprintf("--force-device-scale-factor=%g", p.DevicePixelRatio),
		fmt.Sprintf("--fingerprint-hardware-concurrency=%d", p.HardwareConcurrency),
	}
	if p.AcceptLang != "" {
		args = append(args, "--accept-lang="+p.AcceptLang)
		lang := strings.Split(p.AcceptLang, ",")[0]
		args = append(args, "--lang="+strings.TrimSpace(lang))
	}
	if p.Timezone != "" {
		args = append(args, "--timezone="+p.Timezone)
	}
	if chromeFullVersion != "" {
		args = append(args, "--fingerprint-brand-version="+chromeFullVersion)
	}
	if userAgent != "" {
		args = append(args, "--user-agent="+userAgent)
	}
	if p.GPUVendor != "" {
		args = append(args, "--fingerprint-webgl-vendor="+p.GPUVendor)
	}
	if p.GPURenderer != "" {
		args = append(args, "--fingerprint-webgl-renderer="+p.GPURenderer)
	}
	return args
}

// ChromiumUA builds a Chrome UA string aligned with the persona platform token.
func (p *DevicePersona) ChromiumUA(fullVersion string) string {
	if p == nil {
		return ""
	}
	token := strings.TrimSpace(p.UAPlatformToken)
	if token == "" {
		token = "Windows NT 10.0; Win64; x64"
	}
	ver := strings.TrimSpace(fullVersion)
	if ver == "" {
		ver = "131.0.0.0"
	}
	return fmt.Sprintf("Mozilla/5.0 (%s) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/%s Safari/537.36", token, ver)
}
