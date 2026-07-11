package backend

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

const workbenchAccountOutcomeKind = "account_outcome"

// WorkbenchAccountOutcome records one business-level account action result.
type WorkbenchAccountOutcome struct {
	ID           string                 `json:"id"`
	ProfileID    string                 `json:"profileId"`
	ProfileName  string                 `json:"profileName,omitempty"`
	Site         string                 `json:"site"`
	Action       string                 `json:"action"`
	Success      bool                   `json:"success"`
	ErrorType    string                 `json:"errorType,omitempty"`
	ProxyID      string                 `json:"proxyId,omitempty"`
	ExitIP       string                 `json:"exitIp,omitempty"`
	CadenceScore int                    `json:"cadenceScore,omitempty"`
	Notes        string                 `json:"notes,omitempty"`
	Payload      map[string]interface{} `json:"payload,omitempty"`
	CreatedAt    string                 `json:"createdAt"`
}

// WorkbenchRecordAccountOutcome persists a task/login/register result for trend analysis.
func (a *App) WorkbenchRecordAccountOutcome(outcome WorkbenchAccountOutcome) (*WorkbenchAccountOutcome, error) {
	profile := a.getProfileSnapshot(strings.TrimSpace(outcome.ProfileID))
	if profile == nil {
		return nil, fmt.Errorf("profile not found: %s", outcome.ProfileID)
	}
	if strings.TrimSpace(outcome.ID) == "" {
		outcome.ID = "outcome-" + generateUUID()
	}
	outcome.ProfileID = profile.ProfileId
	outcome.ProfileName = profile.ProfileName
	if outcome.CreatedAt == "" {
		outcome.CreatedAt = time.Now().UTC().Format(time.RFC3339)
	}
	if strings.TrimSpace(outcome.ProxyID) == "" {
		outcome.ProxyID = profile.ProxyId
	}
	if strings.TrimSpace(outcome.ExitIP) == "" && strings.TrimSpace(outcome.ProxyID) != "" {
		health := a.BrowserProxyCheckIPHealth(outcome.ProxyID)
		if health.Ok {
			outcome.ExitIP = health.IP
		}
	}
	if outcome.CadenceScore <= 0 && outcome.Payload != nil {
		if recID, ok := outcome.Payload["recordingId"].(string); ok && recID != "" {
			if report, err := a.BehaviorRecordingAnalyze(recID); err == nil && report != nil {
				outcome.CadenceScore = report.Cadence.Score
			}
		}
	}
	summary := []string{
		fmt.Sprintf("site=%s action=%s success=%v", outcome.Site, outcome.Action, outcome.Success),
	}
	if outcome.ErrorType != "" {
		summary = append(summary, "error="+outcome.ErrorType)
	}
	payload := map[string]interface{}{
		"site":         outcome.Site,
		"action":       outcome.Action,
		"success":      outcome.Success,
		"errorType":    outcome.ErrorType,
		"proxyId":      outcome.ProxyID,
		"exitIp":       outcome.ExitIP,
		"cadenceScore": outcome.CadenceScore,
		"notes":        outcome.Notes,
	}
	for k, v := range outcome.Payload {
		payload[k] = v
	}
	score := 85
	level := "strong"
	if !outcome.Success {
		score = 35
		level = "risk"
	} else if outcome.CadenceScore > 0 && outcome.CadenceScore < 65 {
		score = 55
		level = "normal"
	}
	result := WorkbenchDetectionResult{
		ID:          outcome.ID,
		ProfileID:   outcome.ProfileID,
		ProfileName: outcome.ProfileName,
		Kind:        workbenchAccountOutcomeKind,
		Score:       score,
		Level:       level,
		Source:      outcome.Site,
		Summary:     summary,
		Payload:     payload,
		CreatedAt:   outcome.CreatedAt,
	}
	if err := a.WorkbenchSaveDetectionResult(result); err != nil {
		return nil, err
	}
	return &outcome, nil
}

// WorkbenchListAccountOutcomes lists recent account outcomes for a profile.
func (a *App) WorkbenchListAccountOutcomes(profileID string, limit int) ([]WorkbenchAccountOutcome, error) {
	results, err := a.WorkbenchListDetectionResults(profileID, workbenchAccountOutcomeKind, limit)
	if err != nil {
		return nil, err
	}
	out := make([]WorkbenchAccountOutcome, 0, len(results))
	for _, item := range results {
		out = append(out, detectionResultToAccountOutcome(item))
	}
	return out, nil
}

// WorkbenchAccountOutcomeSummary aggregates success rate and recent trend.
func (a *App) WorkbenchAccountOutcomeSummary(profileID string) (map[string]interface{}, error) {
	items, err := a.WorkbenchListAccountOutcomes(profileID, 50)
	if err != nil {
		return nil, err
	}
	total := len(items)
	success := 0
	bySite := map[string]map[string]int{}
	for _, item := range items {
		if item.Success {
			success++
		}
		if bySite[item.Site] == nil {
			bySite[item.Site] = map[string]int{"total": 0, "success": 0}
		}
		bySite[item.Site]["total"]++
		if item.Success {
			bySite[item.Site]["success"]++
		}
	}
	rate := 0
	if total > 0 {
		rate = success * 100 / total
	}
	health, _ := a.WorkbenchAccountHealthReport(profileID)
	return map[string]interface{}{
		"total":        total,
		"success":      success,
		"successRate":  rate,
		"bySite":       bySite,
		"healthReport": health,
		"note":         "Use this trend with detector-site probes; single success does not guarantee future pass.",
	}, nil
}

func detectionResultToAccountOutcome(item WorkbenchDetectionResult) WorkbenchAccountOutcome {
	out := WorkbenchAccountOutcome{
		ID:        item.ID,
		ProfileID: item.ProfileID,
		Site:      item.Source,
		CreatedAt: item.CreatedAt,
	}
	if item.Payload != nil {
		if v, ok := item.Payload["action"].(string); ok {
			out.Action = v
		}
		if v, ok := item.Payload["success"].(bool); ok {
			out.Success = v
		}
		if v, ok := item.Payload["errorType"].(string); ok {
			out.ErrorType = v
		}
		if v, ok := item.Payload["proxyId"].(string); ok {
			out.ProxyID = v
		}
		if v, ok := item.Payload["exitIp"].(string); ok {
			out.ExitIP = v
		}
		if v, ok := item.Payload["cadenceScore"].(float64); ok {
			out.CadenceScore = int(v)
		}
		if v, ok := item.Payload["notes"].(string); ok {
			out.Notes = v
		}
		b, _ := json.Marshal(item.Payload)
		_ = json.Unmarshal(b, &out.Payload)
	}
	return out
}
