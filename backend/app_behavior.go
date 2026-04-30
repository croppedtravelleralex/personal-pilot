package backend

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"ant-chrome/backend/internal/behavior"
	"ant-chrome/backend/internal/events"
)

type recordingAppError struct {
	status  int
	message string
	err     error
}

func (e recordingAppError) Error() string {
	if e.err != nil {
		return e.message + ": " + e.err.Error()
	}
	return e.message
}

func (e recordingAppError) Unwrap() error {
	return e.err
}

func (e recordingAppError) HTTPStatusCode() int {
	return e.status
}

type recordingSession struct {
	ProfileId string `json:"profileId"`
	DebugPort int    `json:"debugPort"`
	StartedAt string `json:"startedAt"`
}

// BehaviorPresetInfo is the frontend-facing preset metadata.
type BehaviorPresetInfo struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
}

func recordingError(status int, message string, err error) error {
	return recordingAppError{status: status, message: message, err: err}
}

func (a *App) requireRecordingStore() (*behavior.FileRecordingStore, error) {
	if a == nil || a.recordingStore == nil {
		return nil, recordingError(http.StatusServiceUnavailable, "recording store unavailable", nil)
	}
	return a.recordingStore, nil
}

// BehaviorPresetList returns all available behavior presets.
func (a *App) BehaviorPresetList() []BehaviorPresetInfo {
	presets := behavior.BuiltinPresets()
	result := make([]BehaviorPresetInfo, len(presets))
	for i, p := range presets {
		result[i] = BehaviorPresetInfo{
			ID:          p.ID,
			Name:        p.Name,
			Description: p.Description,
		}
	}
	return result
}

func (a *App) GetBehaviorPresets() []behavior.Profile {
	return behavior.BuiltinPresets()
}

func (a *App) StartRecording(profileId string) error {
	return a.BehaviorStartRecording(profileId)
}

func (a *App) StopRecording(profileId string, name string) (*behavior.Recording, error) {
	return a.BehaviorStopRecording(profileId, name)
}

func (a *App) ListRecordings() ([]*behavior.Recording, error) {
	return a.BehaviorRecordingList()
}

func (a *App) ListRecordingSummaries() ([]*behavior.RecordingSummary, error) {
	return a.BehaviorRecordingSummaryList()
}

func (a *App) GetRecording(id string) (*behavior.Recording, error) {
	return a.BehaviorGetRecording(id)
}

func (a *App) GetRecordingDetail(id string, eventOffset int, eventLimit int) (*behavior.RecordingDetailPage, error) {
	return a.BehaviorGetRecordingDetail(id, eventOffset, eventLimit)
}

func (a *App) DeleteRecording(id string) error {
	return a.BehaviorRecordingDelete(id)
}

func (a *App) PlayRecording(profileId string, recordingId string, variation behavior.VariationConfig) error {
	return a.BehaviorPlayRecording(profileId, recordingId, variation)
}

func (a *App) StopPlayback(profileId string) error {
	return a.BehaviorStopPlayback(profileId)
}

func (a *App) QuickRecord(profileId string) (*behavior.Recording, error) {
	return a.BehaviorQuickRecord(profileId)
}

func (a *App) CleanupStaleSessions() error {
	return a.CleanupStaleRecordingSessions()
}

func (a *App) BehaviorRecordingStatus() (*behavior.ActiveRecordingStatus, error) {
	return a.ActiveRecordingStatus()
}

func (a *App) initRecordingSessionDir(recordingDir string) error {
	recordingDir = strings.TrimSpace(recordingDir)
	if recordingDir == "" {
		return fmt.Errorf("recording directory is required")
	}
	sessionDir := filepath.Join(recordingDir, ".sessions")
	if err := os.MkdirAll(sessionDir, 0755); err != nil {
		return err
	}
	a.recordingSessionDir = sessionDir
	return nil
}

