// Package events provides the event name registry for the Ant Browser Wails event system.
// All event names follow the convention: <namespace>:<subdomain>:<action>
package events

// ─── Browser Instance ──────────────────────────────────────────────────────────

const (
	EventBrowserInstanceStarted = "browser:instance:started"
	EventBrowserInstanceStopped = "browser:instance:stopped"
	EventBrowserInstanceCrashed = "browser:instance:crashed"
	EventBrowserInstanceUpdated = "browser:instance:updated"
)

// ─── Proxy Bridge ──────────────────────────────────────────────────────────────

const (
	EventProxyBridgeDied   = "proxy:bridge:died"
	EventProxyBridgeFailed = "proxy:bridge:failed"
)

// ─── Proxy Quality ─────────────────────────────────────────────────────────────

const (
	EventProxySpeedResult   = "proxy:speed:result"
	EventProxyIPHealthResult = "proxy:iphealth:result"
)

// ─── Risk: Fingerprint ─────────────────────────────────────────────────────────

const (
	// EventRiskFingerprintMismatch is emitted when CDP fingerprint verification finds a divergence
	// between expected and actual browser fingerprint values.
	EventRiskFingerprintMismatch = "risk:fingerprint:mismatch"

	// EventRiskFingerprintTimezoneIP is emitted when the browser's timezone does not match
	// the proxy exit IP's geographic location.
	EventRiskFingerprintTimezoneIP = "risk:fingerprint:timezone-ip"
)

// ─── Risk: Proxy ───────────────────────────────────────────────────────────────

const (
	EventRiskProxyHighLatency  = "risk:proxy:high-latency"
	EventRiskProxyHealthDrop   = "risk:proxy:health-drop"
	EventRiskProxyDatacenter   = "risk:proxy:datacenter"
	EventRiskProxyAuthFailure  = "risk:proxy:auth-failure"
)

// ─── Risk: Network ─────────────────────────────────────────────────────────────

const (
	EventRiskWebRTCLeak = "risk:webrtc:leak"
	EventRiskDNSLeak    = "risk:dns:leak"
)

// ─── Risk: Captcha ─────────────────────────────────────────────────────────────

const (
	EventRiskCaptchaDetected = "risk:captcha:detected"
	EventRiskCaptchaFailed   = "risk:captcha:failed"
)

// ─── Risk: Browser ─────────────────────────────────────────────────────────────

const (
	// EventRiskBrowserCrashLoop is emitted when the same profile crashes 3+ times within 5 minutes.
	EventRiskBrowserCrashLoop = "risk:browser:crash-loop"
)

// ─── Risk: Session ─────────────────────────────────────────────────────────────

const (
	EventRiskSessionRateLimit         = "risk:session:rate-limit"
	EventRiskSessionCookieCleared     = "risk:session:cookie-cleared"
	EventRiskSessionSecurityChallenge = "risk:session:security-challenge"
)

// ─── Risk: System ──────────────────────────────────────────────────────────────

const (
	EventRiskSystemLowDisk       = "risk:system:low-disk"
	EventRiskSystemMemoryPressure = "risk:system:memory-pressure"
)

// ─── Risk: Profile ─────────────────────────────────────────────────────────────

const (
	EventRiskProfileCorrupted = "risk:profile:corrupted"
)

// ─── Risk: Proxy Node ──────────────────────────────────────────────────────────

const (
	EventRiskNodeBanned  = "risk:node:banned"
	EventRiskNodeGeoJump = "risk:node:geo-jump"
	EventRiskNodeOffline = "risk:node:offline"
)

// ─── App ───────────────────────────────────────────────────────────────────────

const (
	EventAppRequestClose = "app:request-close"
)

// ─── Download ──────────────────────────────────────────────────────────────────

const (
	EventDownloadProgress = "download:progress"
)

// AllEventNames returns every event name defined in this package.
// Useful for documentation generation and validation.
func AllEventNames() []string {
	return []string{
		EventBrowserInstanceStarted,
		EventBrowserInstanceStopped,
		EventBrowserInstanceCrashed,
		EventBrowserInstanceUpdated,
		EventProxyBridgeDied,
		EventProxyBridgeFailed,
		EventProxySpeedResult,
		EventProxyIPHealthResult,
		EventRiskFingerprintMismatch,
		EventRiskFingerprintTimezoneIP,
		EventRiskProxyHighLatency,
		EventRiskProxyHealthDrop,
		EventRiskProxyDatacenter,
		EventRiskProxyAuthFailure,
		EventRiskWebRTCLeak,
		EventRiskDNSLeak,
		EventRiskCaptchaDetected,
		EventRiskCaptchaFailed,
		EventRiskBrowserCrashLoop,
		EventRiskSessionRateLimit,
		EventRiskSessionCookieCleared,
		EventRiskSessionSecurityChallenge,
		EventRiskSystemLowDisk,
		EventRiskSystemMemoryPressure,
		EventRiskProfileCorrupted,
		EventRiskNodeBanned,
		EventRiskNodeGeoJump,
		EventRiskNodeOffline,
		EventAppRequestClose,
		EventDownloadProgress,
	}
}
