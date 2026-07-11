package events_test

import (
	"database/sql"
	"testing"
	"time"

	_ "modernc.org/sqlite"

	"personal-pilot/backend/internal/events"
)

func openTestEventLogDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open sqlite: %v", err)
	}
	if _, err := db.Exec(`CREATE TABLE event_log (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		event_name TEXT NOT NULL,
		namespace TEXT NOT NULL,
		severity TEXT NOT NULL,
		payload TEXT NOT NULL DEFAULT '{}',
		created_at TEXT NOT NULL
	)`); err != nil {
		t.Fatalf("create table: %v", err)
	}
	return db
}

func TestSQLiteEventLogStoreCRUD(t *testing.T) {
	db := openTestEventLogDB(t)
	defer db.Close()
	store := events.NewSQLiteEventLogStore(db)
	now := time.Now().UTC().Format(time.RFC3339)
	old := time.Now().UTC().Add(-48 * time.Hour).Format(time.RFC3339)

	for _, entry := range []*events.EventLogEntry{
		{EventName: "browser:instance:started", Namespace: "browser", Severity: "info", Payload: map[string]interface{}{"profileId": "p1"}, CreatedAt: now},
		{EventName: "network:exit-ip:changed", Namespace: "network", Severity: "warn", Payload: map[string]interface{}{"ip": "1.2.3.4"}, CreatedAt: now},
		{EventName: "data:scrape:started", Namespace: "data", Severity: "info", Payload: map[string]interface{}{"job": "j1"}, CreatedAt: old},
	} {
		if err := store.Insert(entry); err != nil {
			t.Fatalf("insert: %v", err)
		}
	}

	count, err := store.Count(events.EventLogQuery{})
	if err != nil {
		t.Fatalf("count all: %v", err)
	}
	if count != 3 {
		t.Fatalf("count all = %d, want 3", count)
	}

	browserCount, err := store.Count(events.EventLogQuery{Namespace: "browser"})
	if err != nil {
		t.Fatalf("count browser: %v", err)
	}
	if browserCount != 1 {
		t.Fatalf("count browser = %d, want 1", browserCount)
	}

	rows, err := store.Query(events.EventLogQuery{Namespace: "network", Limit: 10})
	if err != nil {
		t.Fatalf("query network: %v", err)
	}
	if len(rows) != 1 || rows[0].EventName != "network:exit-ip:changed" {
		t.Fatalf("unexpected network rows: %+v", rows)
	}

	deleted, err := store.Prune(time.Now().UTC().Add(-24 * time.Hour))
	if err != nil {
		t.Fatalf("prune: %v", err)
	}
	if deleted != 1 {
		t.Fatalf("prune deleted = %d, want 1", deleted)
	}

	afterPrune, err := store.Count(events.EventLogQuery{})
	if err != nil {
		t.Fatalf("count after prune: %v", err)
	}
	if afterPrune != 2 {
		t.Fatalf("count after prune = %d, want 2", afterPrune)
	}
}
