package launchcode

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"strings"

	"personal-pilot/backend/internal/asymmetric"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/detection"
)

type workbenchProfileRequest struct {
	ProfileID string `json:"profileId"`
}

type workbenchNavigateRequest struct {
	ProfileID string `json:"profileId"`
	URL       string `json:"url"`
	TabID     string `json:"tabId,omitempty"`
}

type workbenchArrangeRequest struct {
	ProfileIDs []string `json:"profileIds"`
	Layout     string   `json:"layout"`
}

type workbenchClickRequest struct {
	ProfileID string `json:"profileId"`
	Selector  string `json:"selector"`
	TabID     string `json:"tabId,omitempty"`
}

type workbenchTypeRequest struct {
	ProfileID string `json:"profileId"`
	Selector  string `json:"selector"`
	Text      string `json:"text"`
	TabID     string `json:"tabId,omitempty"`
}

type workbenchScrollRequest struct {
	ProfileID string `json:"profileId"`
	Distance  uint32 `json:"distance"`
	TabID     string `json:"tabId,omitempty"`
}

func (s *LaunchServer) handleWorkbenchNavigate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}

	var req workbenchNavigateRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	targetURL := strings.TrimSpace(req.URL)
	if profileID == "" || targetURL == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId and url are required"})
		return
	}
	tabID := strings.TrimSpace(req.TabID)
	results, err := operator.WorkbenchExecuteActions(profileID, []ActionRequest{{
		Type:              "navigate",
		URL:               targetURL,
		PostWaitMs:        2500,
		HumanizationLevel: "high",
		TabID:             tabID,
	}})
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	pageURL := ""
	pageTitle := ""
	okNav := false
	errMsg := ""
	if len(results) > 0 {
		okNav = results[0].OK
		pageURL = results[0].PageURL
		pageTitle = results[0].PageTitle
		errMsg = results[0].Error
	}
	if !okNav {
		writeJSON(w, http.StatusBadGateway, map[string]interface{}{
			"ok": false, "profileId": profileID, "url": targetURL, "tabId": tabID,
			"pageUrl": pageURL, "pageTitle": pageTitle, "error": errMsg,
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true, "profileId": profileID, "url": targetURL, "tabId": tabID,
		"pageUrl": pageURL, "pageTitle": pageTitle,
	})
}

func (s *LaunchServer) handleWorkbenchRefresh(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	if err := operator.WorkbenchRefreshProfile(profileID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "refreshed": true})
}

func (s *LaunchServer) handleWorkbenchScreenshot(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	dataURL, err := operator.WorkbenchCaptureScreenshot(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "screenshot": dataURL})
}

func (s *LaunchServer) handleWorkbenchFullReport(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	report, err := operator.WorkbenchCaptureFullReport(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	if report == nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "profileId": profileID, "error": "full report is empty"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":        true,
		"profileId": profileID,
		"report":    report,
		"failures":  report.Failures,
	})
}

func (s *LaunchServer) handleWorkbenchFingerprint(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	snapshot, err := operator.WorkbenchFingerprintProfile(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "fingerprint": snapshot})
}

func (s *LaunchServer) handleWorkbenchFingerprintHealth(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	health, err := operator.WorkbenchFingerprintHealthProfile(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	if health == nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "profileId": profileID, "error": "fingerprint health is empty"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":          true,
		"profileId":   profileID,
		"score":       health.Score,
		"level":       health.Level,
		"checks":      health.Checks,
		"fingerprint": health.Fingerprint,
		"capturedAt":  health.CapturedAt,
		"source":      health.Source,
	})
}

func (s *LaunchServer) handleWorkbenchStealthProbe(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	probeOp, ok := operator.(StealthProbeOperator)
	if !ok {
		writeJSON(w, http.StatusNotImplemented, map[string]interface{}{"ok": false, "error": "stealth probe not supported"})
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	report, err := probeOp.WorkbenchRunStealthProbeSuite(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	creepTrust := extractCreepJSTrust(report)
	target99 := creepTrust >= asymmetric.TargetCreepJSTrust99Plus
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":           true,
		"profileId":    profileID,
		"report":       report,
		"creepTrust":   creepTrust,
		"target99Plus": target99,
	})
}

func extractCreepJSTrust(report map[string]interface{}) float64 {
	if report == nil {
		return 0
	}
	raw, ok := report["creepjs"]
	if !ok {
		return 0
	}
	switch v := raw.(type) {
	case detection.SiteProbeResult:
		return v.TrustScore
	case map[string]interface{}:
		if f, ok := v["trustScore"].(float64); ok {
			return f
		}
	}
	return 0
}

