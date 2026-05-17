package launchcode

import (
	"encoding/json"
	"io"
	"net/http"
	"strings"
	"time"

	"personal-pilot/backend/internal/browser"
)

type instanceStopRequest struct {
	ProfileID string `json:"profileId"`
}

func (s *LaunchServer) handleInstanceStop(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	var req instanceStopRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	stopper, ok := s.starter.(BrowserStopper)
	if !ok || stopper == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "browser stop API is not available"})
		return
	}
	profile, err := stopper.StopInstance(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "error": err.Error(), "profileId": profileID})
		return
	}
	s.ClearActiveProfile(profileID)
	profileName := ""
	if profile != nil {
		profileName = profile.ProfileName
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "stopped": true, "profileId": profileID, "profileName": profileName, "profile": profile})
}

// ─── Instance Status ──────────────────────────────────────────────────────────

func (s *LaunchServer) handleInstanceStatus(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	profileID := strings.TrimSpace(r.URL.Query().Get("profileId"))
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId query param is required"})
		return
	}
	stater, ok := s.starter.(InstanceStater)
	if ok && stater != nil {
		profile, err := stater.GetInstanceStatus(profileID)
		if err != nil {
			writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": err.Error()})
			return
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{
			"ok": true, "profileId": profileID,
			"running": profile.Running, "pid": profile.Pid,
			"debugPort": profile.DebugPort, "debugReady": profile.DebugReady,
			"profile": profile,
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true, "profileId": profileID, "profile": s.managerProfileStatus(profileID),
	})
}

// ─── Instance Copy ────────────────────────────────────────────────────────────

type instanceCopyRequest struct {
	ProfileID string `json:"profileId"`
	NewName   string `json:"newName,omitempty"`
}

func (s *LaunchServer) handleInstanceCopy(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	var req instanceCopyRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	operator, ok := s.starter.(InstanceOperator)
	if ok && operator != nil {
		profile, err := operator.CopyProfile(profileID, req.NewName)
		if err != nil {
			writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "error": err.Error()})
			return
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profile.ProfileId, "profileName": profile.ProfileName, "profile": profile})
		return
	}
	if s.browserMgr != nil {
		profile, err := s.browserMgr.Copy(profileID, req.NewName)
		if err != nil {
			writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{"ok": false, "error": err.Error()})
			return
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profile.ProfileId, "profileName": profile.ProfileName, "profile": profile})
		return
	}
	writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "instance copy API is not available"})
}

// ─── Instance Restart ────────────────────────────────────────────────────────

type instanceRestartRequest struct {
	ProfileID string `json:"profileId"`
}

func (s *LaunchServer) handleInstanceRestart(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	var req instanceRestartRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	stopper, ok := s.starter.(BrowserStopper)
	if !ok || stopper == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "browser restart API is not available (stopper)"})
		return
	}
	starter, ok2 := s.starter.(BrowserStarter)
	if !ok2 || starter == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "browser restart API is not available (starter)"})
		return
	}
	_, stopErr := stopper.StopInstance(profileID)
	if stopErr != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(stopErr), map[string]interface{}{"ok": false, "error": "stop failed: " + stopErr.Error()})
		return
	}
	s.ClearActiveProfile(profileID)
	time.Sleep(500 * time.Millisecond)
	startProfile, startErr := starter.StartInstance(profileID)
	if startErr != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "error": "restart start failed: " + startErr.Error(), "stopped": true})
		return
	}
	s.SetActiveProfile(startProfile)
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "restarted": true, "profileId": profileID, "profile": startProfile})
}

// ─── Instance by ID (GET for status, POST with ?action=) ─────────────────────

func (s *LaunchServer) handleInstanceByID(w http.ResponseWriter, r *http.Request) {
	profileID := strings.TrimPrefix(r.URL.Path, "/api/instances/")
	profileID = strings.TrimSpace(profileID)
	if profileID == "" || strings.Contains(profileID, "/") {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "not found"})
		return
	}
	switch r.Method {
	case http.MethodGet:
		stater, ok := s.starter.(InstanceStater)
		if ok && stater != nil {
			profile, err := stater.GetInstanceStatus(profileID)
			if err != nil {
				writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": err.Error()})
				return
			}
			writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "running": profile.Running, "pid": profile.Pid, "debugPort": profile.DebugPort, "debugReady": profile.DebugReady, "profile": profile})
			return
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": profileID, "status": s.managerProfileStatus(profileID)})
	case http.MethodPost:
		action := strings.TrimSpace(r.URL.Query().Get("action"))
		switch action {
		case "start":
			starter, ok := s.starter.(BrowserStarter)
			if !ok || starter == nil {
				writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "start not available"})
				return
			}
			profile, err := starter.StartInstance(profileID)
			if err != nil {
				writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "error": err.Error()})
				return
			}
			s.SetActiveProfile(profile)
			writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "started": true, "profileId": profileID, "profile": profile})
		case "stop":
			stopper, ok := s.starter.(BrowserStopper)
			if !ok || stopper == nil {
				writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "stop not available"})
				return
			}
			_, stopErr := stopper.StopInstance(profileID)
			if stopErr != nil {
				writeJSON(w, mapInstanceOperationErrorStatus(stopErr), map[string]interface{}{"ok": false, "error": stopErr.Error()})
				return
			}
			s.ClearActiveProfile(profileID)
			writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "stopped": true, "profileId": profileID})
		case "restart":
			s.handleInstanceRestart(w, r)
		default:
			writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "unknown action: " + action + " (supported: start, stop, restart)"})
		}
	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
	}
}

