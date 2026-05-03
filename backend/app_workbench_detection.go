package backend

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

const (
	workbenchDetectionKindFingerprint = "fingerprint_health"
	workbenchDetectionKindIdentity    = "identity_report"
	workbenchDetectionKindDetector    = "detector_site_run"
	workbenchDefaultStateKey          = "default"
	workbenchDetectionRetention       = 50
)

// WorkbenchDetectionResult is persisted only in AntBrowser's own SQLite DB.
// It never writes browser profile data, cookies, history, or user-data-dir.
type WorkbenchDetectionResult struct {
	ID          string                 `json:"id"`
	ProfileID   string                 `json:"profileId"`
	ProfileName string                 `json:"profileName"`
	Kind        string                 `json:"kind"`
	Score       int                    `json:"score"`
	Level       string                 `json:"level"`
	Source      string                 `json:"source"`
	Summary     []string               `json:"summary"`
	Payload     map[string]interface{} `json:"payload"`
	CreatedAt   string                 `json:"createdAt"`
}

type WorkbenchUiState struct {
	Search                  string   `json:"search"`
	StatusFilter            string   `json:"statusFilter"`
	GroupFilter             string   `json:"groupFilter"`
	ActiveGroupID           string   `json:"activeGroupId"`
	SelectedIDs             []string `json:"selectedIds"`
	ScrollTop               int      `json:"scrollTop"`
	TargetURL               string   `json:"targetUrl"`
	SelectedReportKind      string   `json:"selectedReportKind"`
	SelectedReportID        string   `json:"selectedReportId"`
	SelectedReportProfileID string   `json:"selectedReportProfileId"`
	ExpandedItems           []string `json:"expandedItems"`
	ThirdPartyEnabled       bool     `json:"thirdPartyEnabled"`
	UpdatedAt               string   `json:"updatedAt"`
}

type WorkbenchDetectorSite struct {
	ID           string `json:"id"`
	Name         string `json:"name"`
	URL          string `json:"url"`
	Enabled      bool   `json:"enabled"`
	DefaultOn    bool   `json:"defaultOn"`
	Gate         string `json:"gate"`
	TraceWarning string `json:"traceWarning"`
	Notes        string `json:"notes"`
}

func (a *App) WorkbenchSaveDetectionResult(result WorkbenchDetectionResult) error {
	normalized, err := normalizeWorkbenchDetectionResult(result)
	if err != nil {
		return err
	}
	db, err := a.workbenchSQL()
	if err != nil {
		return err
	}

	summaryJSON, err := json.Marshal(normalized.Summary)
	if err != nil {
		return fmt.Errorf("marshal detection summary: %w", err)
	}
	payloadJSON, err := json.Marshal(normalized.Payload)
	if err != nil {
		return fmt.Errorf("marshal detection payload: %w", err)
	}

	tx, err := db.Begin()
	if err != nil {
		return fmt.Errorf("begin detection save: %w", err)
	}
	defer tx.Rollback()

	if _, err := tx.Exec(`
		INSERT INTO workbench_detection_results
			(id, profile_id, profile_name, kind, score, level, source, summary, payload, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(id) DO UPDATE SET
			profile_id=excluded.profile_id,
			profile_name=excluded.profile_name,
			kind=excluded.kind,
			score=excluded.score,
			level=excluded.level,
			source=excluded.source,
			summary=excluded.summary,
			payload=excluded.payload,
			created_at=excluded.created_at`,
		normalized.ID, normalized.ProfileID, normalized.ProfileName, normalized.Kind, normalized.Score,
		normalized.Level, normalized.Source, string(summaryJSON), string(payloadJSON), normalized.CreatedAt,
	); err != nil {
		return fmt.Errorf("save detection result: %w", err)
	}

	if _, err := tx.Exec(`
		DELETE FROM workbench_detection_results
		WHERE profile_id = ? AND kind = ? AND id NOT IN (
			SELECT id FROM workbench_detection_results
			WHERE profile_id = ? AND kind = ?
			ORDER BY datetime(created_at) DESC, rowid DESC
			LIMIT ?
		)`,
		normalized.ProfileID, normalized.Kind, normalized.ProfileID, normalized.Kind, workbenchDetectionRetention,
	); err != nil {
		return fmt.Errorf("prune detection results: %w", err)
	}

	return tx.Commit()
}