func (s *LaunchServer) handleWorkbenchActivate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	if err := operator.WorkbenchActivateProfile(profileID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "activated": true})
}

func (s *LaunchServer) handleWorkbenchArrange(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}

	var req workbenchArrangeRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileIDs := normalizeStringSlice(req.ProfileIDs)
	if len(profileIDs) == 0 {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileIds is required"})
		return
	}
	layout := strings.TrimSpace(req.Layout)
	if layout == "" {
		layout = "grid"
	}

	placements, err := operator.WorkbenchArrangeProfiles(profileIDs, layout)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{
			"ok":         false,
			"profileIds": profileIDs,
			"layout":     layout,
			"placements": placements,
			"error":      err.Error(),
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":         true,
		"profileIds": profileIDs,
		"layout":     layout,
		"placements": placements,
	})
}

func (s *LaunchServer) workbenchOperator(w http.ResponseWriter) (WorkbenchOperator, bool) {
	operator, ok := s.starter.(WorkbenchOperator)
	if !ok || operator == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "workbench API is not available",
		})
		return nil, false
	}
	return operator, true
}

func (s *LaunchServer) handleWorkbenchClick(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}

	var req workbenchClickRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	selector := strings.TrimSpace(req.Selector)
	if profileID == "" || selector == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId and selector are required"})
		return
	}
	tabID := strings.TrimSpace(req.TabID)
	if tabID != "" {
		results, err := operator.WorkbenchExecuteActions(profileID, []ActionRequest{{
			Type: "click", Selector: selector, TabID: tabID, HumanizationLevel: "high",
		}})
		if err != nil {
			writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "tabId": tabID, "error": err.Error()})
			return
		}
		if len(results) == 0 || !results[0].OK {
			errMsg := "click failed"
			if len(results) > 0 && results[0].Error != "" {
				errMsg = results[0].Error
			}
			writeJSON(w, http.StatusBadGateway, map[string]interface{}{"ok": false, "profileId": profileID, "tabId": tabID, "error": errMsg})
			return
		}
	} else if err := operator.WorkbenchClickElement(profileID, selector); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "selector": selector, "tabId": tabID, "clicked": true})
}

func (s *LaunchServer) handleWorkbenchType(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}

	var req workbenchTypeRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	selector := strings.TrimSpace(req.Selector)
	text := req.Text
	if profileID == "" || selector == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId and selector are required"})
		return
	}
	tabID := strings.TrimSpace(req.TabID)
	if tabID != "" {
		results, err := operator.WorkbenchExecuteActions(profileID, []ActionRequest{{
			Type: "type", Selector: selector, Text: text, TabID: tabID, HumanizationLevel: "high",
		}})
		if err != nil {
			writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "tabId": tabID, "error": err.Error()})
			return
		}
		if len(results) == 0 || !results[0].OK {
			errMsg := "type failed"
			if len(results) > 0 && results[0].Error != "" {
				errMsg = results[0].Error
			}
			writeJSON(w, http.StatusBadGateway, map[string]interface{}{"ok": false, "profileId": profileID, "tabId": tabID, "error": errMsg})
			return
		}
	} else if err := operator.WorkbenchTypeText(profileID, selector, text); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "selector": selector, "tabId": tabID, "typed": true})
}

func (s *LaunchServer) handleWorkbenchScroll(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}

	var req workbenchScrollRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	distance := req.Distance
	if distance == 0 {
		distance = 500
	}
	tabID := strings.TrimSpace(req.TabID)
	if tabID != "" {
		results, err := operator.WorkbenchExecuteActions(profileID, []ActionRequest{{
			Type: "scroll", Distance: distance, TabID: tabID, HumanizationLevel: "low",
		}})
		if err != nil {
			writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "tabId": tabID, "error": err.Error()})
			return
		}
		if len(results) == 0 || !results[0].OK {
			errMsg := "scroll failed"
			if len(results) > 0 && results[0].Error != "" {
				errMsg = results[0].Error
			}
			writeJSON(w, http.StatusBadGateway, map[string]interface{}{"ok": false, "profileId": profileID, "tabId": tabID, "error": errMsg})
			return
		}
	} else if err := operator.WorkbenchScrollPage(profileID, distance); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "distance": distance, "tabId": tabID, "scrolled": true})
}

