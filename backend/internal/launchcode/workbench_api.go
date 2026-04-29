package launchcode

import (
	"net/http"
	"strings"
)

type workbenchProfileRequest struct {
	ProfileID string `json:"profileId"`
}

type workbenchNavigateRequest struct {
	ProfileID string `json:"profileId"`
	URL       string `json:"url"`
}

type workbenchArrangeRequest struct {
	ProfileIDs []string `json:"profileIds"`
	Layout     string   `json:"layout"`
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
	if err := operator.WorkbenchNavigateProfile(profileID, targetURL); err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "profileId": profileID, "error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "url": targetURL})
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
