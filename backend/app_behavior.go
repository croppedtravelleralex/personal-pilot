package backend

import (
	"fmt"
	"ant-chrome/backend/internal/behavior"
)

// BehaviorPresetInfo is the frontend-facing preset metadata.
type BehaviorPresetInfo struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
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

// ─── Recording API ──────────────────────────────────────────────

// BehaviorStartRecording starts recording user interactions on a running browser instance.
func (a *App) BehaviorStartRecording(profileId string) error {
	a.recMu.Lock()
	defer a.recMu.Unlock()

	if _, exists := a.recorders[profileId]; exists {
		return fmt.Errorf("already recording on profile %s", profileId)
	}

	a.browserMgr.Mutex.Lock()
	profile, exists := a.browserMgr.Profiles[profileId]
	a.browserMgr.Mutex.Unlock()
	if !exists || profile == nil {
		return fmt.Errorf("profile not found: %s", profileId)
	}
	if !profile.Running {
		return fmt.Errorf("browser not running for profile %s", profileId)
	}

	rec := behavior.NewRecorder()
	if err := rec.StartRecording(profile.DebugPort); err != nil {
		return fmt.Errorf("start recording: %w", err)
	}

	a.recorders[profileId] = rec
	return nil
}

// BehaviorStopRecording stops recording and persists the result.
func (a *App) BehaviorStopRecording(profileId string, name string) (*behavior.Recording, error) {
	a.recMu.Lock()
	rec, exists := a.recorders[profileId]
	if !exists {
		a.recMu.Unlock()
		return nil, fmt.Errorf("no active recording on profile %s", profileId)
	}
	delete(a.recorders, profileId)
	a.recMu.Unlock()

	recording, err := rec.StopRecording(name)
	if err != nil {
		return nil, fmt.Errorf("stop recording: %w", err)
	}

	if err := a.recordingStore.Save(recording); err != nil {
		return nil, fmt.Errorf("save recording: %w", err)
	}

	return recording, nil
}

// BehaviorRecordingList returns all saved recordings.
func (a *App) BehaviorRecordingList() ([]*behavior.Recording, error) {
	return a.recordingStore.List()
}

// BehaviorRecordingDelete deletes a recording by ID.
func (a *App) BehaviorRecordingDelete(id string) error {
	return a.recordingStore.Delete(id)
}

// BehaviorGetRecording returns a single recording by ID.
func (a *App) BehaviorGetRecording(id string) (*behavior.Recording, error) {
	return a.recordingStore.Get(id)
}

// ─── Playback API ──────────────────────────────────────────────

// BehaviorPlayRecording starts playing a recording on a running browser instance.
func (a *App) BehaviorPlayRecording(profileId string, recordingId string, variation behavior.VariationConfig) error {
	a.playMu.Lock()
	defer a.playMu.Unlock()

	if _, exists := a.playbacks[profileId]; exists {
		return fmt.Errorf("already playing on profile %s", profileId)
	}

	a.browserMgr.Mutex.Lock()
	profile, exists := a.browserMgr.Profiles[profileId]
	a.browserMgr.Mutex.Unlock()
	if !exists || profile == nil {
		return fmt.Errorf("profile not found: %s", profileId)
	}
	if !profile.Running {
		return fmt.Errorf("browser not running for profile %s", profileId)
	}

	recording, err := a.recordingStore.Get(recordingId)
	if err != nil {
		return fmt.Errorf("get recording: %w", err)
	}

	engine := behavior.NewPlaybackEngine(recording, variation)
	a.playbacks[profileId] = engine

	// Run playback in background so the Wails call returns immediately
	go func() {
		if err := engine.Play(a.ctx, profile.DebugPort); err != nil {
			// Log error but don't crash
			_ = err
		}
		a.playMu.Lock()
		delete(a.playbacks, profileId)
		a.playMu.Unlock()
	}()

	return nil
}

// BehaviorStopPlayback stops an in-progress playback.
func (a *App) BehaviorStopPlayback(profileId string) error {
	a.playMu.Lock()
	engine, exists := a.playbacks[profileId]
	if !exists {
		a.playMu.Unlock()
		return fmt.Errorf("no active playback on profile %s", profileId)
	}
	delete(a.playbacks, profileId)
	a.playMu.Unlock()

	engine.Stop()
	return nil
}