// ─── New enhanced action endpoints (use unified ActionRequest) ────────────────

type workbenchSingleActionRequest struct {
	ProfileID string          `json:"profileId"`
	Action    ActionRequest   `json:"action"`
	Actions   []ActionRequest `json:"actions"`
}

func (s *LaunchServer) handleWorkbenchHover(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	res := s.executeSingleActionFromBody(w, r, operator, "hover")
	if res == nil {
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true, "profileId": res["profileId"], "action": res["action"],
		"hovered": true, "pageUrl": res["pageUrl"], "pageTitle": res["pageTitle"],
	})
}

func (s *LaunchServer) handleWorkbenchDoubleClick(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	res := s.executeSingleActionFromBody(w, r, operator, "double-click")
	if res == nil {
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true, "profileId": res["profileId"], "action": res["action"],
		"doubleClicked": true, "pageUrl": res["pageUrl"], "pageTitle": res["pageTitle"],
	})
}

func (s *LaunchServer) handleWorkbenchRightClick(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	res := s.executeSingleActionFromBody(w, r, operator, "right-click")
	if res == nil {
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true, "profileId": res["profileId"], "action": res["action"],
		"rightClicked": true, "pageUrl": res["pageUrl"], "pageTitle": res["pageTitle"],
	})
}

func (s *LaunchServer) handleWorkbenchWait(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	res := s.executeSingleActionFromBody(w, r, operator, "wait")
	if res == nil {
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true, "profileId": res["profileId"], "action": res["action"],
		"waited": true, "pageUrl": res["pageUrl"], "pageTitle": res["pageTitle"],
	})
}

func (s *LaunchServer) handleWorkbenchMouseShow(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	if err := operator.WorkbenchShowMousePointer(profileID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "visible": true})
}

func (s *LaunchServer) handleWorkbenchMouseHide(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	if err := operator.WorkbenchHideMousePointer(profileID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "visible": false})
}

func (s *LaunchServer) handleWorkbenchActions(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}

	var req workbenchSingleActionRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	if len(req.Actions) == 0 && req.Action.Type == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "actions or action is required"})
		return
	}

	actions := req.Actions
	if len(actions) == 0 && req.Action.Type != "" {
		actions = []ActionRequest{req.Action}
	}

	// Check for streaming mode
	if strings.ToLower(strings.TrimSpace(r.URL.Query().Get("stream"))) == "1" {
		s.handleWorkbenchActionsStream(w, operator, profileID, actions)
		return
	}

	results, err := operator.WorkbenchExecuteActions(profileID, actions)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{
			"ok": false, "profileId": profileID, "error": err.Error(),
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true, "profileId": profileID, "results": results,
	})
}

// handleWorkbenchActionsStream streams action results as NDJSON (one JSON line per action).
func (s *LaunchServer) handleWorkbenchActionsStream(w http.ResponseWriter, operator WorkbenchOperator, profileID string, actions []ActionRequest) {
	flusher, ok := w.(http.Flusher)
	if !ok {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok": false, "error": "streaming not supported",
		})
		return
	}

	w.Header().Set("Content-Type", "application/x-ndjson")
	w.Header().Set("X-Accel-Buffering", "no")
	w.WriteHeader(http.StatusOK)

	// Write header line
	header := map[string]interface{}{
		"type": "meta", "profileId": profileID, "total": len(actions),
	}
	_ = writeNDJSONLine(w, header)
	flusher.Flush()

	// Execute actions one at a time via the batch executor
	// We must stream results as they come. Currently WorkbenchExecuteActions
	// is all-or-nothing. For true streaming, we execute individual actions.
	for i, action := range actions {
		// Execute single action
		singleActions := []ActionRequest{action}
		results, err := operator.WorkbenchExecuteActions(profileID, singleActions)

		var line map[string]interface{}
		if err != nil {
			line = map[string]interface{}{
				"type": "action", "index": i, "ok": false,
				"action": action, "error": err.Error(),
			}
		} else if len(results) > 0 {
			line = map[string]interface{}{
				"type": "action", "index": i, "ok": results[0].OK,
				"action": action, "result": results[0],
			}
		} else {
			line = map[string]interface{}{
				"type": "action", "index": i, "ok": false,
				"action": action, "error": "no result",
			}
		}
		_ = writeNDJSONLine(w, line)
		flusher.Flush()
	}

	// Write footer
	footer := map[string]interface{}{
		"type": "done", "profileId": profileID, "total": len(actions),
	}
	_ = writeNDJSONLine(w, footer)
	flusher.Flush()
}

