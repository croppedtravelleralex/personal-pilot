package behavior

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestFileRecordingStore_SaveAndGet(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-recordings-save-get")
	defer os.RemoveAll(dir)

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	rec := &Recording{
		ID:          "rec-test-001",
		Name:        "测试录制",
		Description: "测试描述",
		Events: []RecordedEvent{
			{T: 0, Type: "move", X: 100, Y: 200},
			{T: 500, Type: "click", X: 100, Y: 200, Button: 0},
			{T: 1000, Type: "key", Key: "Enter"},
		},
		DurationMs: 1000,
		ViewportW:  1920,
		ViewportH:  1080,
		CreatedAt:  "2025-01-01T00:00:00Z",
	}

	if err := store.Save(rec); err != nil {
		t.Fatalf("Save: %v", err)
	}

	// Verify file exists
	if _, err := os.Stat(filepath.Join(dir, "rec-test-001.json")); err != nil {
		t.Fatalf("file not created: %v", err)
	}
	if _, err := os.Stat(store.manifestPath(rec.ID)); err != nil {
		t.Fatalf("manifest not created: %v", err)
	}
	if _, err := os.Stat(store.eventIndexPath(rec.ID)); err != nil {
		t.Fatalf("event index not created: %v", err)
	}

	// Get and verify
	loaded, err := store.Get("rec-test-001")
	if err != nil {
		t.Fatalf("Get: %v", err)
	}
	if loaded.Name != rec.Name {
		t.Errorf("Name: got %q, want %q", loaded.Name, rec.Name)
	}
	if loaded.DurationMs != rec.DurationMs {
		t.Errorf("DurationMs: got %d, want %d", loaded.DurationMs, rec.DurationMs)
	}
	if len(loaded.Events) != 3 {
		t.Errorf("Events length: got %d, want 3", len(loaded.Events))
	}
	if loaded.Events[0].Type != "move" {
		t.Errorf("First event type: got %q, want move", loaded.Events[0].Type)
	}
}

func TestFileRecordingStore_Get_NotFound(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-recordings-get-notfound")
	defer os.RemoveAll(dir)

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	_, err = store.Get("nonexistent")
	if err == nil {
		t.Fatal("expected error for nonexistent recording")
	}
}

func TestFileRecordingStore_List(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-recordings-list")
	defer os.RemoveAll(dir)

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	// Initially empty
	list, err := store.List()
	if err != nil {
		t.Fatalf("List (empty): %v", err)
	}
	if len(list) != 0 {
		t.Errorf("expected empty list, got %d items", len(list))
	}

	// Save two recordings with different CreatedAt times
	older := &Recording{
		ID: "rec-older", Name: "Older", CreatedAt: "2025-01-01T00:00:00Z",
		Events: []RecordedEvent{}, ViewportW: 1920, ViewportH: 1080,
	}
	newer := &Recording{
		ID: "rec-newer", Name: "Newer", CreatedAt: "2025-06-01T00:00:00Z",
		Events: []RecordedEvent{}, ViewportW: 1920, ViewportH: 1080,
	}
	store.Save(older)
	store.Save(newer)

	list, err = store.List()
	if err != nil {
		t.Fatalf("List: %v", err)
	}
	if len(list) != 2 {
		t.Fatalf("expected 2 recordings, got %d", len(list))
	}
	// Newer should be first (descending by CreatedAt)
	if list[0].ID != "rec-newer" {
		t.Errorf("first item: got %q, want rec-newer", list[0].ID)
	}
	if list[1].ID != "rec-older" {
		t.Errorf("second item: got %q, want rec-older", list[1].ID)
	}
}

func TestFileRecordingStore_ListSummariesOmitsEventsAndKeepsEventCount(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-recordings-list-summaries")
	defer os.RemoveAll(dir)

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	rec := &Recording{
		ID:        "rec-summary",
		Name:      "Summary",
		CreatedAt: "2025-06-01T00:00:00Z",
		Events: []RecordedEvent{
			{T: 0, Type: "move", X: 1, Y: 2},
			{T: 100, Type: "scroll", DeltaY: 50},
		},
		DurationMs: 100,
		ViewportW:  1920,
		ViewportH:  1080,
	}
	if err := store.Save(rec); err != nil {
		t.Fatalf("Save: %v", err)
	}

	summaries, err := store.ListSummaries()
	if err != nil {
		t.Fatalf("ListSummaries: %v", err)
	}
	if len(summaries) != 1 {
		t.Fatalf("expected 1 summary, got %d", len(summaries))
	}
	if summaries[0].ID != rec.ID {
		t.Fatalf("summary ID = %q, want %q", summaries[0].ID, rec.ID)
	}
	if summaries[0].EventCount != 2 {
		t.Fatalf("EventCount = %d, want 2", summaries[0].EventCount)
	}

	loaded, err := store.Get(rec.ID)
	if err != nil {
		t.Fatalf("Get: %v", err)
	}
	if len(loaded.Events) != 2 {
		t.Fatalf("detail events length = %d, want 2", len(loaded.Events))
	}
	if loaded.EventCount != 2 {
		t.Fatalf("detail EventCount = %d, want 2", loaded.EventCount)
	}
}

