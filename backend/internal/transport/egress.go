package transport

import (
	"fmt"
	"strconv"
	"strings"
)

// DefaultChromeMajor is the product Chromium major used when no UA is available
// at the local proxy-bridge layer (aligned with browser.DefaultChromiumVersion).
const DefaultChromeMajor = 139

// EgressIdentity is the shared outbound identity for browser args and API clients.
type EgressIdentity struct {
	UAMajor     int
	FullUA      string
	AcceptLang  string
	ClientHello string // e.g. "Chrome_Auto"
}

// ChromeHelloPreset maps a Chromium major to a utls ClientHello preset name.
func ChromeHelloPreset(major int) string {
	switch {
	case major >= 133:
		return "Chrome_133"
	case major >= 131:
		return "Chrome_131"
	case major >= 120:
		return "Chrome_120"
	default:
		return "Chrome_Auto"
	}
}

// ChromeMajorTLSBaseline returns outbound TLS metadata aligned to a Chrome major (docs/49 C4).
// JA3 field is a template label for config selection — not an observed JA3 hash.
func ChromeMajorTLSBaseline(major int) OutboundConfig {
	if major <= 0 {
		major = DefaultChromeMajor
	}
	cfg := RuntimeProfile(RuntimeFamilyChrome)
	preset := ChromeHelloPreset(major)
	cfg.TLS.JA3 = preset
	cfg.RouteTag = fmt.Sprintf("chrome-%d", major)
	return cfg
}

// TLSUACoherent reports whether a User-Agent major maps to the same TLS template label.
func TLSUACoherent(ua string, tlsTemplate string) bool {
	major := ParseUAMajor(ua)
	if major <= 0 {
		major = DefaultChromeMajor
	}
	expected := ChromeHelloPreset(major)
	return strings.EqualFold(strings.TrimSpace(tlsTemplate), expected)
}

// ParseUAMajor extracts the Chrome/Chromium major from a User-Agent string.
func ParseUAMajor(ua string) int {
	ua = strings.TrimSpace(ua)
	for _, marker := range []string{"Chrome/", "Chromium/"} {
		if i := strings.Index(ua, marker); i >= 0 {
			rest := ua[i+len(marker):]
			end := 0
			for end < len(rest) && rest[end] >= '0' && rest[end] <= '9' {
				end++
			}
			if end > 0 {
				n, _ := strconv.Atoi(rest[:end])
				return n
			}
		}
	}
	return 0
}

// EgressFromUserAgent builds an egress identity from a browser-like UA.
func EgressFromUserAgent(ua, acceptLang string) EgressIdentity {
	ua = BrowserLikeUserAgent(ua)
	major := ParseUAMajor(ua)
	return EgressIdentity{
		UAMajor:     major,
		FullUA:      ua,
		AcceptLang:  strings.TrimSpace(acceptLang),
		ClientHello: ChromeHelloPreset(major),
	}
}