func (a *App) WorkbenchListDetectionResults(profileID string, kind string, limit int) ([]WorkbenchDetectionResult, error) {
	db, err := a.workbenchSQL()
	if err != nil {
		return nil, err
	}
	profileID = strings.TrimSpace(profileID)
	kind = strings.TrimSpace(kind)
	if kind != "" && !validWorkbenchDetectionKind(kind) {
		return nil, fmt.Errorf("unsupported detection kind: %s", kind)
	}
	if limit <= 0 || limit > 200 {
		limit = workbenchDetectionRetention
	}

	query := `SELECT id, profile_id, profile_name, kind, score, level, source, summary, payload, created_at
		FROM workbench_detection_results`
	conditions := make([]string, 0, 2)
	args := make([]interface{}, 0, 3)
	if profileID != "" {
		conditions = append(conditions, "profile_id = ?")
		args = append(args, profileID)
	}
	if kind != "" {
		conditions = append(conditions, "kind = ?")
		args = append(args, kind)
	}
	if len(conditions) > 0 {
		query += " WHERE " + strings.Join(conditions, " AND ")
	}
	query += " ORDER BY datetime(created_at) DESC, rowid DESC LIMIT ?"
	args = append(args, limit)

	rows, err := db.Query(query, args...)
	if err != nil {
		return nil, fmt.Errorf("list detection results: %w", err)
	}
	defer rows.Close()

	results := make([]WorkbenchDetectionResult, 0)
	for rows.Next() {
		result, err := scanWorkbenchDetectionResult(rows)
		if err != nil {
			return nil, err
		}
		results = append(results, result)
	}
	return results, rows.Err()
}

func (a *App) WorkbenchGetUiState() (WorkbenchUiState, error) {
	db, err := a.workbenchSQL()
	if err != nil {
		return defaultWorkbenchUiState(), err
	}
	var payload string
	err = db.QueryRow(`SELECT payload FROM workbench_ui_state WHERE state_key = ?`, workbenchDefaultStateKey).Scan(&payload)
	if err == sql.ErrNoRows {
		return defaultWorkbenchUiState(), nil
	}
	if err != nil {
		return defaultWorkbenchUiState(), fmt.Errorf("load workbench ui state: %w", err)
	}
	state := defaultWorkbenchUiState()
	if err := json.Unmarshal([]byte(payload), &state); err != nil {
		return defaultWorkbenchUiState(), fmt.Errorf("parse workbench ui state: %w", err)
	}
	return normalizeWorkbenchUiState(state), nil
}

func (a *App) WorkbenchSaveUiState(state WorkbenchUiState) error {
	db, err := a.workbenchSQL()
	if err != nil {
		return err
	}
	normalized := normalizeWorkbenchUiState(state)
	normalized.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
	payload, err := json.Marshal(normalized)
	if err != nil {
		return fmt.Errorf("marshal workbench ui state: %w", err)
	}
	_, err = db.Exec(`
		INSERT INTO workbench_ui_state (state_key, payload, updated_at)
		VALUES (?, ?, ?)
		ON CONFLICT(state_key) DO UPDATE SET
			payload=excluded.payload,
			updated_at=excluded.updated_at`,
		workbenchDefaultStateKey, string(payload), normalized.UpdatedAt,
	)
	if err != nil {
		return fmt.Errorf("save workbench ui state: %w", err)
	}
	return nil
}

func (a *App) WorkbenchListDetectorSites() []WorkbenchDetectorSite {
	return append([]WorkbenchDetectorSite{}, builtinWorkbenchDetectorSites()...)
}

func (a *App) WorkbenchRunDetectorSite(profileID string, detectorID string) (*WorkbenchDetectionResult, error) {
	profile, err := a.runningProfileForWorkbench(profileID)
	if err != nil {
		return nil, err
	}
	site, ok := findWorkbenchDetectorSite(detectorID)
	if !ok {
		return nil, fmt.Errorf("unsupported detector site: %s", detectorID)
	}
	quality, reason := a.workbenchProxyQuality(profile)
	if quality != "medium" && quality != "strict" {
		return nil, fmt.Errorf("third-party detector blocked: proxy quality below medium (%s)", reason)
	}
	if err := cdpOpenNewTarget(profile.DebugPort, site.URL); err != nil {
		result := detectorSiteResult(profile, site, "risk", []string{"第三方检测站打开失败: " + err.Error()})
		_ = a.WorkbenchSaveDetectionResult(*result)
		return nil, err
	}
	result := detectorSiteResult(profile, site, "info", []string{
		"已在真实 profile 新标签打开检测站",
		"本次访问可能留下 cookie/history/服务器日志痕迹，软件不会自动清理",
	})
	if err := a.WorkbenchSaveDetectionResult(*result); err != nil {
		return nil, err
	}
	return result, nil
}

