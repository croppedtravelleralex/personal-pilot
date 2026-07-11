package asymmetric

import "math"

// TargetCreepJSTrust99Plus is the minimum CreepJS trust score for S+ fingerprint bonus.
const TargetCreepJSTrust99Plus = 85.0

// StealthDimension scores one defensive layer 0–100.
type StealthDimension struct {
	ID     string   `json:"id"`
	Score  int      `json:"score"`
	Weight float64  `json:"weight"`
	Notes  []string `json:"notes,omitempty"`
}

// StealthMatrixInput feeds the multi-layer 99+ evaluation model.
type StealthMatrixInput struct {
	// Network layer
	WebRTCClean      bool
	DNSConsistent    bool
	ResidentialProxy bool
	VerifyV2Passed   bool
	IPBudgetHeadroom bool // visits today under cap

	// Fingerprint layer
	Runtime80of80    bool
	GeoLocaleMatch   bool
	WebdriverHidden  bool
	CreepJSTrust     float64 // 0 = unknown, 0-100 scale
	DetectionScore   int

	// Behavior layer
	EntropyHumanLike bool
	CadenceScore     int
	BioNoiseActive   bool

	// Trust / API layer
	TrustBundleValid bool
	GraphTokenFresh  bool
	CookiesInjected  bool
	APIFirstReady    bool

	// Operational layer
	InHumanWindow    bool
	AccountSuccess   int
	ChallengeRatePct float64
	PauseActive      bool
}

// StealthMatrixReport is the 99+ readiness output.
type StealthMatrixReport struct {
	TotalScore   float64            `json:"totalScore"`
	DisplayGrade string             `json:"displayGrade"` // S+, S, A, B, C
	Strategy     string             `json:"strategy"`
	Dimensions   []StealthDimension `json:"dimensions"`
	Gaps         []string           `json:"gaps"`
	BonusPoints  float64            `json:"bonusPoints"`
}

// EvaluateStealthMatrix computes weighted multi-layer score (can exceed 99 with bonuses).
func EvaluateStealthMatrix(in StealthMatrixInput) StealthMatrixReport {
	dims := []StealthDimension{
		evalNetwork(in),
		evalFingerprint(in),
		evalBehavior(in),
		evalTrust(in),
		evalOperational(in),
	}
	total := 0.0
	for _, d := range dims {
		total += float64(d.Score) * d.Weight
	}
	bonus := 0.0
	gaps := make([]string, 0, 12)
	if in.APIFirstReady && in.TrustBundleValid && in.GraphTokenFresh {
		bonus += 2.5
	}
	if in.Runtime80of80 && in.GeoLocaleMatch && in.WebRTCClean && in.DNSConsistent {
		bonus += 1.5
	}
	if in.CreepJSTrust >= TargetCreepJSTrust99Plus {
		bonus += 1.0
	}
	total += bonus
	if in.PauseActive {
		total -= 8
		gaps = append(gaps, "profile in challenge cooldown pause")
	}
	if !in.TrustBundleValid {
		gaps = append(gaps, "missing ProfileTrustBundle — cap ~88 without API trust inheritance")
	}
	if !in.ResidentialProxy {
		gaps = append(gaps, "non-residential proxy lowers block-cost asymmetry")
	}
	if in.CreepJSTrust > 0 && in.CreepJSTrust < 70 {
		gaps = append(gaps, "CreepJS trust below 70 — run WorkbenchRunStealthProbeSuite")
	}
	if !in.IPBudgetHeadroom {
		gaps = append(gaps, "IP daily visit budget exhausted — wait or rotate exit IP")
	}
	if total < 0 {
		total = 0
	}
	grade := gradeFromScore(total)
	strategy := "browser_patch_only"
	switch {
	case in.APIFirstReady && in.TrustBundleValid:
		strategy = "api_first_cost_asymmetry"
	case in.TrustBundleValid:
		strategy = "session_inheritance_hybrid"
	case total >= 90:
		strategy = "browser_humanized_matrix"
	}
	return StealthMatrixReport{
		TotalScore:   math.Round(total*10) / 10,
		DisplayGrade: grade,
		Strategy:     strategy,
		Dimensions:   dims,
		Gaps:         gaps,
		BonusPoints:  bonus,
	}
}

