package launchcode

import (
	"encoding/json"
	"errors"
	"net/http"
	"strconv"
	"strings"

	"ant-chrome/backend/internal/behavior"
	"ant-chrome/backend/internal/logger"
)

var recAPILog = logger.New("RecordingAPI")

// writeRecError writes an error response. Internal errors (5xx) return a generic
// message to avoid leaking debug details; the real error is logged server-side.
func writeRecError(w http.ResponseWriter, status int, err error) {
	msg := err.Error()
	if status >= 500 {
		recAPILog.Error("recording API error", logger.F("status", status), logger.F("error", msg))
		msg = "internal server error"
	}
	writeJSON(w, status, map[string]interface{}{"ok": false, "error": msg})
}

type recordingHTTPStatusCoder interface {
	HTTPStatusCode() int
}

func recordingHTTPStatus(err error, fallback int) int {
	var statusErr recordingHTTPStatusCoder
	if errors.As(err, &statusErr) {
		status := statusErr.HTTPStatusCode()
		if status >= 400 && status <= 599 {
			return status
		}
	}
	if errors.Is(err, behavior.ErrInvalidRecordingID) {
		return http.StatusBadRequest
	}
	if errors.Is(err, behavior.ErrRecordingNotFound) {
		return http.StatusNotFound
	}
	return fallback
}

// RecordingAPI 录制与行为操作接口（由 App 层实现并注入）
type RecordingAPI interface {
	StartRecording(profileId string) error
	StopRecording(profileId string, name string) (*behavior.Recording, error)
	ListRecordings() ([]*behavior.Recording, error)
	ListRecordingSummaries() ([]*behavior.RecordingSummary, error)
	GetRecording(id string) (*behavior.Recording, error)
	GetRecordingDetail(id string, eventOffset int, eventLimit int) (*behavior.RecordingDetailPage, error)
	DeleteRecording(id string) error
	PlayRecording(profileId string, recordingId string, variation behavior.VariationConfig) error
	StopPlayback(profileId string) error
	QuickRecord(profileId string) (*behavior.Recording, error)
	CleanupStaleSessions() error
	ActiveRecordingStatus() (*behavior.ActiveRecordingStatus, error)
	GetBehaviorPresets() []behavior.Profile
}

// ============================================================================
// POST /api/recording/start
// ============================================================================

type recordingStartRequest struct {
	ProfileID string `json:"profileId"`
}

func (s *LaunchServer) handleRecordingStart(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only POST is allowed"})
		return
	}
	var req recordingStartRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid JSON: " + err.Error()})
		return
	}
	if req.ProfileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	if err := s.recording.StartRecording(req.ProfileID); err != nil {
		writeRecError(w, recordingHTTPStatus(err, http.StatusInternalServerError), err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "profileId": req.ProfileID, "recording": true})
}

// ============================================================================
// POST /api/recording/stop
// ============================================================================

type recordingStopRequest struct {
	ProfileID string `json:"profileId"`
	Name      string `json:"name"`
}

func (s *LaunchServer) handleRecordingStop(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only POST is allowed"})
		return
	}
	var req recordingStopRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid JSON: " + err.Error()})
		return
	}
	if req.ProfileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	if req.Name == "" {
		req.Name = "录制 " + strings.TrimSpace(req.ProfileID)
	}
	rec, err := s.recording.StopRecording(req.ProfileID, req.Name)
	if err != nil {
		writeRecError(w, recordingHTTPStatus(err, http.StatusInternalServerError), err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "recording": rec})
}

// ============================================================================
// GET /api/recording/list
// ============================================================================

func (s *LaunchServer) handleRecordingList(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only GET is allowed"})
		return
	}
	list, err := s.recording.ListRecordingSummaries()
	if err != nil {
		writeRecError(w, recordingHTTPStatus(err, http.StatusInternalServerError), err)
		return
	}
	if list == nil {
		list = []*behavior.RecordingSummary{}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "count": len(list), "items": list})
}

