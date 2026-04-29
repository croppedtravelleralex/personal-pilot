package backend

import (
	"encoding/json"
	"errors"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"ant-chrome/backend/internal/behavior"
	"ant-chrome/backend/internal/browser"
	"ant-chrome/backend/internal/config"
)

type httpStatusError interface {
	HTTPStatusCode() int
}

func TestBehaviorRecordingAPIsReturnStoreUnavailableWhenStoreNil(t *testing.T) {
	app := NewApp(t.TempDir())

	cases := []struct {
		name string
		run  func() error
	}{
		{name: "StartRecording", run: func() error { return app.StartRecording("profile-1") }},
		{name: "StopRecording", run: func() error {
			_, err := app.StopRecording("profile-1", "recording")
			return err
		}},
		{name: "ListRecordings", run: func() error {
			_, err := app.ListRecordings()
			return err
		}},
		{name: "GetRecording", run: func() error {
			_, err := app.GetRecording("rec-1")
			return err
		}},
		{name: "GetRecordingDetail", run: func() error {
			_, err := app.GetRecordingDetail("rec-1", 0, 50)
			return err
		}},
		{name: "DeleteRecording", run: func() error { return app.DeleteRecording("rec-1") }},
		{name: "PlayRecording", run: func() error {
			return app.PlayRecording("profile-1", "rec-1", behavior.VariationConfig{})
		}},
		{name: "StopPlayback", run: func() error { return app.StopPlayback("profile-1") }},
		{name: "QuickRecord", run: func() error {
			_, err := app.QuickRecord("profile-1")
			return err
		}},
		{name: "CleanupStaleSessions", run: func() error { return app.CleanupStaleSessions() }},
		{name: "ActiveRecordingStatus", run: func() error {
			_, err := app.ActiveRecordingStatus()
			return err
		}},
		{name: "BehaviorRecordingRename", run: func() error { return app.BehaviorRecordingRename("rec-1", "new name") }},
		{name: "BehaviorRecordingExport", run: func() error {
			_, err := app.BehaviorRecordingExport("rec-1")
			return err
		}},
		{name: "BehaviorRecordingImport", run: func() error {
			_, err := app.BehaviorRecordingImport(`{"recording":{}}`, "imported")
			return err
		}},
		{name: "BehaviorRecordingCopy", run: func() error {
			_, err := app.BehaviorRecordingCopy("rec-1", "copy")
			return err
		}},
		{name: "BehaviorRecordingTrim", run: func() error {
			_, err := app.BehaviorRecordingTrim("rec-1", 1, 1, "trim")
			return err
		}},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			defer func() {
				if r := recover(); r != nil {
					t.Fatalf("expected explicit error, got panic: %v", r)
				}
			}()

			err := tc.run()
			if err == nil {
				t.Fatal("expected recording store unavailable error")
			}
			if !strings.Contains(err.Error(), "recording store unavailable") {
				t.Fatalf("error = %q, want recording store unavailable", err.Error())
			}
			var statusErr httpStatusError
			if !errors.As(err, &statusErr) {
				t.Fatalf("error %T does not expose HTTPStatusCode", err)
			}
			if got := statusErr.HTTPStatusCode(); got != http.StatusServiceUnavailable {
				t.Fatalf("HTTPStatusCode = %d, want %d", got, http.StatusServiceUnavailable)
			}
		})
	}
}

func TestRecordingSessionDirLifecycleUsesSafeProfileFileNames(t *testing.T) {
	root := t.TempDir()
	app := NewApp(root)
	recordingDir := filepath.Join(root, "data", "recordings")

	if err := app.initRecordingSessionDir(recordingDir); err != nil {
		t.Fatalf("initRecordingSessionDir: %v", err)
	}
	if want := filepath.Join(recordingDir, ".sessions"); app.recordingSessionDir != want {
		t.Fatalf("recordingSessionDir = %q, want %q", app.recordingSessionDir, want)
	}
	if info, err := os.Stat(app.recordingSessionDir); err != nil || !info.IsDir() {
		t.Fatalf("session dir not created: info=%v err=%v", info, err)
	}

	profileID := `profile/..\escape`
	if err := app.saveRecordingSession(profileID, 9222); err != nil {
		t.Fatalf("saveRecordingSession: %v", err)
	}
	sessionPath, err := app.sessionFilePath(profileID)
	if err != nil {
		t.Fatalf("sessionFilePath: %v", err)
	}
	rel, err := filepath.Rel(app.recordingSessionDir, sessionPath)
	if err != nil {
		t.Fatalf("session path rel: %v", err)
	}
	if strings.HasPrefix(rel, "..") || filepath.IsAbs(rel) {
		t.Fatalf("session path escaped session dir: %q", sessionPath)
	}
	if strings.Contains(filepath.Base(sessionPath), "..") {
		t.Fatalf("session file name should not contain traversal fragments: %q", filepath.Base(sessionPath))
	}

	loaded, err := app.loadRecordingSession(profileID)
	if err != nil {
		t.Fatalf("loadRecordingSession: %v", err)
	}
	if loaded.ProfileId != profileID || loaded.DebugPort != 9222 {
		t.Fatalf("loaded session = %+v, want profile %q port 9222", loaded, profileID)
	}

	if err := app.deleteRecordingSession(profileID); err != nil {
		t.Fatalf("deleteRecordingSession: %v", err)
	}
	if _, err := os.Stat(sessionPath); !os.IsNotExist(err) {
		t.Fatalf("session file should be deleted, stat err=%v", err)
	}
}

