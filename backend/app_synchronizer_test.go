package backend

import (
	"path/filepath"
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