func writeNDJSONLine(w http.ResponseWriter, v interface{}) error {
	data, err := json.Marshal(v)
	if err != nil {
		return fmt.Errorf("marshal ndjson: %w", err)
	}
	data = append(data, '\n')
	_, err = w.Write(data)
	return err
}

// executeSingleActionFromBody extracts a profileId + action from the JSON body,
// executes it via WorkbenchExecuteActions, and returns a result map or nil on failure.
// Used by hover/doubleClick/rightClick/wait handlers.
func (s *LaunchServer) executeSingleActionFromBody(w http.ResponseWriter, r *http.Request, operator WorkbenchOperator, defaultType string) map[string]interface{} {
	var req workbenchSingleActionRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return nil
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok": false, "error": "profileId is required",
		})
		return nil
	}
	action := req.Action
	if action.Type == "" {
		action.Type = defaultType
	}

	results, err := operator.WorkbenchExecuteActions(profileID, []ActionRequest{action})
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{
			"ok": false, "profileId": profileID, "error": err.Error(),
		})
		return nil
	}
	if len(results) == 0 {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok": false, "profileId": profileID, "error": "no result returned",
		})
		return nil
	}
	res := results[0]
	if !res.OK {
		writeJSON(w, mapInstanceOperationErrorStatus(errors.New(res.Error)), map[string]interface{}{
			"ok": false, "profileId": profileID, "error": res.Error, "errorCode": res.ErrorCode,
		})
		return nil
	}
	return map[string]interface{}{
		"profileId": profileID, "action": action, "pageUrl": res.PageURL, "pageTitle": res.PageTitle,
	}
}

func decodeWorkbenchProfileID(w http.ResponseWriter, r *http.Request) (string, bool) {
	var req workbenchProfileRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return "", false
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return "", false
	}
	return profileID, true
}

// ─── Identity Report ─────────────────────────────────────────────────────────

func (s *LaunchServer) handleWorkbenchIdentityReport(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	report, err := operator.IdentityReportProfile(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":        true,
		"profileId": profileID,
		"report":    report,
	})
}

// ─── Identity Consistency ────────────────────────────────────────────────────

func (s *LaunchServer) handleWorkbenchIdentityConsistency(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	report, err := operator.IdentityReportProfile(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":             true,
		"profileId":      profileID,
		"coherenceScore": report.Subscores.Consistency,
		"summary":        report.Summary,
	})
}

// ─── Cookies ─────────────────────────────────────────────────────────────────

func (s *LaunchServer) handleWorkbenchCookiesGet(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID, ok2 := decodeWorkbenchProfileID(w, r)
	if !ok2 {
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	cookies, err := operator.WorkbenchGetCookies(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	if cookies == nil {
		cookies = []map[string]interface{}{}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":        true,
		"profileId": profileID,
		"count":     len(cookies),
		"cookies":   cookies,
	})
}

type cookiesSetRequest struct {
	ProfileID string                   `json:"profileId"`
	Cookies   []map[string]interface{} `json:"cookies"`
}

func (s *LaunchServer) handleWorkbenchCookiesSet(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	var req cookiesSetRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	for _, cookie := range req.Cookies {
		if err := operator.WorkbenchSetCookie(profileID, cookie); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "profileId": profileID, "error": "set cookie failed: " + err.Error()})
			return
		}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "set": len(req.Cookies)})
}

func (s *LaunchServer) handleWorkbenchCookiesClear(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	operator, ok2 := s.workbenchOperator(w)
	if !ok2 {
		return
	}
	if err := operator.WorkbenchClearCookies(profileID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "cleared": true})
}

// ─── Tabs ────────────────────────────────────────────────────────────────────

func (s *LaunchServer) handleWorkbenchTabsList(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	operator, ok2 := s.workbenchOperator(w)
	if !ok2 {
		return
	}
	tabs, err := operator.WorkbenchListTabs(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	if tabs == nil {
		tabs = []browser.Tab{}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "count": len(tabs), "tabs": tabs})
}

type tabsActionRequest struct {
	ProfileID string `json:"profileId"`
	TabID     string `json:"tabId,omitempty"`
	URL       string `json:"url,omitempty"`
}

func (s *LaunchServer) handleWorkbenchTabsSwitch(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	var req tabsActionRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" || req.TabID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId and tabId are required"})
		return
	}
	if err := operator.WorkbenchSwitchTab(profileID, req.TabID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "switched": true})
}