// ============================================================================
// GET /api/recording/status
// ============================================================================

func (s *LaunchServer) handleRecordingStatus(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only GET is allowed"})
		return
	}
	status, err := s.recording.ActiveRecordingStatus()
	if err != nil {
		writeRecError(w, recordingHTTPStatus(err, http.StatusInternalServerError), err)
		return
	}
	if status == nil {
		status = &behavior.ActiveRecordingStatus{ProfileIDs: []string{}}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":                    true,
		"active":                status.Active,
		"count":                 status.Count,
		"profileId":             status.ProfileID,
		"profileIds":            status.ProfileIDs,
		"inMemoryProfileIds":    status.InMemoryProfileIDs,
		"recoverableProfileIds": status.RecoverableProfileIDs,
	})
}

// ============================================================================
// GET /api/recording/ (detail by ID)
// DELETE /api/recording/ (delete by ID)
// ============================================================================

func (s *LaunchServer) handleRecordingByID(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}

	// Extract ID from /api/recording/<id>
	id := strings.TrimPrefix(r.URL.Path, "/api/recording/")
	id = strings.TrimSuffix(id, "/")
	if id == "" || id == "list" || id == "start" || id == "stop" || id == "play" || id == "quick" {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "not found"})
		return
	}
	// Reject path traversal attempts
	if strings.Contains(id, "..") || strings.Contains(id, "/") || strings.Contains(id, "\\") {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid recording ID"})
		return
	}

	switch r.Method {
	case http.MethodGet:
		eventOffset, eventLimit, paged, err := recordingEventPageQuery(r)
		if err != nil {
			writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": err.Error()})
			return
		}
		if paged {
			detail, err := s.recording.GetRecordingDetail(id, eventOffset, eventLimit)
			if err != nil {
				writeRecError(w, recordingHTTPStatus(err, http.StatusNotFound), err)
				return
			}
			writeJSON(w, http.StatusOK, map[string]interface{}{
				"ok":          true,
				"recording":   detail.Recording,
				"events":      detail.Events,
				"eventOffset": detail.EventOffset,
				"eventLimit":  detail.EventLimit,
				"eventTotal":  detail.EventTotal,
				"stats":       detail.Stats,
			})
			return
		}

		rec, err := s.recording.GetRecording(id)
		if err != nil {
			writeRecError(w, recordingHTTPStatus(err, http.StatusNotFound), err)
			return
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "recording": rec})

	case http.MethodDelete:
		if err := s.recording.DeleteRecording(id); err != nil {
			writeRecError(w, recordingHTTPStatus(err, http.StatusInternalServerError), err)
			return
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "deleted": true, "recordingId": id})

	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only GET/DELETE are allowed"})
	}
}

func recordingEventPageQuery(r *http.Request) (int, int, bool, error) {
	query := r.URL.Query()
	rawOffset := strings.TrimSpace(query.Get("eventOffset"))
	rawLimit := strings.TrimSpace(query.Get("eventLimit"))
	paged := rawOffset != "" || rawLimit != ""
	if !paged {
		return 0, 0, false, nil
	}

	offset := 0
	limit := 100
	var err error
	if rawOffset != "" {
		offset, err = strconv.Atoi(rawOffset)
		if err != nil || offset < 0 {
			return 0, 0, true, errors.New("eventOffset must be a non-negative integer")
		}
	}
	if rawLimit != "" {
		limit, err = strconv.Atoi(rawLimit)
		if err != nil || limit <= 0 {
			return 0, 0, true, errors.New("eventLimit must be a positive integer")
		}
	}
	return offset, limit, true, nil
}

// ============================================================================
// POST /api/recording/play
// ============================================================================

type recordingPlayRequest struct {
	ProfileID   string                    `json:"profileId"`
	RecordingID string                    `json:"recordingId"`
	Variation   *behavior.VariationConfig `json:"variation"`
}

