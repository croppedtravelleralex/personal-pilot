package trust

import (
	"encoding/json"
	"fmt"
	"strings"
)

// CookieEntry is one browser cookie for CDP Network.setCookie.
type CookieEntry struct {
	Name     string  `json:"name"`
	Value    string  `json:"value"`
	Domain   string  `json:"domain"`
	Path     string  `json:"path,omitempty"`
	Secure   bool    `json:"secure,omitempty"`
	HTTPOnly bool    `json:"httpOnly,omitempty"`
	SameSite string  `json:"sameSite,omitempty"`
	Expires  float64 `json:"expires,omitempty"`
}

// ParseCookiesJSON decodes cookies array from bundle storage.
func ParseCookiesJSON(raw string) ([]CookieEntry, error) {
	raw = trim(raw)
	if raw == "" {
		return nil, nil
	}
	var cookies []CookieEntry
	if err := json.Unmarshal([]byte(raw), &cookies); err == nil {
		return cookies, nil
	}
	var wrapper struct {
		Cookies []CookieEntry `json:"cookies"`
	}
	if err := json.Unmarshal([]byte(raw), &wrapper); err != nil {
		return nil, fmt.Errorf("invalid cookies json")
	}
	return wrapper.Cookies, nil
}

func trim(s string) string {
	return strings.TrimSpace(s)
}