// ─── Batch Operations ────────────────────────────────────────────────────────

type instanceBatchRequest struct {
	ProfileIDs []string `json:"profileIds"`
}

func (s *LaunchServer) handleInstanceBatchStart(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	var req instanceBatchRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	if len(req.ProfileIDs) == 0 {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileIds is required"})
		return
	}
	starter, ok := s.starter.(BrowserStarter)
	if !ok || starter == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "batch start not available"})
		return
	}
	type batchItem struct {
		ProfileID string `json:"profileId"`
		OK        bool   `json:"ok"`
		Error     string `json:"error,omitempty"`
		Pid       int    `json:"pid,omitempty"`
		DebugPort int    `json:"debugPort,omitempty"`
	}
	results := make([]batchItem, 0, len(req.ProfileIDs))
	var lastProfile *browser.Profile
	for _, pid := range req.ProfileIDs {
		pid = strings.TrimSpace(pid)
		if pid == "" {
			continue
		}
		profile, err := starter.StartInstance(pid)
		item := batchItem{ProfileID: pid}
		if err != nil {
			item.Error = err.Error()
		} else {
			item.OK = true
			item.Pid = profile.Pid
			item.DebugPort = profile.DebugPort
			lastProfile = profile
		}
		results = append(results, item)
	}
	if lastProfile != nil {
		s.SetActiveProfile(lastProfile)
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "count": len(results), "results": results})
}

func (s *LaunchServer) handleInstanceBatchStop(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	var req instanceBatchRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	if len(req.ProfileIDs) == 0 {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileIds is required"})
		return
	}
	stopper, ok := s.starter.(BrowserStopper)
	if !ok || stopper == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "batch stop not available"})
		return
	}
	type batchItem struct {
		ProfileID string `json:"profileId"`
		OK        bool   `json:"ok"`
		Error     string `json:"error,omitempty"`
	}
	results := make([]batchItem, 0, len(req.ProfileIDs))
	for _, pid := range req.ProfileIDs {
		pid = strings.TrimSpace(pid)
		if pid == "" {
			continue
		}
		_, err := stopper.StopInstance(pid)
		s.ClearActiveProfile(pid)
		results = append(results, batchItem{ProfileID: pid, OK: err == nil, Error: func() string { if err != nil { return err.Error() }; return "" }()})
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "count": len(results), "results": results})
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

func (s *LaunchServer) managerProfileStatus(profileID string) map[string]interface{} {
	if s.browserMgr == nil {
		return map[string]interface{}{"profileId": profileID, "error": "manager not available"}
	}
	s.browserMgr.Mutex.Lock()
	profile, ok := s.browserMgr.Profiles[profileID]
	s.browserMgr.Mutex.Unlock()
	if !ok {
		return map[string]interface{}{"profileId": profileID, "error": "profile not found"}
	}
	return map[string]interface{}{
		"profileId": profile.ProfileId, "profileName": profile.ProfileName,
		"running": profile.Running, "pid": profile.Pid,
		"debugPort": profile.DebugPort, "debugReady": profile.DebugReady,
		"lastError": profile.LastError,
	}
}

func decodeLimitedJSONBody(r *http.Request, dst interface{}) (int, string) {
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(dst); err != nil {
		return http.StatusBadRequest, "invalid request body"
	}
	return http.StatusOK, ""
}

func mapInstanceOperationErrorStatus(err error) int {
	if err == nil {
		return http.StatusOK
	}
	msg := strings.ToLower(strings.TrimSpace(err.Error()))
	switch {
	case msg == "":
		return http.StatusInternalServerError
	case strings.Contains(msg, "profile not found"),
		strings.Contains(msg, "instance not found"),
		strings.Contains(msg, "not found"),
		strings.Contains(msg, "不存在"):
		return http.StatusNotFound
	case strings.Contains(msg, "not running"),
		strings.Contains(msg, "debug port not ready"),
		strings.Contains(msg, "cdp ownership rejected"),
		strings.Contains(msg, "not ready"),
		strings.Contains(msg, "未运行"),
		strings.Contains(msg, "未就绪"):
		return http.StatusConflict
	case strings.Contains(msg, strings.ToLower(strings.TrimSpace(http.ErrNotSupported.Error()))):
		return http.StatusServiceUnavailable
	default:
		return http.StatusInternalServerError
	}
}