func (a *App) workbenchSQL() (*sql.DB, error) {
	if a == nil || a.db == nil || a.db.GetConn() == nil {
		return nil, fmt.Errorf("database is not initialized")
	}
	return a.db.GetConn(), nil
}

func normalizeWorkbenchDetectionResult(result WorkbenchDetectionResult) (WorkbenchDetectionResult, error) {
	result.ID = strings.TrimSpace(result.ID)
	result.ProfileID = strings.TrimSpace(result.ProfileID)
	result.ProfileName = strings.TrimSpace(result.ProfileName)
	result.Kind = strings.TrimSpace(result.Kind)
	result.Level = strings.TrimSpace(result.Level)
	result.Source = strings.TrimSpace(result.Source)
	result.CreatedAt = strings.TrimSpace(result.CreatedAt)
	if result.ID == "" {
		result.ID = "det-" + generateUUID()
	}
	if result.ProfileID == "" {
		return result, fmt.Errorf("profileId is required")
	}
	if !validWorkbenchDetectionKind(result.Kind) {
		return result, fmt.Errorf("unsupported detection kind: %s", result.Kind)
	}
	if result.Score < 0 {
		result.Score = 0
	}
	if result.Score > 100 {
		result.Score = 100
	}
	if result.Source == "" {
		result.Source = "local-cdp"
	}
	if result.Level == "" {
		result.Level = "unknown"
	}
	if result.CreatedAt == "" {
		result.CreatedAt = time.Now().UTC().Format(time.RFC3339)
	}
	if result.Summary == nil {
		result.Summary = []string{}
	}
	if result.Payload == nil {
		result.Payload = map[string]interface{}{}
	}
	return result, nil
}

func validWorkbenchDetectionKind(kind string) bool {
	switch kind {
	case workbenchDetectionKindFingerprint, workbenchDetectionKindIdentity, workbenchDetectionKindDetector:
		return true
	default:
		return false
	}
}

func scanWorkbenchDetectionResult(rows *sql.Rows) (WorkbenchDetectionResult, error) {
	var result WorkbenchDetectionResult
	var summaryJSON string
	var payloadJSON string
	if err := rows.Scan(
		&result.ID,
		&result.ProfileID,
		&result.ProfileName,
		&result.Kind,
		&result.Score,
		&result.Level,
		&result.Source,
		&summaryJSON,
		&payloadJSON,
		&result.CreatedAt,
	); err != nil {
		return result, fmt.Errorf("scan detection result: %w", err)
	}
	if strings.TrimSpace(summaryJSON) != "" {
		_ = json.Unmarshal([]byte(summaryJSON), &result.Summary)
	}
	if result.Summary == nil {
		result.Summary = []string{}
	}
	if strings.TrimSpace(payloadJSON) != "" {
		_ = json.Unmarshal([]byte(payloadJSON), &result.Payload)
	}
	if result.Payload == nil {
		result.Payload = map[string]interface{}{}
	}
	return result, nil
}

func defaultWorkbenchUiState() WorkbenchUiState {
	return WorkbenchUiState{
		StatusFilter: "all",
		GroupFilter:  "all",
	}
}

func normalizeWorkbenchUiState(state WorkbenchUiState) WorkbenchUiState {
	state.Search = strings.TrimSpace(state.Search)
	state.StatusFilter = normalizedWorkbenchStateChoice(state.StatusFilter, []string{"all", "running", "stopped"}, "all")
	state.GroupFilter = strings.TrimSpace(state.GroupFilter)
	if state.GroupFilter == "" {
		state.GroupFilter = "all"
	}
	state.ActiveGroupID = strings.TrimSpace(state.ActiveGroupID)
	state.SelectedIDs = uniqueProfileIDs(state.SelectedIDs)
	if state.ScrollTop < 0 {
		state.ScrollTop = 0
	}
	state.TargetURL = strings.TrimSpace(state.TargetURL)
	state.SelectedReportKind = strings.TrimSpace(state.SelectedReportKind)
	if state.SelectedReportKind != "" && !validWorkbenchDetectionKind(state.SelectedReportKind) {
		state.SelectedReportKind = ""
	}
	state.SelectedReportID = strings.TrimSpace(state.SelectedReportID)
	state.SelectedReportProfileID = strings.TrimSpace(state.SelectedReportProfileID)
	state.ExpandedItems = uniqueProfileIDs(state.ExpandedItems)
	state.UpdatedAt = strings.TrimSpace(state.UpdatedAt)
	return state
}

