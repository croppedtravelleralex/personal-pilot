package browser

import (
	"database/sql"
	"path/filepath"
	"testing"

	"personal-pilot/backend/internal/config"

	_ "modernc.org/sqlite"
)

func TestSQLiteCoreDAOEnsureCamoufoxPreset(t *testing.T) {
	db, err := sql.Open("sqlite", filepath.Join(t.TempDir(), "app.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()

	if _, err := db.Exec(`
		CREATE TABLE browser_cores (
			core_id    TEXT PRIMARY KEY,
			core_name  TEXT NOT NULL,
			core_path  TEXT NOT NULL,
			kind       TEXT NOT NULL DEFAULT 'chromium',
			is_default INTEGER NOT NULL DEFAULT 0,
			sort_order INTEGER NOT NULL DEFAULT 0,
			created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
		)
	`); err != nil {
		t.Fatal(err)
	}

	dao := NewSQLiteCoreDAO(db)
	if err := dao.EnsureCamoufoxPreset(); err != nil {
		t.Fatal(err)
	}
	if err := dao.EnsureCamoufoxPreset(); err != nil {
		t.Fatal(err)
	}

	cores, err := dao.List()
	if err != nil {
		t.Fatal(err)
	}
	if len(cores) != 1 {
		t.Fatalf("expected exactly one Camoufox preset, got %d", len(cores))
	}
	if cores[0].Kind != config.CoreKindCamoufox {
		t.Fatalf("expected kind camoufox, got %q", cores[0].Kind)
	}
}
