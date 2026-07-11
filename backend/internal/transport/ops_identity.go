package transport

import "strings"

const ProductUserAgent = "PersonalPilot/1.1"

// BrowserLikeUserAgent returns profileUA when set, otherwise ProductUserAgent.
func BrowserLikeUserAgent(profileUA string) string {
	if strings.TrimSpace(profileUA) != "" {
		return strings.TrimSpace(profileUA)
	}
	return ProductUserAgent
}
