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