func TestFileRecordingStore_ListSummariesUsesManifestWhenEventsCannotUnmarshal(t *testing.T) {
	dir := t.TempDir()

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	rec := &Recording{
		ID:        "rec-summary-index",
		Name:      "Summary Index",
		CreatedAt: "2025-06-01T00:00:00Z",
		Events: []RecordedEvent{
			{T: 0, Type: "move", X: 1, Y: 2},
			{T: 100, Type: "scroll", DeltaY: 50},
		},
		DurationMs: 100,
		ViewportW:  1920,
		ViewportH:  1080,
	}
	if err := store.Save(rec); err != nil {
		t.Fatalf("Save: %v", err)
	}

	path := filepath.Join(dir, rec.ID+".json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read recording file: %v", err)
	}
	var payload map[string]json.RawMessage
	if err := json.Unmarshal(data, &payload); err != nil {
		t.Fatalf("decode recording file: %v", err)
	}
	payload["events"] = json.RawMessage(`"events intentionally not decoded"`)
	data, err = json.Marshal(payload)
	if err != nil {
		t.Fatalf("encode mutated recording file: %v", err)
	}
	if err := os.WriteFile(path, data, 0644); err != nil {
		t.Fatalf("write mutated recording file: %v", err)
	}

	summaries, err := store.ListSummaries()
	if err != nil {
		t.Fatalf("ListSummaries: %v", err)
	}
	if len(summaries) != 1 {
		t.Fatalf("summary count = %d, want 1", len(summaries))
	}
	if summaries[0].ID != rec.ID || summaries[0].EventCount != 2 {
		t.Fatalf("summary = %+v, want indexed metadata for %s", summaries[0], rec.ID)
	}
}

func TestFileRecordingStore_GetDetailPageUsesIndexForRequestedEvents(t *testing.T) {
	dir := t.TempDir()

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	rec := &Recording{
		ID:        "rec-detail-index",
		Name:      "Detail Index",
		CreatedAt: "2025-06-01T00:00:00Z",
		Events: []RecordedEvent{
			{T: 0, Type: "move", X: 1, Y: 2},
			{T: 100, Type: "scroll", DeltaY: 50},
		},
		DurationMs: 100,
		ViewportW:  1920,
		ViewportH:  1080,
	}
	if err := store.Save(rec); err != nil {
		t.Fatalf("Save: %v", err)
	}

	path := filepath.Join(dir, rec.ID+".json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read recording file: %v", err)
	}
	refs, err := buildRecordingEventRefs(data)
	if err != nil {
		t.Fatalf("build refs: %v", err)
	}
	if len(refs) != 2 {
		t.Fatalf("ref count = %d, want 2", len(refs))
	}
	secondStart := int(refs[1].Offset)
	copy(data[secondStart:secondStart+refs[1].Length], bytes.Repeat([]byte("x"), refs[1].Length))
	if err := os.WriteFile(path, data, 0644); err != nil {
		t.Fatalf("write corrupted recording file: %v", err)
	}
	if _, err := store.Get(rec.ID); err == nil {
		t.Fatal("Get should fail after corrupting a non-requested event")
	}

	page, err := store.GetDetailPage(rec.ID, 0, 1)
	if err != nil {
		t.Fatalf("GetDetailPage: %v", err)
	}
	if page.EventOffset != 0 || page.EventLimit != 1 || page.EventTotal != 2 {
		t.Fatalf("unexpected page metadata: %+v", page)
	}
	if len(page.Events) != 1 || page.Events[0].Type != "move" {
		t.Fatalf("events = %#v, want first move event only", page.Events)
	}
	if page.Stats.Total != 2 || page.Stats.Move != 1 || page.Stats.Scroll != 1 {
		t.Fatalf("stats = %+v, want indexed full stats", page.Stats)
	}
}