func (a *App) sessionFilePath(profileId string) (string, error) {
	if strings.TrimSpace(a.recordingSessionDir) == "" {
		return "", fmt.Errorf("recording session directory unavailable")
	}
	if strings.TrimSpace(profileId) == "" {
		return "", fmt.Errorf("profileId is required")
	}
	name := base64.RawURLEncoding.EncodeToString([]byte(profileId)) + ".json"
	path := filepath.Join(a.recordingSessionDir, name)
	cleanBase := filepath.Clean(a.recordingSessionDir)
	cleanPath := filepath.Clean(path)
	rel, err := filepath.Rel(cleanBase, cleanPath)
	if err != nil {
		return "", err
	}
	if strings.HasPrefix(rel, "..") || filepath.IsAbs(rel) {
		return "", fmt.Errorf("invalid session path")
	}
	return cleanPath, nil
}

func (a *App) saveRecordingSession(profileId string, debugPort int) error {
	path, err := a.sessionFilePath(profileId)
	if err != nil {
		return err
	}
	payload := recordingSession{
		ProfileId: profileId,
		DebugPort: debugPort,
		StartedAt: time.Now().Format(time.RFC3339),
	}
	data, err := json.MarshalIndent(payload, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0644)
}

func (a *App) loadRecordingSession(profileId string) (*recordingSession, error) {
	path, err := a.sessionFilePath(profileId)
	if err != nil {
		return nil, err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var session recordingSession
	if err := json.Unmarshal(data, &session); err != nil {
		return nil, err
	}
	return &session, nil
}

func (a *App) deleteRecordingSession(profileId string) error {
	path, err := a.sessionFilePath(profileId)
	if err != nil {
		return err
	}
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return err
	}
	return nil
}

func (a *App) listRecordingSessions() ([]*recordingSession, error) {
	if strings.TrimSpace(a.recordingSessionDir) == "" {
		return nil, nil
	}
	entries, err := os.ReadDir(a.recordingSessionDir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	sessions := make([]*recordingSession, 0, len(entries))
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}
		data, err := os.ReadFile(filepath.Join(a.recordingSessionDir, entry.Name()))
		if err != nil {
			continue
		}
		var session recordingSession
		if err := json.Unmarshal(data, &session); err != nil {
			continue
		}
		if strings.TrimSpace(session.ProfileId) != "" {
			sessions = append(sessions, &session)
		}
	}
	return sessions, nil
}

func (a *App) isProfileRunning(profileId string) bool {
	if a == nil || a.browserMgr == nil {
		return false
	}
	a.browserMgr.Mutex.Lock()
	defer a.browserMgr.Mutex.Unlock()
	profile := a.browserMgr.Profiles[profileId]
	return profile != nil && profile.Running
}

func (a *App) profileDebugPort(profileId string) (int, error) {
	if a == nil || a.browserMgr == nil {
		return 0, recordingError(http.StatusServiceUnavailable, "browser manager unavailable", nil)
	}
	a.browserMgr.Mutex.Lock()
	profile := a.browserMgr.Profiles[profileId]
	if profile == nil {
		a.browserMgr.Mutex.Unlock()
		return 0, recordingError(http.StatusNotFound, "profile not found", nil)
	}
	if !profile.Running {
		a.browserMgr.Mutex.Unlock()
		return 0, recordingError(http.StatusConflict, "browser not running", nil)
	}
	if !profile.DebugReady || profile.DebugPort <= 0 {
		a.browserMgr.Mutex.Unlock()
		return 0, recordingError(http.StatusConflict, "debug port not ready", nil)
	}
	snapshot := *profile
	a.browserMgr.Mutex.Unlock()
	if err := a.validateProfileCDPOwnership(&snapshot); err != nil {
		return 0, recordingError(http.StatusConflict, err.Error(), nil)
	}
	return snapshot.DebugPort, nil
}

