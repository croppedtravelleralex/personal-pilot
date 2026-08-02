package scheduler

import (
	"fmt"
	"path/filepath"
	"testing"
	"time"

	"personal-pilot/backend/internal/database"
)

func newMigratedSQLiteTaskStore(t *testing.T) (*database.DB, *SQLiteTaskStore) {
	t.Helper()

	db, err := database.NewDB(filepath.Join(t.TempDir(), "app.db"))
	if err != nil {
		t.Fatal(err)
	}
	if err := db.Migrate(); err != nil {
		t.Fatal(err)
	}
	return db, NewSQLiteTaskStore(db.GetConn())
}

func TestSQLiteTaskStorePersistsRuntimeState(t *testing.T) {
	db, store := newMigratedSQLiteTaskStore(t)
	defer db.Close()

	task := &TaskDef{
		ID:         "runtime-state",
		Name:       "runtime state",
		Trigger:    TaskTrigger{Type: TriggerInterval, Interval: "5m"},
		Actions:    []TaskAction{{Type: "wait", Target: "body", Timeout: 1000}},
		MaxRetries: 2,
		RetryDelay: "10s",
		ProfileID:  "profile-1",
		Enabled:    true,
		CreatedAt:  time.Date(2026, 6, 1, 9, 0, 0, 0, time.UTC),
	}
	lastRunAt := time.Date(2026, 6, 1, 9, 30, 0, 0, time.UTC)
	task.SetRuntimeState(StatusFailed, lastRunAt, "selector not found", 1)

	if err := store.Save(task); err != nil {
		t.Fatal(err)
	}

	got, err := store.Get(task.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got == nil {
		t.Fatal("expected task")
	}
	if got.Status() != StatusFailed {
		t.Fatalf("status = %s, want %s", got.Status(), StatusFailed)
	}
	if got.LastError() != "selector not found" {
		t.Fatalf("lastError = %q", got.LastError())
	}
	if got.RetryCount() != 1 {
		t.Fatalf("retryCount = %d, want 1", got.RetryCount())
	}
	if !got.LastRunAt().Equal(lastRunAt) {
		t.Fatalf("lastRunAt = %s, want %s", got.LastRunAt(), lastRunAt)
	}
}

func TestSchedulerRunTaskNowPersistsDoneState(t *testing.T) {
	db, store := newMigratedSQLiteTaskStore(t)
	defer db.Close()

	runner := &recordingRunner{}
	s := New(store, runner, nil)
	task := &TaskDef{
		ID:        "run-now",
		Name:      "run now",
		Trigger:   TaskTrigger{Type: TriggerEvent, Event: "manual"},
		CreatedAt: time.Date(2026, 6, 1, 10, 0, 0, 0, time.UTC),
		Enabled:   true,
	}
	if err := s.AddTask(task); err != nil {
		t.Fatal(err)
	}

	s.RunTaskNow(task.ID)

	got, err := store.Get(task.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got == nil {
		t.Fatal("expected persisted task")
	}
	if got.Status() != StatusDone {
		t.Fatalf("status = %s, want %s", got.Status(), StatusDone)
	}
	if got.LastRunAt().IsZero() {
		t.Fatal("lastRunAt should be persisted")
	}
}

func TestSchedulerRunTaskNowPersistsFailureState(t *testing.T) {
	db, store := newMigratedSQLiteTaskStore(t)
	defer db.Close()

	runner := &recordingRunner{err: fmt.Errorf("boom")}
	s := New(store, runner, nil)
	task := &TaskDef{
		ID:         "run-fail",
		Name:       "run fail",
		Trigger:    TaskTrigger{Type: TriggerEvent, Event: "manual"},
		MaxRetries: 0,
		CreatedAt:  time.Date(2026, 6, 1, 10, 0, 0, 0, time.UTC),
		Enabled:    true,
	}
	if err := s.AddTask(task); err != nil {
		t.Fatal(err)
	}

	s.RunTaskNow(task.ID)

	got, err := store.Get(task.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got == nil {
		t.Fatal("expected persisted task")
	}
	if got.Status() != StatusFailed {
		t.Fatalf("status = %s, want %s", got.Status(), StatusFailed)
	}
	if got.LastError() != "boom" {
		t.Fatalf("lastError = %q", got.LastError())
	}
	if got.RetryCount() != 1 {
		t.Fatalf("retryCount = %d, want 1", got.RetryCount())
	}
}

func TestSchedulerRunTaskNowPersistsRetryFailureBeforeDelayCompletes(t *testing.T) {
	db, store := newMigratedSQLiteTaskStore(t)
	defer db.Close()

	runner := &recordingRunner{err: fmt.Errorf("temporary failure")}
	s := New(store, runner, nil)
	task := &TaskDef{
		ID:         "run-retry",
		Name:       "run retry",
		Trigger:    TaskTrigger{Type: TriggerEvent, Event: "manual"},
		MaxRetries: 2,
		RetryDelay: "1h",
		CreatedAt:  time.Date(2026, 6, 1, 10, 0, 0, 0, time.UTC),
		Enabled:    true,
	}
	if err := s.AddTask(task); err != nil {
		t.Fatal(err)
	}

	s.RunTaskNow(task.ID)

	got, err := store.Get(task.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got == nil {
		t.Fatal("expected persisted task")
	}
	if got.Status() != StatusRunning {
		t.Fatalf("status = %s, want %s during retry delay", got.Status(), StatusRunning)
	}
	if got.LastError() != "temporary failure" {
		t.Fatalf("lastError = %q", got.LastError())
	}
	if got.RetryCount() != 1 {
		t.Fatalf("retryCount = %d, want 1", got.RetryCount())
	}
}

func TestSchedulerRunTaskNowClearsFailureStateAfterSuccess(t *testing.T) {
	db, store := newMigratedSQLiteTaskStore(t)
	defer db.Close()

	runner := &recordingRunner{err: fmt.Errorf("boom")}
	s := New(store, runner, nil)
	task := &TaskDef{
		ID:         "fail-then-pass",
		Name:       "fail then pass",
		Trigger:    TaskTrigger{Type: TriggerEvent, Event: "manual"},
		MaxRetries: 0,
		CreatedAt:  time.Date(2026, 6, 1, 10, 0, 0, 0, time.UTC),
		Enabled:    true,
	}
	if err := s.AddTask(task); err != nil {
		t.Fatal(err)
	}
	s.RunTaskNow(task.ID)

	runner.err = nil
	s.RunTaskNow(task.ID)

	got, err := store.Get(task.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got == nil {
		t.Fatal("expected persisted task")
	}
	if got.Status() != StatusDone {
		t.Fatalf("status = %s, want %s", got.Status(), StatusDone)
	}
	if got.LastError() != "" {
		t.Fatalf("lastError = %q, want empty after success", got.LastError())
	}
	if got.RetryCount() != 0 {
		t.Fatalf("retryCount = %d, want 0 after success", got.RetryCount())
	}
}
