package asymmetric

import (
	"math"
	"strings"
	"time"
)

// HumanTimePolicy defines when automation may run to mimic human circadian rhythm.
type HumanTimePolicy struct {
	Timezone       string `json:"timezone"`
	ActiveStartHour int   `json:"activeStartHour"` // inclusive, local
	ActiveEndHour   int   `json:"activeEndHour"`   // exclusive
	WeekendShift    int   `json:"weekendShift"`    // hours to shift window on Sat/Sun
	BlockOvernight  bool   `json:"blockOvernight"`
}

func DefaultHumanTimePolicy() HumanTimePolicy {
	return HumanTimePolicy{
		Timezone:        "Asia/Shanghai",
		ActiveStartHour: 7,
		ActiveEndHour:   23,
		WeekendShift:    1,
		BlockOvernight:  true,
	}
}

// InHumanWindow reports whether now falls inside the allowed human activity window.
func (p HumanTimePolicy) InHumanWindow(now time.Time) bool {
	loc := time.Local
	if tz := strings.TrimSpace(p.Timezone); tz != "" {
		if l, err := time.LoadLocation(tz); err == nil {
			loc = l
		}
	}
	local := now.In(loc)
	start := p.ActiveStartHour
	end := p.ActiveEndHour
	if local.Weekday() == time.Saturday || local.Weekday() == time.Sunday {
		start += p.WeekendShift
		end += p.WeekendShift
	}
	hour := local.Hour()
	if p.BlockOvernight && (hour < 7 || hour >= 23) {
		return false
	}
	if start <= end {
		return hour >= start && hour < end
	}
	return hour >= start || hour < end
}

// ActivityEntropyInput feeds hourly activity buckets for Shannon entropy.
type ActivityEntropyInput struct {
	HourlyCounts [24]int
}

// ShannonEntropyH returns H in bits; human-like daily traffic often ~2.5-3.5.
func (in ActivityEntropyInput) ShannonEntropyH() float64 {
	total := 0
	for _, c := range in.HourlyCounts {
		total += c
	}
	if total == 0 {
		return 0
	}
	h := 0.0
	for _, c := range in.HourlyCounts {
		if c == 0 {
			continue
		}
		p := float64(c) / float64(total)
		h -= p * math.Log2(p)
	}
	return h
}

// EntropyAssessment interprets H for bot-vs-human heuristics.
type EntropyAssessment struct {
	H              float64 `json:"h"`
	Level          string  `json:"level"`
	BotLike        bool    `json:"botLike"`
	Recommendation string  `json:"recommendation"`
}

func AssessEntropy(h float64) EntropyAssessment {
	out := EntropyAssessment{H: h}
	switch {
	case h <= 0:
		out.Level = "unknown"
		out.Recommendation = "collect more activity timestamps before automation"
	case h > 4.2:
		out.Level = "bot_uniform"
		out.BotLike = true
		out.Recommendation = "stop 24h uniform scheduling; restrict to local peak hours"
	case h > 3.8:
		out.Level = "high_spread"
		out.Recommendation = "add weekend gaps and 1-3h idle blocks"
	case h >= 2.2 && h <= 3.6:
		out.Level = "human_like"
		out.Recommendation = "entropy within human band; maintain current cadence"
	default:
		out.Level = "narrow"
		out.Recommendation = " diversify active hours across 6-10 hour band"
	}
	return out
}

// CostAsymmetryInput models economic/compliance asymmetry signals.
type CostAsymmetryInput struct {
	HasTrustBundle     bool    `json:"hasTrustBundle"`
	HasGraphToken      bool    `json:"hasGraphToken"`
	ResidentialProxy   bool    `json:"residentialProxy"`
	SingleAccountMode  bool    `json:"singleAccountMode"`
	LowFrequencyOps    bool    `json:"lowFrequencyOps"`
	ChallengeRatePct   float64 `json:"challengeRatePct"`
	InHumanWindow      bool    `json:"inHumanWindow"`
	EntropyHumanLike   bool    `json:"entropyHumanLike"`
	DetectionScore     int     `json:"detectionScore"`
	AccountSuccessRate int     `json:"accountSuccessRate"`
}