func (s *LaunchServer) handleWorkbenchTabsClose(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	var req tabsActionRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" || req.TabID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId and tabId are required"})
		return
	}
	if err := operator.WorkbenchCloseTab(profileID, req.TabID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "closed": true})
}

func (s *LaunchServer) handleWorkbenchTabsNew(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	var req tabsActionRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	tabID, err := operator.WorkbenchNewTab(profileID, req.URL)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "tabId": tabID, "created": true})
}

// ─── Storage ─────────────────────────────────────────────────────────────────

func (s *LaunchServer) handleWorkbenchStorageGet(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	operator, ok2 := s.workbenchOperator(w)
	if !ok2 {
		return
	}
	items, err := operator.WorkbenchGetLocalStorage(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	if items == nil {
		items = map[string]string{}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "count": len(items), "items": items})
}

type storageSetRequest struct {
	ProfileID string            `json:"profileId"`
	Items     map[string]string `json:"items"`
}

func (s *LaunchServer) handleWorkbenchStorageSet(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	var req storageSetRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	if err := operator.WorkbenchSetLocalStorage(profileID, req.Items); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "set": len(req.Items)})
}

func (s *LaunchServer) handleWorkbenchSessionStorageGet(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	operator, ok2 := s.workbenchOperator(w)
	if !ok2 {
		return
	}
	items, err := operator.WorkbenchGetSessionStorage(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	if items == nil {
		items = map[string]string{}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "count": len(items), "items": items})
}

func (s *LaunchServer) handleWorkbenchSessionStorageSet(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	var req storageSetRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	if err := operator.WorkbenchSetSessionStorage(profileID, req.Items); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "set": len(req.Items)})
}

// ─── Behavior Engine ─────────────────────────────────────────────────────────

type behaviorStartRequest struct {
	ProfileID string `json:"profileId"`
	PresetID  string `json:"presetId,omitempty"`
}

func (s *LaunchServer) handleWorkbenchBehaviorStart(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	var req behaviorStartRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	if err := operator.WorkbenchBehaviorStart(profileID, req.PresetID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "behaviorStarted": true})
}

func (s *LaunchServer) handleWorkbenchBehaviorStop(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	operator, ok2 := s.workbenchOperator(w)
	if !ok2 {
		return
	}
	if err := operator.WorkbenchBehaviorStop(profileID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "behaviorStopped": true})
}

type behaviorConfigRequest struct {
	ProfileID string  `json:"profileId"`
	Intensity float64 `json:"intensity"`
}

func (s *LaunchServer) handleWorkbenchBehaviorConfig(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	var req behaviorConfigRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	if err := operator.WorkbenchBehaviorConfig(profileID, req.Intensity); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "intensity": req.Intensity})
}

// ─── Nurture ─────────────────────────────────────────────────────────────────

type nurtureStartRequest struct {
	ProfileID       string `json:"profileId"`
	BehaviorPreset  string `json:"behaviorPreset,omitempty"`
	DurationMinutes int    `json:"durationMinutes,omitempty"`
}

func (s *LaunchServer) handleWorkbenchNurtureStart(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	operator, ok := s.workbenchOperator(w)
	if !ok {
		return
	}
	var req nurtureStartRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	if err := operator.WorkbenchNurtureStart(profileID, req.BehaviorPreset); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "nurtureStarted": true})
}

func (s *LaunchServer) handleWorkbenchNurtureStop(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	operator, ok2 := s.workbenchOperator(w)
	if !ok2 {
		return
	}
	if err := operator.WorkbenchNurtureStop(profileID); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "nurtureStopped": true})
}

// ─── Proxy ───────────────────────────────────────────────────────────────────

func (s *LaunchServer) handleWorkbenchProxyCheck(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	operator, ok2 := s.workbenchOperator(w)
	if !ok2 {
		return
	}
	health, err := operator.WorkbenchCheckProxy(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":        true,
		"profileId": profileID,
		"health":    health,
	})
}

func (s *LaunchServer) handleWorkbenchProxySpeedtest(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID, ok := decodeWorkbenchProfileID(w, r)
	if !ok {
		return
	}
	operator, ok2 := s.workbenchOperator(w)
	if !ok2 {
		return
	}
	result, err := operator.WorkbenchProxySpeedtest(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":        true,
		"profileId": profileID,
		"result":    result,
	})
}