func (s *LaunchServer) handleRecordingPlay(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only POST is allowed"})
		return
	}
	var req recordingPlayRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid JSON: " + err.Error()})
		return
	}
	if req.ProfileID == "" || req.RecordingID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId and recordingId are required"})
		return
	}
	variation := behavior.VariationConfig{
		Intensity:        0.3,
		TimingJitter:     200,
		PositionJitter:   5,
		SpeedVariation:   0.2,
		MicroCorrections: true,
		ExtraPauses:      true,
	}
	if req.Variation != nil {
		variation = *req.Variation
	}
	if err := s.recording.PlayRecording(req.ProfileID, req.RecordingID, variation); err != nil {
		writeRecError(w, recordingHTTPStatus(err, http.StatusInternalServerError), err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "playing": true, "profileId": req.ProfileID, "recordingId": req.RecordingID})
}

// ============================================================================
// POST /api/recording/play/stop
// ============================================================================

type recordingStopPlayRequest struct {
	ProfileID string `json:"profileId"`
}

func (s *LaunchServer) handleRecordingStopPlay(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only POST is allowed"})
		return
	}
	var req recordingStopPlayRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid JSON: " + err.Error()})
		return
	}
	if req.ProfileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	if err := s.recording.StopPlayback(req.ProfileID); err != nil {
		writeRecError(w, recordingHTTPStatus(err, http.StatusInternalServerError), err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "stopped": true, "profileId": req.ProfileID})
}

// ============================================================================
// POST /api/recording/quick
// ============================================================================

type recordingQuickRequest struct {
	ProfileID string `json:"profileId"`
}

func (s *LaunchServer) handleRecordingQuick(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only POST is allowed"})
		return
	}
	var req recordingQuickRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid JSON: " + err.Error()})
		return
	}
	if req.ProfileID == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "profileId is required"})
		return
	}
	rec, err := s.recording.QuickRecord(req.ProfileID)
	if err != nil {
		writeRecError(w, recordingHTTPStatus(err, http.StatusInternalServerError), err)
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "recording": rec})
}

// ============================================================================
// POST /api/recording/sessions/cleanup
// GET is kept as a deprecated compatibility entry.
// ============================================================================

func (s *LaunchServer) handleRecordingCleanup(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	if r.Method != http.MethodPost && r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only POST is allowed"})
		return
	}
	if err := s.recording.CleanupStaleSessions(); err != nil {
		writeRecError(w, recordingHTTPStatus(err, http.StatusInternalServerError), err)
		return
	}
	payload := map[string]interface{}{"ok": true, "cleanedUp": true}
	if r.Method == http.MethodGet {
		payload["deprecated"] = true
		payload["compatibility"] = "GET is deprecated; use POST /api/recording/sessions/cleanup"
	}
	writeJSON(w, http.StatusOK, payload)
}

// ============================================================================
// GET /api/behavior/presets
// ============================================================================

func (s *LaunchServer) handleBehaviorPresets(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	presets := s.recording.GetBehaviorPresets()
	if presets == nil {
		presets = []behavior.Profile{}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "count": len(presets), "items": presets})
}

// ============================================================================
// GET /api/behavior/presets/{id}
// ============================================================================

func (s *LaunchServer) handleBehaviorPresetByID(w http.ResponseWriter, r *http.Request) {
	if s.recording == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "recording API not available"})
		return
	}
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "only GET is allowed"})
		return
	}

	id := strings.TrimPrefix(r.URL.Path, "/api/behavior/presets/")
	id = strings.TrimSuffix(id, "/")
	if strings.Contains(id, "..") || strings.Contains(id, "/") || strings.Contains(id, "\\") {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "invalid preset ID"})
		return
	}

	presets := s.recording.GetBehaviorPresets()
	for _, p := range presets {
		if p.ID == id {
			writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "preset": p})
			return
		}
	}
	writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "preset not found: " + id})
}