func (a *App) runningRecordingProfile(profileId string) (*BrowserProfile, error) {
	if a == nil || a.browserMgr == nil {
		return nil, recordingError(http.StatusServiceUnavailable, "browser manager unavailable", nil)
	}
	a.browserMgr.Mutex.Lock()
	defer a.browserMgr.Mutex.Unlock()
	profile := a.browserMgr.Profiles[profileId]
	if profile == nil {
		return nil, recordingError(http.StatusNotFound, "profile not found", nil)
	}
	if !profile.Running {
		return nil, recordingError(http.StatusConflict, "browser not running", nil)
	}
	return profile, nil
}

// BehaviorStartRecording starts recording user interactions on a running browser instance.
func (a *App) BehaviorStartRecording(profileId string) error {
	if _, err := a.requireRecordingStore(); err != nil {
		return err
	}
	debugPort, err := a.profileDebugPort(profileId)
	if err != nil {
		return err
	}

	a.recMu.Lock()
	defer a.recMu.Unlock()
	if a.recorders == nil {
		a.recorders = make(map[string]*behavior.Recorder)
	}
	if _, exists := a.recorders[profileId]; exists {
		return recordingError(http.StatusConflict, "already recording", nil)
	}

	rec := behavior.NewRecorder()
	if err := rec.StartRecording(debugPort); err != nil {
		return recordingError(http.StatusInternalServerError, "start recording", err)
	}

	a.recorders[profileId] = rec
	if err := a.saveRecordingSession(profileId, debugPort); err != nil {
		delete(a.recorders, profileId)
		return recordingError(http.StatusInternalServerError, "save recording session", err)
	}
	return nil
}

// BehaviorStopRecording stops recording and persists the result.
func (a *App) BehaviorStopRecording(profileId string, name string) (*behavior.Recording, error) {
	store, err := a.requireRecordingStore()
	if err != nil {
		return nil, err
	}

	a.recMu.Lock()
	rec, exists := a.recorders[profileId]
	if !exists {
		a.recMu.Unlock()
		return nil, recordingError(http.StatusConflict, "no active recording", nil)
	}
	delete(a.recorders, profileId)
	a.recMu.Unlock()

	recording, err := rec.StopRecording(name)
	if err != nil {
		return nil, recordingError(http.StatusInternalServerError, "stop recording", err)
	}
	if err := store.Save(recording); err != nil {
		return nil, recordingError(http.StatusInternalServerError, "save recording", err)
	}
	_ = a.deleteRecordingSession(profileId)
	return recording, nil
}

func (a *App) BehaviorQuickRecord(profileId string) (*behavior.Recording, error) {
	if err := a.BehaviorStartRecording(profileId); err != nil {
		return nil, err
	}
	time.Sleep(2 * time.Second)
	return a.BehaviorStopRecording(profileId, "quick recording")
}

// BehaviorRecordingList returns all saved recordings.
func (a *App) BehaviorRecordingList() ([]*behavior.Recording, error) {
	store, err := a.requireRecordingStore()
	if err != nil {
		return nil, err
	}
	return store.List()
}

func (a *App) BehaviorRecordingSummaryList() ([]*behavior.RecordingSummary, error) {
	store, err := a.requireRecordingStore()
	if err != nil {
		return nil, err
	}
	return store.ListSummaries()
}

// BehaviorRecordingDelete deletes a recording by ID.
func (a *App) BehaviorRecordingDelete(id string) error {
	store, err := a.requireRecordingStore()
	if err != nil {
		return err
	}
	if err := store.Delete(id); err != nil {
		return recordingError(recordingStoreStatus(err), "delete recording", err)
	}
	return nil
}

// BehaviorGetRecording returns a single recording by ID.
func (a *App) BehaviorGetRecording(id string) (*behavior.Recording, error) {
	store, err := a.requireRecordingStore()
	if err != nil {
		return nil, err
	}
	rec, err := store.Get(id)
	if err != nil {
		return nil, recordingError(recordingStoreStatus(err), "get recording", err)
	}
	return rec, nil
}