func normalizedWorkbenchStateChoice(value string, allowed []string, fallback string) string {
	value = strings.TrimSpace(value)
	for _, item := range allowed {
		if value == item {
			return value
		}
	}
	return fallback
}

func builtinWorkbenchDetectorSites() []WorkbenchDetectorSite {
	const traceWarning = "使用真实 profile 新标签访问，可能留下 cookie/history/服务器日志痕迹；软件不会自动清理。"
	return []WorkbenchDetectorSite{
		{
			ID:           "browserleaks",
			Name:         "BrowserLeaks",
			URL:          "https://browserleaks.com/",
			Enabled:      true,
			DefaultOn:    false,
			Gate:         "medium",
			TraceWarning: traceWarning,
			Notes:        "Canvas/WebGL/Fonts 等浏览器可见指纹分项检测。",
		},
		{
			ID:           "creepjs",
			Name:         "CreepJS",
			URL:          "https://abrahamjuliot.github.io/creepjs/",
			Enabled:      true,
			DefaultOn:    false,
			Gate:         "medium",
			TraceWarning: traceWarning,
			Notes:        "Headless、WebRTC、Timezone、Canvas、Audio、Fonts 等多维检测。",
		},
		{
			ID:           "pixelscan",
			Name:         "Pixelscan",
			URL:          "https://pixelscan.net/",
			Enabled:      true,
			DefaultOn:    false,
			Gate:         "medium",
			TraceWarning: traceWarning,
			Notes:        "Fingerprint、IP、DNS leak、Bot、位置一致性诊断。",
		},
	}
}

func findWorkbenchDetectorSite(detectorID string) (WorkbenchDetectorSite, bool) {
	detectorID = strings.TrimSpace(strings.ToLower(detectorID))
	for _, site := range builtinWorkbenchDetectorSites() {
		if strings.EqualFold(site.ID, detectorID) {
			return site, true
		}
	}
	return WorkbenchDetectorSite{}, false
}

func detectorSiteResult(profile *BrowserProfile, site WorkbenchDetectorSite, level string, summary []string) *WorkbenchDetectionResult {
	return &WorkbenchDetectionResult{
		ID:          "det-" + generateUUID(),
		ProfileID:   profile.ProfileId,
		ProfileName: profile.ProfileName,
		Kind:        workbenchDetectionKindDetector,
		Score:       0,
		Level:       level,
		Source:      site.ID,
		Summary:     summary,
		Payload: map[string]interface{}{
			"detectorId":   site.ID,
			"detectorName": site.Name,
			"url":          site.URL,
			"traceWarning": site.TraceWarning,
		},
		CreatedAt: time.Now().UTC().Format(time.RFC3339),
	}
}

func (a *App) workbenchProxyQuality(profile *BrowserProfile) (string, string) {
	if profile == nil {
		return "unknown", "profile missing"
	}
	proxyID := strings.TrimSpace(profile.ProxyId)
	if proxyID == "" || proxyID == "__direct__" {
		return "unknown", "no proxy bound"
	}
	for _, item := range a.getLatestProxies() {
		if !strings.EqualFold(item.ProxyId, proxyID) {
			continue
		}
		var health ProxyIPHealthResult
		if strings.TrimSpace(item.LastIPHealthJSON) == "" {
			return "unknown", "proxy IP health not checked"
		}
		if err := json.Unmarshal([]byte(item.LastIPHealthJSON), &health); err != nil {
			return "unknown", "proxy IP health parse failed"
		}
		if !health.Ok {
			return "risk", firstNonEmptyString(health.Error, "proxy IP health failed")
		}
		if health.FraudScore > 60 || health.IsBroadcast {
			return "risk", "proxy IP health below medium"
		}
		if !workbenchHealthHTTPSUsable(health, item.LastTestOk) {
			return "risk", "proxy HTTPS is not verified"
		}
		return "medium", "IP health good + HTTPS usable"
	}
	return "unknown", "proxy not found"
}

func workbenchHealthHTTPSUsable(health ProxyIPHealthResult, speedOK bool) bool {
	if health.RawData != nil {
		if raw, ok := health.RawData["realHttpsCheck"].(map[string]interface{}); ok && mapBool(raw, "ok") {
			return true
		}
		if mapBool(health.RawData, "realCheckOk") {
			return true
		}
	}
	return speedOK
}

func cdpOpenNewTarget(debugPort int, targetURL string) error {
	conn, err := cdpConnect(debugPort)
	if err != nil {
		return err
	}
	defer conn.Close()
	_, err = cdpSend(conn, "Target.createTarget", map[string]interface{}{
		"url":       targetURL,
		"newWindow": false,
	})
	return err
}
