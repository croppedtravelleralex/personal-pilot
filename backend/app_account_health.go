package backend

import (
	"fmt"
	"strings"
	"time"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/detection"
	"personal-pilot/backend/internal/session"
)

// WorkbenchAccountHealthReport aggregates detection, identity, trajectory, and proxy signals.
type WorkbenchAccountHealthReport struct {
	ProfileID        string                              `json:"profileId"`
	ProfileName      string                              `json:"profileName"`
	OverallScore     int                                 `json:"overallScore"`
	OverallLevel     string                              `json:"overallLevel"`
	DetectionPassed  bool                                `json:"detectionPassed"`
	DetectionScore   int                                 `json:"detectionScore"`
	DetectionSummary string                              `json:"detectionSummary"`
	IdentityScore    int                                 `json:"identityScore"`
	Trajectory       session.TrajectoryValidationReport    `json:"trajectory"`
	Signals          LiveDetectionSignals                `json:"signals"`
	Recommendations  []string                            `json:"recommendations"`
	ConfidenceNote   string                              `json:"confidenceNote"`
}

func (a *App) WorkbenchAccountHealthReport(profileID string) (*WorkbenchAccountHealthReport, error) {
	profileID = strings.TrimSpace(profileID)
	if profileID == "" {
		return nil, fmt.Errorf("profileId is required")
	}
	profile := a.getProfileSnapshot(profileID)
	if profile == nil {
		return nil, fmt.Errorf("profile not found: %s", profileID)
	}

	var fpSnapshot *browser.FingerprintSnapshot
	if running, err := a.runningProfileForWorkbench(profileID); err == nil && running != nil {
		if fp, err := browser.ExtractFingerprint(running.DebugPort); err == nil {
			fpSnapshot = fp
		}
	}

	trust := 75
	identityScore := 0
	if report, err := a.IdentityReportProfile(profileID); err == nil && report != nil {
		trust = report.Score
		identityScore = report.Score
	}

	autoIn := a.liveAutoScoreInput(profileID, fpSnapshot, trust)
	siteScore := detection.EvaluateAutoScore(autoIn)
	signals := a.collectLiveDetectionSignals(profileID, fpSnapshot)

	traj := session.ValidateTrajectoryCrossReference(session.BuildTrajectoryFromProfileMetadata(
		profile.ProfileId, profile.ProxyId, profile.HumanizeSeed, profile.BehaviorProfileID,
		profile.CreatedAt, profile.LastStartAt, strings.TrimSpace(profile.BehaviorProfileID) != "",
	))

	overall := (siteScore.Score + identityScore + traj.Score) / 3
	if !signals.VerifyV2Passed && signals.ExitIP != "" {
		overall -= 5
	}
	if overall < 0 {
		overall = 0
	}
	level := "risk"
	switch {
	case overall >= 85:
		level = "strong"
	case overall >= 65:
		level = "normal"
	}

	recs := detection.RemediationHints(siteScore)
	if traj.Score < 65 {
		recs = append(recs, "stabilize proxy/seed across sessions before automation")
	}
	if identityScore < 65 {
		recs = append(recs, "improve behavior cadence and lifecycle continuity")
	}
	recs = append(recs, "run WorkbenchRunDetectionBundle then verify account outcomes over 7+ days")

	return &WorkbenchAccountHealthReport{
		ProfileID:        profile.ProfileId,
		ProfileName:      profile.ProfileName,
		OverallScore:     overall,
		OverallLevel:     level,
		DetectionPassed:  siteScore.Passed,
		DetectionScore:   siteScore.Score,
		DetectionSummary: siteScore.Summary,
		IdentityScore:    identityScore,
		Trajectory:       traj,
		Signals:          signals,
		Recommendations:  recs,
		ConfidenceNote: "This is probabilistic readiness, not a guarantee that target-site risk engines cannot detect automation. Combine with detector sites and real account outcomes.",
	}, nil
}

// WorkbenchRunDetectionBundle opens high-value leak detector pages and persists a scored result.
func (a *App) WorkbenchRunDetectionBundle(profileID string) (map[string]interface{}, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	quality, reason := a.workbenchProxyQuality(profile)
	if quality != "medium" && quality != "strict" {
		return nil, fmt.Errorf("detection bundle blocked: proxy quality below medium (%s)", reason)
	}

	targets := []struct {
		id  string
		url string
	}{
		{"browserleaks-ip", "https://browserleaks.com/ip"},
		{"browserleaks-webrtc", "https://browserleaks.com/webrtc"},
		{"pixelscan", "https://pixelscan.net/"},
	}
	opened := make([]string, 0, len(targets))
	for _, target := range targets {
		if err := cdpOpenNewTarget(profile.DebugPort, target.url); err != nil {
			return nil, fmt.Errorf("open %s: %w", target.id, err)
		}
		opened = append(opened, target.id)
	}

	fp, _ := browser.ExtractFingerprint(profile.DebugPort)
	autoIn := a.liveAutoScoreInput(profileID, fp, 80)
	score := detection.EvaluateAutoScore(autoIn)
	result := WorkbenchDetectionResult{
		ID:          "det-bundle-" + generateUUID(),
		ProfileID:   profile.ProfileId,
		ProfileName: profile.ProfileName,
		Kind:        workbenchDetectionKindDetector,
		Score:       score.Score,
		Level:       score.Level,
		Source:      "detection_bundle",
		Summary:     []string{score.Summary, "opened: " + strings.Join(opened, ", ")},
		Payload: map[string]interface{}{
			"opened":    opened,
			"passed":    score.Passed,
			"signals":   a.collectLiveDetectionSignals(profileID, fp),
			"traceNote": "Manual review required on opened tabs; software does not parse third-party scores yet.",
		},
		CreatedAt: timeNowRFC3339(),
	}
	if err := a.WorkbenchSaveDetectionResult(result); err != nil {
		return nil, err
	}
	return map[string]interface{}{
		"opened":  opened,
		"score":   score.Score,
		"level":   score.Level,
		"passed":  score.Passed,
		"summary": score.Summary,
		"hints":   detection.RemediationHints(score),
		"resultId": result.ID,
	}, nil
}

func timeNowRFC3339() string {
	return time.Now().UTC().Format(time.RFC3339)
}