// CostAsymmetryReport scores how costly blocking this traffic would be for the defender.
type CostAsymmetryReport struct {
	Score          int      `json:"score"`
	Level          string   `json:"level"`
	Strategy       string   `json:"strategy"`
	Recommendations []string `json:"recommendations"`
}

// EvaluateCostAsymmetry returns a 0-99 stealth readiness score (not a guarantee).
func EvaluateCostAsymmetry(in CostAsymmetryInput) CostAsymmetryReport {
	score := 40
	recs := make([]string, 0, 8)
	if in.HasTrustBundle {
		score += 15
		recs = append(recs, "trust inheritance active: prefer API/token path over browser login")
	} else {
		recs = append(recs, "complete one high-quality manual bootstrap and save ProfileTrustBundle")
	}
	if in.HasGraphToken {
		score += 12
	}
	if in.ResidentialProxy {
		score += 8
	} else {
		recs = append(recs, "use residential/mobile proxy to raise block cost")
	}
	if in.SingleAccountMode && in.LowFrequencyOps {
		score += 10
	} else {
		recs = append(recs, "stay in single-account low-frequency research band")
	}
	if in.InHumanWindow {
		score += 6
	} else {
		recs = append(recs, "defer automation until local human activity window")
	}
	if in.EntropyHumanLike {
		score += 8
	}
	if in.DetectionScore >= 85 {
		score += 8
	} else if in.DetectionScore >= 65 {
		score += 4
	} else {
		recs = append(recs, "improve detection score via WorkbenchRunDetectionBundle")
	}
	if in.AccountSuccessRate >= 85 {
		score += 7
	}
	if in.ChallengeRatePct > 20 {
		score -= 15
		recs = append(recs, "challenge rate high: rotate fingerprint seed and pause 24-48h")
	}
	if score > 99 {
		score = 99
	}
	if score < 0 {
		score = 0
	}
	level := "risk"
	switch {
	case score >= 92:
		level = "asymmetric_advantage"
	case score >= 80:
		level = "favorable"
	case score >= 65:
		level = "neutral"
	}
	strategy := "browser_only_patch"
	switch {
	case in.HasGraphToken && in.HasTrustBundle:
		strategy = "api_first_trust_inheritance"
	case in.HasTrustBundle:
		strategy = "session_inheritance_browser_fallback"
	case score >= 80:
		strategy = "browser_humanized_low_entropy"
	}
	return CostAsymmetryReport{
		Score:           score,
		Level:           level,
		Strategy:        strategy,
		Recommendations: recs,
	}
}

// ChallengeRecord is one CAPTCHA/403/interstitial event for feedback tuning.
type ChallengeRecord struct {
	ProfileID     string    `json:"profileId"`
	Site          string    `json:"site"`
	ChallengeType string    `json:"challengeType"`
	FingerprintSeed string  `json:"fingerprintSeed,omitempty"`
	ProxyID       string    `json:"proxyId,omitempty"`
	ExitIP        string    `json:"exitIp,omitempty"`
	RecordedAt    time.Time `json:"recordedAt"`
}

// FeedbackAdjustments suggests parameter shifts after repeated challenges.
type FeedbackAdjustments struct {
	RotateFingerprintSeed bool     `json:"rotateFingerprintSeed"`
	PauseHours            int      `json:"pauseHours"`
	SwitchProxy           bool     `json:"switchProxy"`
	LowerAutomationLevel  bool     `json:"lowerAutomationLevel"`
	PreferAPIPath         bool     `json:"preferAPIPath"`
	Notes                 []string `json:"notes"`
}

func DeriveFeedback(challenges []ChallengeRecord, window time.Duration) FeedbackAdjustments {
	out := FeedbackAdjustments{}
	if len(challenges) == 0 {
		return out
	}
	recent := 0
	siteCount := map[string]int{}
	for _, c := range challenges {
		if time.Since(c.RecordedAt) <= window {
			recent++
			siteCount[c.Site]++
		}
	}
	if recent >= 3 {
		out.RotateFingerprintSeed = true
		out.PauseHours = 24
		out.SwitchProxy = true
		out.LowerAutomationLevel = true
		out.Notes = append(out.Notes, "3+ challenges in window: enter cooldown")
	}
	if recent >= 1 {
		out.PreferAPIPath = true
		out.Notes = append(out.Notes, "prefer Graph/API path over browser login surface")
	}
	return out
}
