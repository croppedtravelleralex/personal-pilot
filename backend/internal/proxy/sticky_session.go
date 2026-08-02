package proxy

import (
	"sync"
	"time"
)

// StickySessionTracker enforces sticky session TTL for profile-proxy pairs.
type StickySessionTracker struct {
	mu       sync.Mutex
	sessions map[string]stickySession
}

type stickySession struct {
	ProfileID string
	ProxyID   string
	ExitIP    string
	ExpiresAt time.Time
	TTL       time.Duration
}

// NewStickySessionTracker creates an empty tracker.
func NewStickySessionTracker() *StickySessionTracker {
	return &StickySessionTracker{sessions: make(map[string]stickySession)}
}

// Bind records or refreshes a sticky session for profileID.
func (t *StickySessionTracker) Bind(profileID, proxyID, exitIP string, ttl time.Duration) {
	if t == nil || profileID == "" || ttl <= 0 {
		return
	}
	if ttl < 10*time.Minute {
		ttl = 10 * time.Minute
	}
	t.mu.Lock()
	defer t.mu.Unlock()
	t.sessions[profileID] = stickySession{
		ProfileID: profileID,
		ProxyID:   proxyID,
		ExitIP:    exitIP,
		ExpiresAt: time.Now().Add(ttl),
		TTL:       ttl,
	}
}

// Validate returns false when sticky session expired or proxy/ip mismatched.
func (t *StickySessionTracker) Validate(profileID, proxyID, exitIP string) (ok bool, reason string) {
	if t == nil || profileID == "" {
		return true, ""
	}
	t.mu.Lock()
	defer t.mu.Unlock()
	s, exists := t.sessions[profileID]
	if !exists {
		return true, ""
	}
	if time.Now().After(s.ExpiresAt) {
		delete(t.sessions, profileID)
		return false, "sticky_session_expired"
	}
	if proxyID != "" && s.ProxyID != "" && proxyID != s.ProxyID {
		return false, "sticky_proxy_mismatch"
	}
	if exitIP != "" && s.ExitIP != "" && exitIP != s.ExitIP {
		return false, "sticky_exit_ip_drift"
	}
	return true, ""
}

// TTLRemaining returns remaining sticky TTL for profile.
func (t *StickySessionTracker) TTLRemaining(profileID string) time.Duration {
	if t == nil {
		return 0
	}
	t.mu.Lock()
	defer t.mu.Unlock()
	s, ok := t.sessions[profileID]
	if !ok {
		return 0
	}
	if d := time.Until(s.ExpiresAt); d > 0 {
		return d
	}
	return 0
}
