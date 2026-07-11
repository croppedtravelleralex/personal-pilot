package session

import (
	"strings"
	"time"
)

// TrajectoryPoint is one continuity checkpoint for cross-validation.
type TrajectoryPoint struct {
	At           time.Time
	ProfileID    string
	ProxyID      string
	FingerprintSeed string
	BehaviorProfileID string
	HasRecording bool
}

// TrajectoryValidationReport summarizes SessionBundle vs behavior history coherence.
type TrajectoryValidationReport struct {
	Score   int      `json:"score"`
	Level   string   `json:"level"`
	Issues  []string `json:"issues"`
	Matches []string `json:"matches"`
}

// ValidateTrajectoryCrossReference compares timeline points for consistency.
func ValidateTrajectoryCrossReference(points []TrajectoryPoint) TrajectoryValidationReport {
	report := TrajectoryValidationReport{Score: 100, Level: "strong"}
	if len(points) == 0 {
		report.Score = 50
		report.Level = "weak"
		report.Issues = append(report.Issues, "no trajectory points")
		return report
	}
	var lastProxy, lastSeed string
	for _, p := range points {
		if lastProxy != "" && p.ProxyID != "" && lastProxy != p.ProxyID {
			report.Issues = append(report.Issues, "proxy changed across sessions")
			report.Score -= 15
		}
		if lastSeed != "" && p.FingerprintSeed != "" && lastSeed != p.FingerprintSeed {
			report.Issues = append(report.Issues, "fingerprint seed changed")
			report.Score -= 20
		}
		if p.HasRecording {
			report.Matches = append(report.Matches, "recording continuity at "+p.At.Format(time.RFC3339))
		}
		if p.ProxyID != "" {
			lastProxy = p.ProxyID
		}
		if p.FingerprintSeed != "" {
			lastSeed = p.FingerprintSeed
		}
	}
	if report.Score < 0 {
		report.Score = 0
	}
	switch {
	case report.Score >= 85:
		report.Level = "strong"
	case report.Score >= 65:
		report.Level = "normal"
	default:
		report.Level = "weak"
	}
	if len(report.Issues) == 0 {
		report.Matches = append(report.Matches, "session bundle timeline coherent")
	}
	return report
}

// BuildTrajectoryFromProfileMetadata constructs points from exportable metadata.
func BuildTrajectoryFromProfileMetadata(profileID, proxyID, seed, behaviorID string, createdAt, lastStart string, hasRecording bool) []TrajectoryPoint {
	points := make([]TrajectoryPoint, 0, 2)
	if t, ok := parseTimeFlexible(createdAt); ok {
		points = append(points, TrajectoryPoint{
			At: t, ProfileID: profileID, ProxyID: proxyID, FingerprintSeed: seed,
			BehaviorProfileID: behaviorID, HasRecording: false,
		})
	}
	if t, ok := parseTimeFlexible(lastStart); ok {
		points = append(points, TrajectoryPoint{
			At: t, ProfileID: profileID, ProxyID: proxyID, FingerprintSeed: seed,
			BehaviorProfileID: behaviorID, HasRecording: hasRecording,
		})
	}
	return points
}

func parseTimeFlexible(raw string) (time.Time, bool) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return time.Time{}, false
	}
	layouts := []string{time.RFC3339, "2006-01-02 15:04:05", time.RFC3339Nano}
	for _, layout := range layouts {
		if t, err := time.Parse(layout, raw); err == nil {
			return t, true
		}
	}
	return time.Time{}, false
}
