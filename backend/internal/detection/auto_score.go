package detection

import "strings"

// SiteScore represents an external detector site evaluation.
type SiteScore struct {
	SiteID  string `json:"siteId"`
	URL     string `json:"url"`
	Score   int    `json:"score"`
	Level   string `json:"level"`
	Summary string `json:"summary"`
	Passed  bool   `json:"passed"`
}

// AutoScoreInput carries probe signals from CDP validation.
type AutoScoreInput struct {
	SiteID           string
	WebdriverHidden  bool
	WebrtcClean      bool
	DNSObserved      bool
	DNSConsistent    bool
	CanvasConsistent bool
	TrustScore       int
}

// EvaluateAutoScore maps probe signals to a site pass/fail score.
func EvaluateAutoScore(in AutoScoreInput) SiteScore {
	score := 100
	summary := make([]string, 0, 6)
	if !in.WebdriverHidden {
		score -= 40
		summary = append(summary, "webdriver exposed")
	}
	if !in.WebrtcClean {
		score -= 25
		summary = append(summary, "webrtc leak suspect")
	}
	if in.DNSObserved && !in.DNSConsistent {
		score -= 20
		summary = append(summary, "dns inconsistent")
	}
	if !in.CanvasConsistent {
		score -= 10
		summary = append(summary, "canvas unstable")
	}
	if in.TrustScore > 0 && in.TrustScore < 60 {
		score -= 15
		summary = append(summary, "identity trust low")
	}
	if score < 0 {
		score = 0
	}
	level := "strong"
	switch {
	case score >= 85:
		level = "strong"
	case score >= 65:
		level = "normal"
	default:
		level = "risk"
	}
	if len(summary) == 0 {
		summary = append(summary, "all probes passed")
	}
	return SiteScore{
		SiteID:  in.SiteID,
		Score:   score,
		Level:   level,
		Summary: strings.Join(summary, "; "),
		Passed:  score >= 65,
	}
}

// RemediationHints returns actionable fixes for failed probes.
func RemediationHints(score SiteScore) []string {
	if score.Passed {
		return []string{"no remediation required"}
	}
	hints := make([]string, 0, 4)
	if strings.Contains(score.Summary, "webdriver") {
		hints = append(hints, "ensure environment injection applied before navigation")
	}
	if strings.Contains(score.Summary, "webrtc") {
		hints = append(hints, "enforce disable_non_proxied_udp and block private ICE candidates")
	}
	if strings.Contains(score.Summary, "dns") {
		hints = append(hints, "use socks5h bridge and host-resolver-rules")
	}
	if len(hints) == 0 {
		hints = append(hints, "re-run fingerprint materialize and restart instance")
	}
	return hints
}
