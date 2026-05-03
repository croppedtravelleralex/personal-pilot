package backend

import (
	"path/filepath"
	"personal-pilot/backend/internal/database"
	"testing"
)

func TestWorkbenchTasksPersistence(t *testing.T) {
	app := NewApp(t.TempDir())
	input := []WorkbenchTask{
		{
			ID:          "task-1",
			Type:        "navigate",
			ProfileID:   "profile-1",
			ProfileName: "Profile 1",
			Detail:      "https://example.com",
			Status:      "success",
			CreatedAt:   "2026-04-29T00:00:00Z",
			UpdatedAt:   "2026-04-29T00:00:01Z",
		},
		{
			ID:        "task-2",
			Type:      "refresh",
			ProfileID: "profile-2",
			Status:    "error",
			Error:     "failed",
			CreatedAt: "2026-04-29T00:00:02Z",
			UpdatedAt: "2026-04-29T00:00:03Z",
		},
	}

	if err := app.SynchronizerSaveTasks(input); err != nil {
		t.Fatalf("save tasks: %v", err)
	}
	if path := app.workbenchTasksPath(); filepath.Base(path) != "tasks.json" {
		t.Fatalf("unexpected tasks path: %s", path)
	}

	tasks, err := app.SynchronizerListTasks(1)
	if err != nil {
		t.Fatalf("list tasks: %v", err)
	}
	if len(tasks) != 1 {
		t.Fatalf("len(tasks) = %d, want 1", len(tasks))
	}
	if tasks[0].ID != "task-1" || tasks[0].Detail != "https://example.com" {
		t.Fatalf("unexpected first task: %+v", tasks[0])
	}
}

func TestWorkbenchDetectionResultPersistencePrunesPerProfileKind(t *testing.T) {
	app := newWorkbenchDBTestApp(t)

	for i := 0; i < 55; i++ {
		result := WorkbenchDetectionResult{
			ID:        "det-" + string(rune('a'+i)),
			ProfileID: "profile-1",
			Kind:      workbenchDetectionKindFingerprint,
			Score:     i,
			Level:     "warning",
			Source:    "local-cdp",
			Summary:   []string{"check"},
			Payload:   map[string]interface{}{"index": i},
			CreatedAt: "2026-04-29T00:00:" + twoDigits(i) + "Z",
		}
		if err := app.WorkbenchSaveDetectionResult(result); err != nil {
			t.Fatalf("save detection %d: %v", i, err)
		}
	}

	results, err := app.WorkbenchListDetectionResults("profile-1", workbenchDetectionKindFingerprint, 100)
	if err != nil {
		t.Fatalf("list detection: %v", err)
	}
	if len(results) != workbenchDetectionRetention {
		t.Fatalf("len(results) = %d, want %d", len(results), workbenchDetectionRetention)
	}
	if results[0].Score != 54 {
		t.Fatalf("latest result score = %d, want 54", results[0].Score)
	}
	if results[len(results)-1].Score != 5 {
		t.Fatalf("oldest retained score = %d, want 5", results[len(results)-1].Score)
	}
}

func TestWorkbenchUiStatePersistence(t *testing.T) {
	app := newWorkbenchDBTestApp(t)

	state := WorkbenchUiState{
		Search:                  " alpha ",
		StatusFilter:            "running",
		GroupFilter:             "group-a",
		ActiveGroupID:           "group-a",
		SelectedIDs:             []string{"p1", "p1", "p2"},
		ScrollTop:               345,
		TargetURL:               " https://example.com ",
		SelectedReportKind:      workbenchDetectionKindIdentity,
		SelectedReportID:        "report-1",
		SelectedReportProfileID: "p1",
		ExpandedItems:           []string{"risk", "risk", "fingerprint"},
		ThirdPartyEnabled:       true,
	}
	if err := app.WorkbenchSaveUiState(state); err != nil {
		t.Fatalf("save ui state: %v", err)
	}

	got, err := app.WorkbenchGetUiState()
	if err != nil {
		t.Fatalf("get ui state: %v", err)
	}
	if got.Search != "alpha" || got.TargetURL != "https://example.com" || got.StatusFilter != "running" {
		t.Fatalf("state was not normalized/restored: %+v", got)
	}
	if len(got.SelectedIDs) != 2 || got.SelectedIDs[0] != "p1" || got.SelectedIDs[1] != "p2" {
		t.Fatalf("selected ids not deduped: %+v", got.SelectedIDs)
	}
	if !got.ThirdPartyEnabled || got.ScrollTop != 345 {
		t.Fatalf("state flags not restored: %+v", got)
	}
}

func TestWorkbenchLayoutRectsStayInsideWorkArea(t *testing.T) {
	area := workbenchWindowRect{x: 10, y: 20, width: 1200, height: 800}
	rects := workbenchLayoutRects(area, 12, "grid")
	if len(rects) != 12 {
		t.Fatalf("len(rects) = %d, want 12", len(rects))
	}
	for i, rect := range rects {
		if rect.width <= 0 || rect.height <= 0 {
			t.Fatalf("rect %d has invalid size: %+v", i, rect)
		}
		if rect.x < area.x || rect.y < area.y {
			t.Fatalf("rect %d starts outside area: %+v", i, rect)
		}
		if rect.x+rect.width > area.x+area.width || rect.y+rect.height > area.y+area.height {
			t.Fatalf("rect %d exceeds area: rect=%+v area=%+v", i, rect, area)
		}
	}
}

func TestWorkbenchMainLeftLayout(t *testing.T) {
	area := workbenchWindowRect{x: 0, y: 0, width: 1000, height: 600}
	rects := workbenchLayoutRects(area, 3, "main-left")
	if len(rects) != 3 {
		t.Fatalf("len(rects) = %d, want 3", len(rects))
	}
	if rects[0].width <= rects[1].width {
		t.Fatalf("main window should be wider: main=%+v side=%+v", rects[0], rects[1])
	}
	if rects[1].x <= rects[0].x {
		t.Fatalf("side window should be placed to the right: main=%+v side=%+v", rects[0], rects[1])
	}
}

func newWorkbenchDBTestApp(t *testing.T) *App {
	t.Helper()
	root := t.TempDir()
	db, err := database.NewDB(filepath.Join(root, "app.db"))
	if err != nil {
		t.Fatalf("new db: %v", err)
	}
	t.Cleanup(func() {
		_ = db.Close()
	})
	if err := db.Migrate(); err != nil {
		t.Fatalf("migrate db: %v", err)
	}
	app := NewApp(root)
	app.db = db
	return app
}

func twoDigits(value int) string {
	value = value % 60
	if value < 10 {
		return "0" + string(rune('0'+value))
	}
	return string(rune('0'+value/10)) + string(rune('0'+value%10))
}