func TestCleanupStaleSessionsRemovesStoppedProfileAndKeepsRunningProfile(t *testing.T) {
	root := t.TempDir()
	app := NewApp(root)
	recordingDir := filepath.Join(root, "data", "recordings")

	store, err := behavior.NewFileRecordingStore(recordingDir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}
	app.recordingStore = store
	if err := app.initRecordingSessionDir(recordingDir); err != nil {
		t.Fatalf("initRecordingSessionDir: %v", err)
	}
	app.browserMgr = browser.NewManager(config.DefaultConfig(), root)

	staleID := "profile-stale"
	activeID := "profile-active"
	app.browserMgr.Profiles[staleID] = &BrowserProfile{ProfileId: staleID, Running: false}
	app.browserMgr.Profiles[activeID] = &BrowserProfile{ProfileId: activeID, Running: true}

	if err := app.saveRecordingSession(staleID, 9333); err != nil {
		t.Fatalf("save stale session: %v", err)
	}
	if err := app.saveRecordingSession(activeID, 9444); err != nil {
		t.Fatalf("save active session: %v", err)
	}
	stalePath, err := app.sessionFilePath(staleID)
	if err != nil {
		t.Fatalf("stale session path: %v", err)
	}
	activePath, err := app.sessionFilePath(activeID)
	if err != nil {
		t.Fatalf("active session path: %v", err)
	}

	if err := app.CleanupStaleSessions(); err != nil {
		t.Fatalf("CleanupStaleSessions: %v", err)
	}
	if _, err := os.Stat(stalePath); !os.IsNotExist(err) {
		t.Fatalf("stale session should be deleted, stat err=%v", err)
	}
	if _, err := os.Stat(activePath); err != nil {
		t.Fatalf("active session should remain: %v", err)
	}
}

func TestBehaviorRecordingStatusExposesRecoverableActiveSession(t *testing.T) {
	root := t.TempDir()
	app := NewApp(root)
	recordingDir := filepath.Join(root, "data", "recordings")

	store, err := behavior.NewFileRecordingStore(recordingDir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}
	app.recordingStore = store
	if err := app.initRecordingSessionDir(recordingDir); err != nil {
		t.Fatalf("initRecordingSessionDir: %v", err)
	}
	app.browserMgr = browser.NewManager(config.DefaultConfig(), root)

	staleID := "profile-stale"
	activeID := "profile-active"
	app.browserMgr.Profiles[staleID] = &BrowserProfile{ProfileId: staleID, Running: false}
	app.browserMgr.Profiles[activeID] = &BrowserProfile{ProfileId: activeID, Running: true}

	if err := app.saveRecordingSession(staleID, 9333); err != nil {
		t.Fatalf("save stale session: %v", err)
	}
	if err := app.saveRecordingSession(activeID, 9444); err != nil {
		t.Fatalf("save active session: %v", err)
	}

	status, err := app.ActiveRecordingStatus()
	if err != nil {
		t.Fatalf("ActiveRecordingStatus: %v", err)
	}
	if !status.Active {
		t.Fatalf("status.Active = false, want true")
	}
	if status.Count != 1 {
		t.Fatalf("status.Count = %d, want 1", status.Count)
	}
	if status.ProfileID != activeID {
		t.Fatalf("status.ProfileID = %q, want %q", status.ProfileID, activeID)
	}
	if len(status.ProfileIDs) != 1 || status.ProfileIDs[0] != activeID {
		t.Fatalf("status.ProfileIDs = %#v, want [%q]", status.ProfileIDs, activeID)
	}
	if len(status.RecoverableProfileIDs) != 1 || status.RecoverableProfileIDs[0] != activeID {
		t.Fatalf("status.RecoverableProfileIDs = %#v, want [%q]", status.RecoverableProfileIDs, activeID)
	}
}