func TestFileRecordingStore_GetDetailPageFallsBackForLegacyJSON(t *testing.T) {
	dir := t.TempDir()

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	rec := &Recording{
		ID:        "rec-legacy",
		Name:      "Legacy",
		CreatedAt: "2025-01-01T00:00:00Z",
		Events: []RecordedEvent{
			{T: 0, Type: "move"},
			{T: 100, Type: "scroll"},
		},
		DurationMs: 100,
		ViewportW:  1280,
		ViewportH:  720,
	}
	data, err := json.MarshalIndent(rec, "", "  ")
	if err != nil {
		t.Fatalf("marshal legacy recording: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dir, rec.ID+".json"), data, 0644); err != nil {
		t.Fatalf("write legacy recording: %v", err)
	}

	summaries, err := store.ListSummaries()
	if err != nil {
		t.Fatalf("ListSummaries: %v", err)
	}
	if len(summaries) != 1 || summaries[0].EventCount != 2 {
		t.Fatalf("legacy summaries = %+v, want fallback event count", summaries)
	}

	page, err := store.GetDetailPage(rec.ID, 1, 1)
	if err != nil {
		t.Fatalf("GetDetailPage: %v", err)
	}
	if len(page.Events) != 1 || page.Events[0].Type != "scroll" {
		t.Fatalf("legacy page events = %#v, want scroll", page.Events)
	}
	if page.EventTotal != 2 || page.Stats.Total != 2 {
		t.Fatalf("legacy page metadata = %+v, want fallback totals", page)
	}
}

func TestFileRecordingStore_Delete(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-recordings-delete")
	defer os.RemoveAll(dir)

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	rec := &Recording{
		ID: "rec-to-delete", Name: "Delete Me", CreatedAt: "2025-01-01T00:00:00Z",
		Events: []RecordedEvent{}, ViewportW: 1920, ViewportH: 1080,
	}
	store.Save(rec)

	if err := store.Delete("rec-to-delete"); err != nil {
		t.Fatalf("Delete: %v", err)
	}

	// Verify file is gone
	if _, err := os.Stat(filepath.Join(dir, "rec-to-delete.json")); !os.IsNotExist(err) {
		t.Error("file should have been deleted")
	}

	// Verify Get returns error
	if _, err := store.Get("rec-to-delete"); err == nil {
		t.Error("Get after delete should return error")
	}
}

func TestFileRecordingStore_Delete_NotFound(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-recordings-delete-notfound")
	defer os.RemoveAll(dir)

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	err = store.Delete("nonexistent")
	if err == nil {
		t.Fatal("expected error for deleting nonexistent recording")
	}
}

func TestFileRecordingStore_List_SkipsDirectoriesAndNonJSON(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-recordings-list-filter")
	defer os.RemoveAll(dir)

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	// Create a subdirectory (should be skipped)
	os.MkdirAll(filepath.Join(dir, ".sessions"), 0755)
	// Create a non-JSON file (should be skipped)
	os.WriteFile(filepath.Join(dir, "notes.txt"), []byte("hello"), 0644)

	list, err := store.List()
	if err != nil {
		t.Fatalf("List: %v", err)
	}
	if len(list) != 0 {
		t.Errorf("expected 0 recordings (filtered out non-JSON), got %d", len(list))
	}
}

func TestFileRecordingStore_Rename(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-recordings-rename")
	defer os.RemoveAll(dir)

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	rec := &Recording{
		ID: "rec-rename-test", Name: "原名", CreatedAt: "2025-01-01T00:00:00Z",
		Events: []RecordedEvent{}, ViewportW: 1920, ViewportH: 1080,
	}
	store.Save(rec)

	if err := store.Rename("rec-rename-test", "新名"); err != nil {
		t.Fatalf("Rename: %v", err)
	}

	loaded, err := store.Get("rec-rename-test")
	if err != nil {
		t.Fatalf("Get after rename: %v", err)
	}
	if loaded.Name != "新名" {
		t.Errorf("Name after rename: got %q, want 新名", loaded.Name)
	}
}

func TestFileRecordingStore_RenameRefreshesManifest(t *testing.T) {
	dir := t.TempDir()

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	rec := &Recording{
		ID:        "rec-rename-index",
		Name:      "old",
		CreatedAt: "2025-01-01T00:00:00Z",
		Events:    []RecordedEvent{{T: 0, Type: "move"}},
		ViewportW: 1920,
		ViewportH: 1080,
	}
	if err := store.Save(rec); err != nil {
		t.Fatalf("Save: %v", err)
	}
	if err := store.Rename(rec.ID, "new"); err != nil {
		t.Fatalf("Rename: %v", err)
	}

	summaries, err := store.ListSummaries()
	if err != nil {
		t.Fatalf("ListSummaries: %v", err)
	}
	if len(summaries) != 1 || summaries[0].Name != "new" {
		t.Fatalf("summary after rename = %+v, want refreshed manifest", summaries)
	}
}

func TestFileRecordingStore_Rename_NotFound(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-recordings-rename-notfound")
	defer os.RemoveAll(dir)

	store, err := NewFileRecordingStore(dir)
	if err != nil {
		t.Fatalf("NewFileRecordingStore: %v", err)
	}

	err = store.Rename("nonexistent", "newname")
	if err == nil {
		t.Fatal("expected error for renaming nonexistent recording")
	}
}