func gradeFromScore(score float64) string {
	switch {
	case score >= 99:
		return "S+"
	case score >= 95:
		return "S"
	case score >= 88:
		return "A"
	case score >= 75:
		return "B"
	default:
		return "C"
	}
}

func evalNetwork(in StealthMatrixInput) StealthDimension {
	score := 30
	notes := make([]string, 0, 6)
	if in.WebRTCClean {
		score += 20
	} else {
		notes = append(notes, "webrtc leak suspect")
	}
	if in.DNSConsistent {
		score += 15
	} else {
		notes = append(notes, "dns inconsistent with exit IP")
	}
	if in.ResidentialProxy {
		score += 15
	}
	if in.VerifyV2Passed {
		score += 10
	}
	if in.IPBudgetHeadroom {
		score += 10
	} else {
		notes = append(notes, "IP visit budget tight")
	}
	return StealthDimension{ID: "network", Score: clamp100(score), Weight: 0.22, Notes: notes}
}

func evalFingerprint(in StealthMatrixInput) StealthDimension {
	score := 25
	notes := make([]string, 0, 6)
	if in.Runtime80of80 {
		score += 20
	}
	if in.GeoLocaleMatch {
		score += 15
	} else {
		notes = append(notes, "locale/timezone may drift from proxy geo")
	}
	if in.WebdriverHidden {
		score += 15
	}
	if in.DetectionScore >= 85 {
		score += 10
	} else if in.DetectionScore >= 65 {
		score += 5
	}
	if in.CreepJSTrust >= TargetCreepJSTrust99Plus {
		score += 15
	} else if in.CreepJSTrust >= 70 {
		score += 8
	} else if in.CreepJSTrust > 0 {
		notes = append(notes, "creepjs trust low")
	}
	return StealthDimension{ID: "fingerprint", Score: clamp100(score), Weight: 0.22, Notes: notes}
}

func evalBehavior(in StealthMatrixInput) StealthDimension {
	score := 40
	if in.EntropyHumanLike {
		score += 25
	}
	if in.CadenceScore >= 65 {
		score += 20
	} else if in.CadenceScore >= 50 {
		score += 10
	}
	if in.BioNoiseActive {
		score += 15
	}
	return StealthDimension{ID: "behavior", Score: clamp100(score), Weight: 0.16, Notes: nil}
}

func evalTrust(in StealthMatrixInput) StealthDimension {
	score := 10
	notes := make([]string, 0, 4)
	if in.TrustBundleValid {
		score += 35
	} else {
		notes = append(notes, "no trust bundle")
	}
	if in.GraphTokenFresh {
		score += 25
	}
	if in.CookiesInjected {
		score += 15
	}
	if in.APIFirstReady {
		score += 15
	}
	return StealthDimension{ID: "trust_api", Score: clamp100(score), Weight: 0.25, Notes: notes}
}

func evalOperational(in StealthMatrixInput) StealthDimension {
	score := 35
	notes := make([]string, 0, 4)
	if in.InHumanWindow {
		score += 15
	}
	if in.AccountSuccess >= 85 {
		score += 25
	} else if in.AccountSuccess >= 70 {
		score += 12
	}
	if in.ChallengeRatePct <= 5 {
		score += 25
	} else if in.ChallengeRatePct <= 15 {
		score += 10
	} else {
		notes = append(notes, "elevated challenge rate")
	}
	return StealthDimension{ID: "operational", Score: clamp100(score), Weight: 0.15, Notes: notes}
}

func clamp100(v int) int {
	if v > 100 {
		return 100
	}
	if v < 0 {
		return 0
	}
	return v
}

// EvaluateCostAsymmetry remains for backward compatibility; maps matrix to legacy shape.
func EvaluateCostAsymmetryLegacy(matrix StealthMatrixReport) CostAsymmetryReport {
	score := int(matrix.TotalScore)
	if score > 99 {
		score = 99
	}
	level := "risk"
	switch matrix.DisplayGrade {
	case "S+", "S":
		level = "asymmetric_advantage"
	case "A":
		level = "favorable"
	case "B":
		level = "neutral"
	}
	return CostAsymmetryReport{
		Score:           score,
		Level:           level,
		Strategy:        matrix.Strategy,
		Recommendations: matrix.Gaps,
	}
}