func (a *App) BehaviorGetRecordingDetail(id string, eventOffset int, eventLimit int) (*behavior.RecordingDetailPage, error) {
	store, err := a.requireRecordingStore()
	if err != nil {
		return nil, err
	}
	detail, err := store.GetDetailPage(id, eventOffset, eventLimit)
	if err != nil {
		return nil, recordingError(recordingStoreStatus(err), "get recording detail", err)
	}
	return detail, nil
}

func (a *App) BehaviorRecordingRename(id string, name string) error {
	store, err := a.requireRecordingStore()
	if err != nil {
		return err
	}
	if err := store.Rename(id, strings.TrimSpace(name)); err != nil {
		return recordingError(recordingStoreStatus(err), "rename recording", err)
	}
	return nil
}

func (a *App) BehaviorRecordingExport(id string) (*behavior.RecordingExportBundle, error) {
	rec, err := a.BehaviorGetRecording(id)
	if err != nil {
		return nil, err
	}
	return behavior.NewRecordingExportBundle(rec), nil
}

func (a *App) BehaviorRecordingImport(payload string, name string) (*behavior.Recording, error) {
	store, err := a.requireRecordingStore()
	if err != nil {
		return nil, err
	}
	rec, err := behavior.NewRecordingFromImportPayload(payload, name)
	if err != nil {
		return nil, recordingError(http.StatusBadRequest, "import recording", err)
	}
	if err := store.Save(rec); err != nil {
		return nil, recordingError(http.StatusInternalServerError, "save imported recording", err)
	}
	return rec, nil
}

func (a *App) BehaviorRecordingCopy(id string, name string) (*behavior.Recording, error) {
	store, err := a.requireRecordingStore()
	if err != nil {
		return nil, err
	}
	source, err := store.Get(id)
	if err != nil {
		return nil, recordingError(recordingStoreStatus(err), "get source recording", err)
	}
	rec := behavior.NewRecordingCopy(source, name)
	if err := store.Save(rec); err != nil {
		return nil, recordingError(http.StatusInternalServerError, "save copied recording", err)
	}
	return rec, nil
}

func (a *App) BehaviorRecordingTrim(id string, startEvent int, endEvent int, name string) (*behavior.Recording, error) {
	store, err := a.requireRecordingStore()
	if err != nil {
		return nil, err
	}
	source, err := store.Get(id)
	if err != nil {
		return nil, recordingError(recordingStoreStatus(err), "get source recording", err)
	}
	rec, err := behavior.NewRecordingTrim(source, startEvent, endEvent, name)
	if err != nil {
		return nil, recordingError(http.StatusBadRequest, "trim recording", err)
	}
	if err := store.Save(rec); err != nil {
		return nil, recordingError(http.StatusInternalServerError, "save trimmed recording", err)
	}
	return rec, nil
}

func recordingStoreStatus(err error) int {
	switch {
	case errors.Is(err, behavior.ErrInvalidRecordingID):
		return http.StatusBadRequest
	case errors.Is(err, behavior.ErrRecordingNotFound):
		return http.StatusNotFound
	default:
		return http.StatusInternalServerError
	}
}

// BehaviorPlayRecording starts playing a recording on a running browser instance.
func (a *App) BehaviorPlayRecording(profileId string, recordingId string, variation behavior.VariationConfig) error {
	store, err := a.requireRecordingStore()
	if err != nil {
		return err
	}
	debugPort, err := a.profileDebugPort(profileId)
	if err != nil {
		return err
	}

	a.playMu.Lock()
	if a.playbacks == nil {
		a.playbacks = make(map[string]*behavior.PlaybackEngine)
	}
	if _, exists := a.playbacks[profileId]; exists {
		a.playMu.Unlock()
		return recordingError(http.StatusConflict, "already playing", nil)
	}
	recording, err := store.Get(recordingId)
	if err != nil {
		a.playMu.Unlock()
		return recordingError(recordingStoreStatus(err), "get recording", err)
	}

	engine := behavior.NewPlaybackEngine(recording, variation)
	engine.SetProgressCallback(func(progress behavior.PlaybackProgress) {
		if a.ctx != nil {
			a.emit(events.EventAutomationPlaybackProgress, progress)
		}
	})
	a.playbacks[profileId] = engine
	a.playMu.Unlock()

	go func() {
		playErr := engine.Play(a.ctx, debugPort)
		if a.ctx != nil {
			if playErr != nil {
				a.emit(events.EventAutomationPlaybackFailed, map[string]interface{}{
					"profileId":   profileId,
					"recordingId": recordingId,
					"error":       playErr.Error(),
				})
			} else {
				a.emit(events.EventAutomationPlaybackCompleted, map[string]interface{}{
					"profileId":   profileId,
					"recordingId": recordingId,
				})
			}
		}
		a.playMu.Lock()
		delete(a.playbacks, profileId)
		a.playMu.Unlock()
	}()

	return nil
}