func TestBehaviorGetRecordingDetailPaginatesEvents(t *testing.T) {
	root := t.TempDir()
	app := NewApp(root)
	recordingDir := filepath.Join(root, "data", "recordings")

	store, err := behavior.NewFileRecordingStore(recordingDir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}
	app.recordingStore = store

	rec := &behavior.Recording{
		ID:         "rec-detail",
		Name:       "detail",
		DurationMs: 300,
		ViewportW:  1280,
		ViewportH:  720,
		CreatedAt:  "2026-04-29T00:00:00Z",
		Events: []behavior.RecordedEvent{
			{T: 0, Type: "move"},
			{T: 100, Type: "down"},
			{T: 200, Type: "scroll"},
		},
	}
	if err := store.Save(rec); err != nil {
		t.Fatalf("Save: %v", err)
	}

	detail, err := app.GetRecordingDetail(rec.ID, 1, 1)
	if err != nil {
		t.Fatalf("GetRecordingDetail: %v", err)
	}
	if detail.Recording == nil || detail.Recording.ID != rec.ID {
		t.Fatalf("detail summary = %#v, want recording %q", detail.Recording, rec.ID)
	}
	if detail.EventOffset != 1 || detail.EventLimit != 1 || detail.EventTotal != 3 {
		t.Fatalf("detail page metadata = %+v", detail)
	}
	if len(detail.Events) != 1 || detail.Events[0].Type != "down" {
		t.Fatalf("detail events = %#v, want one down event", detail.Events)
	}
	if detail.Stats.Total != 3 || detail.Stats.Move != 1 || detail.Stats.Click != 1 || detail.Stats.Scroll != 1 {
		t.Fatalf("detail stats = %+v", detail.Stats)
	}
}

func TestBehaviorGetRecordingDetailUsesIndexedEventPage(t *testing.T) {
	root := t.TempDir()
	app := NewApp(root)
	recordingDir := filepath.Join(root, "data", "recordings")

	store, err := behavior.NewFileRecordingStore(recordingDir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}
	app.recordingStore = store

	rec := &behavior.Recording{
		ID:         "rec-indexed-detail",
		Name:       "indexed detail",
		DurationMs: 200,
		ViewportW:  1280,
		ViewportH:  720,
		CreatedAt:  "2026-04-29T00:00:00Z",
		Events: []behavior.RecordedEvent{
			{T: 0, Type: "move"},
			{T: 100, Type: "scroll"},
		},
	}
	if err := store.Save(rec); err != nil {
		t.Fatalf("Save: %v", err)
	}

	recordingPath := filepath.Join(recordingDir, rec.ID+".json")
	data, err := os.ReadFile(recordingPath)
	if err != nil {
		t.Fatalf("read recording file: %v", err)
	}
	data = []byte(strings.Replace(string(data), `"scroll"`, `"broken`, 1))
	if err := os.WriteFile(recordingPath, data, 0644); err != nil {
		t.Fatalf("write corrupted recording file: %v", err)
	}
	if _, err := store.Get(rec.ID); err == nil {
		t.Fatal("Get should fail after corrupting full recording JSON")
	}

	detail, err := app.GetRecordingDetail(rec.ID, 0, 1)
	if err != nil {
		t.Fatalf("GetRecordingDetail should use indexed page: %v", err)
	}
	if len(detail.Events) != 1 || detail.Events[0].Type != "move" {
		t.Fatalf("detail events = %#v, want indexed first move event", detail.Events)
	}
	if detail.EventTotal != 2 || detail.Stats.Total != 2 {
		t.Fatalf("detail metadata = %+v, want indexed totals", detail)
	}
}

