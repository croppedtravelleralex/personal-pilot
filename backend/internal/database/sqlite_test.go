package database

import (
	"path/filepath"
	"testing"
)

func TestMigrateRepairsBrowserCoreKindColumnWhenVersionAlreadyAdvanced(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "app.db")
	db, err := NewDB(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()

	if _, err := db.conn.Exec(`
		CREATE TABLE browser_cores (
			core_id    TEXT PRIMARY KEY,
			core_name  TEXT NOT NULL,
			core_path  TEXT NOT NULL,
			is_default INTEGER NOT NULL DEFAULT 0,
			sort_order INTEGER NOT NULL DEFAULT 0,
			created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
		);
		CREATE TABLE browser_profiles (
			profile_id       TEXT PRIMARY KEY,
			profile_name     TEXT NOT NULL
		);
		CREATE TABLE schema_migrations (
			version    INTEGER PRIMARY KEY,
			desc       TEXT NOT NULL DEFAULT '',
			applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
		);
		INSERT INTO schema_migrations (version, desc) VALUES (12, 'drifted');
	`); err != nil {
		t.Fatal(err)
	}

	if err := db.Migrate(); err != nil {
		t.Fatal(err)
	}

	hasKind, err := db.tableHasColumn("browser_cores", "kind")
	if err != nil {
		t.Fatal(err)
	}
	if !hasKind {
		t.Fatal("expected browser_cores.kind to be repaired")
	}
}

func TestMigrateRepairsSchedulerTaskRuntimeColumnsWhenVersionAlreadyAdvanced(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "app.db")
	db, err := NewDB(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()

	if _, err := db.conn.Exec(`
		CREATE TABLE scheduler_tasks (
			id               TEXT PRIMARY KEY,
			name             TEXT    NOT NULL,
			trigger_type     TEXT    NOT NULL DEFAULT 'interval',
			trigger_cron     TEXT    NOT NULL DEFAULT '',
			trigger_interval TEXT    NOT NULL DEFAULT '',
			trigger_event    TEXT    NOT NULL DEFAULT '',
			actions          TEXT    NOT NULL DEFAULT '[]',
			max_retries      INTEGER NOT NULL DEFAULT 3,
			retry_delay      TEXT    NOT NULL DEFAULT '10s',
			depends_on       TEXT    NOT NULL DEFAULT '[]',
			profile_id       TEXT    NOT NULL DEFAULT '',
			enabled          INTEGER NOT NULL DEFAULT 1,
			created_at       TEXT    NOT NULL,
			updated_at       TEXT    NOT NULL
		);
		CREATE TABLE browser_cores (
			core_id    TEXT PRIMARY KEY,
			core_name  TEXT NOT NULL,
			core_path  TEXT NOT NULL,
			is_default INTEGER NOT NULL DEFAULT 0,
			sort_order INTEGER NOT NULL DEFAULT 0,
			created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
			kind TEXT NOT NULL DEFAULT 'chromium'
		);
		CREATE TABLE browser_profiles (
			profile_id       TEXT PRIMARY KEY,
			profile_name     TEXT NOT NULL
		);
		CREATE TABLE schema_migrations (
			version    INTEGER PRIMARY KEY,
			desc       TEXT NOT NULL DEFAULT '',
			applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
		);
		INSERT INTO schema_migrations (version, desc) VALUES (13, 'drifted');
	`); err != nil {
		t.Fatal(err)
	}

	if err := db.Migrate(); err != nil {
		t.Fatal(err)
	}

	for _, column := range []string{"status", "last_run_at", "last_error", "retry_count"} {
		hasColumn, err := db.tableHasColumn("scheduler_tasks", column)
		if err != nil {
			t.Fatal(err)
		}
		if !hasColumn {
			t.Fatalf("expected scheduler_tasks.%s to be repaired", column)
		}
	}
}

func TestMigrateCreatesProxySubscriptionStore(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "app.db")
	db, err := NewDB(dbPath)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()

	if err := db.Migrate(); err != nil {
		t.Fatal(err)
	}
	for _, column := range []string{
		"subscription_id", "name", "source_type", "source_url", "group_name",
		"auto_refresh", "refresh_interval_m", "last_refresh_at", "last_error",
		"created_at", "updated_at",
	} {
		hasColumn, err := db.tableHasColumn("proxy_subscriptions", column)
		if err != nil {
			t.Fatal(err)
		}
		if !hasColumn {
			t.Fatalf("expected proxy_subscriptions.%s", column)
		}
	}
	var indexCount int
	if err := db.conn.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_proxy_subscriptions_source_url'`).Scan(&indexCount); err != nil {
		t.Fatal(err)
	}
	if indexCount != 1 {
		t.Fatalf("subscription source index count = %d", indexCount)
	}
}
