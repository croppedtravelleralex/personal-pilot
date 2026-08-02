package trust

import (
	"encoding/json"
	"strings"
	"time"
)

// TrustSurface is the canonical session inheritance payload for automation.
type TrustSurface struct {
	Cookies        []map[string]interface{} `json:"cookies,omitempty"`
	LocalStorage   map[string]string        `json:"localStorage,omitempty"`
	SessionStorage map[string]string        `json:"sessionStorage,omitempty"`
	RefreshToken   string                   `json:"refreshToken,omitempty"`
	HarvestedAt    time.Time                `json:"harvestedAt,omitempty"`
	Source         string                   `json:"source,omitempty"` // "bundle" | "session" | "harvest"
}

// FromBundle maps an existing Bundle into TrustSurface best-effort.
func FromBundle(b Bundle) TrustSurface {
	s := TrustSurface{
		LocalStorage:   b.LocalStorage,
		SessionStorage: b.SessionStorage,
		RefreshToken:   strings.TrimSpace(b.RefreshToken),
		HarvestedAt:    b.UpdatedAt,
		Source:         "bundle",
	}
	if cookies, err := ParseCookiesJSON(b.CookiesJSON); err == nil && len(cookies) > 0 {
		s.Cookies = cookieEntriesToMaps(cookies)
	}
	return s
}

// HasUsableSession reports whether cookies or a refresh token are present.
func (s TrustSurface) HasUsableSession() bool {
	if strings.TrimSpace(s.RefreshToken) != "" {
		return true
	}
	return len(s.Cookies) > 0
}

func cookieEntriesToMaps(cookies []CookieEntry) []map[string]interface{} {
	out := make([]map[string]interface{}, 0, len(cookies))
	for _, c := range cookies {
		raw, err := json.Marshal(c)
		if err != nil {
			continue
		}
		var m map[string]interface{}
		if err := json.Unmarshal(raw, &m); err != nil {
			continue
		}
		out = append(out, m)
	}
	return out
}
