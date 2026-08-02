package trust

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

// Provider identifies the OAuth/session provider.
type Provider string

const (
	ProviderMicrosoft   Provider = "microsoft"
	ProviderGoogle      Provider = "google"
	ProviderGeneric     Provider = "generic"
	ProviderLocalSession Provider = "local_session"
)

// Bundle is a persisted trust inheritance payload for API-first automation.
type Bundle struct {
	ProfileID    string            `json:"profileId"`
	Provider     Provider          `json:"provider"`
	RefreshToken string            `json:"refreshToken,omitempty"`
	AccessToken  string            `json:"accessToken,omitempty"`
	ExpiresAt    time.Time         `json:"expiresAt,omitempty"`
	Scopes       []string          `json:"scopes,omitempty"`
	CookiesJSON  string            `json:"cookiesJson,omitempty"`
	LocalStorage map[string]string `json:"localStorage,omitempty"`
	SessionStorage map[string]string `json:"sessionStorage,omitempty"`
	ProxyID      string            `json:"proxyId,omitempty"`
	ExitIP       string            `json:"exitIp,omitempty"`
	Notes        string            `json:"notes,omitempty"`
	UpdatedAt    time.Time         `json:"updatedAt"`
}

func (b *Bundle) ValidAccessToken(now time.Time) bool {
	return strings.TrimSpace(b.AccessToken) != "" && (b.ExpiresAt.IsZero() || b.ExpiresAt.After(now))
}

func (b *Bundle) HasRefreshToken() bool {
	return strings.TrimSpace(b.RefreshToken) != ""
}

// RedactedCopy returns a safe copy for API responses.
func (b Bundle) RedactedCopy() Bundle {
	copy := b
	if copy.RefreshToken != "" {
		copy.RefreshToken = redactToken(copy.RefreshToken)
	}
	if copy.AccessToken != "" {
		copy.AccessToken = redactToken(copy.AccessToken)
	}
	return copy
}

func redactToken(raw string) string {
	raw = strings.TrimSpace(raw)
	if len(raw) <= 8 {
		return "***"
	}
	return raw[:4] + "..." + raw[len(raw)-4:]
}

// ParseBundleJSON decodes stored bundle JSON.
func ParseBundleJSON(raw string) (Bundle, error) {
	var b Bundle
	if strings.TrimSpace(raw) == "" {
		return b, fmt.Errorf("empty bundle")
	}
	if err := json.Unmarshal([]byte(raw), &b); err != nil {
		return b, err
	}
	return b, nil
}