func TestBehaviorRecordingExportImportCopyAndTrim(t *testing.T) {
	root := t.TempDir()
	app := NewApp(root)
	recordingDir := filepath.Join(root, "data", "recordings")

	store, err := behavior.NewFileRecordingStore(recordingDir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}
	app.recordingStore = store

	source := &behavior.Recording{
		ID:               "rec-source",
		Name:             "source",
		Description:      "source description",
		EventCount:       99,
		DurationMs:       9999,
		ViewportW:        1280,
		ViewportH:        720,
		StartURL:         "https://example.test/start",
		CurrentURL:       "https://example.test/end",
		Title:            "Example",
		DevicePixelRatio: 2,
		Scale:            1,
		CreatedAt:        "2026-01-01T00:00:00Z",
		Events: []behavior.RecordedEvent{
			{T: 10, Type: "move", X: 1, Y: 2},
			{T: 110, Type: "scroll", DeltaY: 30},
			{T: 260, Type: "key", Key: "A", Text: "", Sensitive: true},
		},
	}
	if err := store.Save(source); err != nil {
		t.Fatalf("Save source: %v", err)
	}

	exported, err := app.BehaviorRecordingExport(source.ID)
	if err != nil {
		t.Fatalf("BehaviorRecordingExport: %v", err)
	}
	if exported.Version != 1 || exported.Recording == nil || exported.Recording.ID != source.ID {
		t.Fatalf("exported bundle = %+v, want version 1 and source recording", exported)
	}
	if exported.Recording.EventCount != 3 || exported.Recording.DurationMs != 250 {
		t.Fatalf("exported metadata eventCount=%d duration=%d, want 3/250", exported.Recording.EventCount, exported.Recording.DurationMs)
	}

	copied, err := app.BehaviorRecordingCopy(source.ID, "copied")
	if err != nil {
		t.Fatalf("BehaviorRecordingCopy: %v", err)
	}
	assertNewRecordingClone(t, copied, source, "copied", 3, 250)
	if copied.Events[2].Sensitive != true || copied.Events[2].Text != "" {
		t.Fatalf("copied sensitive event = %+v, want existing redacted value kept", copied.Events[2])
	}

	trimmed, err := app.BehaviorRecordingTrim(source.ID, 2, 3, "trimmed")
	if err != nil {
		t.Fatalf("BehaviorRecordingTrim: %v", err)
	}
	assertNewRecordingClone(t, trimmed, source, "trimmed", 2, 150)
	if trimmed.Events[0].Type != "scroll" || trimmed.Events[0].T != 0 {
		t.Fatalf("trimmed first event = %+v, want rebased scroll at t=0", trimmed.Events[0])
	}
	if trimmed.Events[1].Type != "key" || trimmed.Events[1].T != 150 {
		t.Fatalf("trimmed second event = %+v, want rebased key at t=150", trimmed.Events[1])
	}

	payload, err := json.Marshal(exported)
	if err != nil {
		t.Fatalf("marshal export bundle: %v", err)
	}
	imported, err := app.BehaviorRecordingImport(string(payload), "imported")
	if err != nil {
		t.Fatalf("BehaviorRecordingImport: %v", err)
	}
	assertNewRecordingClone(t, imported, source, "imported", 3, 250)

	if _, err := store.Get(source.ID); err != nil {
		t.Fatalf("source recording should remain after copy/import/trim: %v", err)
	}
}

func TestBehaviorRecordingTrimRejectsInvalidOneBasedRange(t *testing.T) {
	root := t.TempDir()
	app := NewApp(root)
	recordingDir := filepath.Join(root, "data", "recordings")

	store, err := behavior.NewFileRecordingStore(recordingDir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}
	app.recordingStore = store
	rec := &behavior.Recording{
		ID:        "rec-range",
		Name:      "range",
		CreatedAt: "2026-01-01T00:00:00Z",
		Events: []behavior.RecordedEvent{
			{T: 0, Type: "move"},
			{T: 100, Type: "scroll"},
		},
		ViewportW: 1280,
		ViewportH: 720,
	}
	if err := store.Save(rec); err != nil {
		t.Fatalf("Save: %v", err)
	}

	_, err = app.BehaviorRecordingTrim(rec.ID, 0, 2, "bad")
	if err == nil {
		t.Fatal("expected invalid trim range error")
	}
	var statusErr httpStatusError
	if !errors.As(err, &statusErr) {
		t.Fatalf("error %T does not expose HTTPStatusCode", err)
	}
	if got := statusErr.HTTPStatusCode(); got != http.StatusBadRequest {
		t.Fatalf("HTTPStatusCode = %d, want %d", got, http.StatusBadRequest)
	}
}

func assertNewRecordingClone(t *testing.T, got *behavior.Recording, source *behavior.Recording, wantName string, wantEvents int, wantDuration int64) {
	t.Helper()
	if got == nil {
		t.Fatal("recording is nil")
	}
	if got.ID == "" || got.ID == source.ID {
		t.Fatalf("ID = %q, want non-empty new ID different from %q", got.ID, source.ID)
	}
	if got.CreatedAt == "" || got.CreatedAt == source.CreatedAt {
		t.Fatalf("CreatedAt = %q, want fresh timestamp different from %q", got.CreatedAt, source.CreatedAt)
	}
	if got.Name != wantName {
		t.Fatalf("Name = %q, want %q", got.Name, wantName)
	}
	if got.EventCount != wantEvents || len(got.Events) != wantEvents {
		t.Fatalf("events count EventCount=%d len=%d, want %d", got.EventCount, len(got.Events), wantEvents)
	}
	if got.DurationMs != wantDuration {
		t.Fatalf("DurationMs = %d, want %d", got.DurationMs, wantDuration)
	}
	if got.ViewportW != source.ViewportW || got.ViewportH != source.ViewportH {
		t.Fatalf("viewport = %dx%d, want %dx%d", got.ViewportW, got.ViewportH, source.ViewportW, source.ViewportH)
	}
}