// BehaviorStopPlayback stops an in-progress playback.
func (a *App) BehaviorStopPlayback(profileId string) error {
	if _, err := a.requireRecordingStore(); err != nil {
		return err
	}
	a.playMu.Lock()
	engine, exists := a.playbacks[profileId]
	if !exists {
		a.playMu.Unlock()
		return recordingError(http.StatusConflict, "no active playback", nil)
	}
	delete(a.playbacks, profileId)
	a.playMu.Unlock()

	engine.Stop()
	return nil
}

func (a *App) BehaviorPlaybackReview(profileId string, decision string) error {
	if _, err := a.requireRecordingStore(); err != nil {
		return err
	}
	a.playMu.Lock()
	engine, exists := a.playbacks[profileId]
	a.playMu.Unlock()
	if !exists {
		return recordingError(http.StatusConflict, "no active playback", nil)
	}
	if err := engine.SubmitReviewDecision(decision); err != nil {
		return recordingError(http.StatusBadRequest, "review playback", err)
	}
	return nil
}

func (a *App) CleanupStaleRecordingSessions() error {
	if _, err := a.requireRecordingStore(); err != nil {
		return err
	}
	sessions, err := a.listRecordingSessions()
	if err != nil {
		return recordingError(http.StatusInternalServerError, "list recording sessions", err)
	}
	for _, session := range sessions {
		if !a.isProfileRunning(session.ProfileId) {
			_ = a.deleteRecordingSession(session.ProfileId)
		}
	}
	return nil
}

func (a *App) ActiveRecordingStatus() (*behavior.ActiveRecordingStatus, error) {
	if _, err := a.requireRecordingStore(); err != nil {
		return nil, err
	}

	inMemory := make(map[string]struct{})
	a.recMu.Lock()
	for profileID := range a.recorders {
		if a.isProfileRunning(profileID) {
			inMemory[profileID] = struct{}{}
		}
	}
	a.recMu.Unlock()

	recoverable := make(map[string]struct{})
	sessions, err := a.listRecordingSessions()
	if err != nil {
		return nil, recordingError(http.StatusInternalServerError, "list recording sessions", err)
	}
	for _, session := range sessions {
		if a.isProfileRunning(session.ProfileId) {
			recoverable[session.ProfileId] = struct{}{}
		}
	}

	profileSet := make(map[string]struct{})
	for id := range inMemory {
		profileSet[id] = struct{}{}
	}
	for id := range recoverable {
		profileSet[id] = struct{}{}
	}

	profileIDs := sortedStringKeys(profileSet)
	inMemoryIDs := sortedStringKeys(inMemory)
	recoverableIDs := sortedStringKeys(recoverable)
	status := &behavior.ActiveRecordingStatus{
		Active:                len(profileIDs) > 0,
		Count:                 len(profileIDs),
		ProfileIDs:            profileIDs,
		InMemoryProfileIDs:    inMemoryIDs,
		RecoverableProfileIDs: recoverableIDs,
	}
	if len(profileIDs) > 0 {
		status.ProfileID = profileIDs[0]
	}
	return status, nil
}

func sortedStringKeys(values map[string]struct{}) []string {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}
