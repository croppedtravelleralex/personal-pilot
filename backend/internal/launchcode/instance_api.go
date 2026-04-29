package launchcode

import (
	"encoding/json"
	"io"
	"net/http"
	"strings"
)

type instanceStopRequest struct {
	ProfileID string `json:"profileId"`
}

func (s *LaunchServer) handleInstanceStop(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}

	var req instanceStopRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{
			"ok":    false,
			"error": errMsg,
		})
		return
	}

	profileID := strings.TrimSpace(req.ProfileID)
	if profileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "profileId is required",
		})
		return
	}

	stopper, ok := s.starter.(BrowserStopper)
	if !ok || stopper == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "browser stop API is not available",
		})
		return
	}

	profile, err := stopper.StopInstance(profileID)
	if err != nil {
		writeJSON(w, mapInstanceOperationErrorStatus(err), map[string]interface{}{
			"ok":        false,
			"error":     err.Error(),
			"profileId": profileID,
		})
		return
	}
	s.ClearActiveProfile(profileID)

	profileName := ""
	if profile != nil {
		profileName = profile.ProfileName
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":          true,
		"stopped":     true,
		"profileId":   profileID,
		"profileName": profileName,
		"profile":     profile,
	})
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
