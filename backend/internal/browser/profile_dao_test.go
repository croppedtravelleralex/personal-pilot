package browser

import (
	"database/sql"
	"os"
	"path/filepath"
	"testing"

	_ "modernc.org/sqlite"
)

func TestSQLiteProfileDAO_UpsertAndList(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "ant-test-profile-dao")
	defer os.RemoveAll(dir)
	os.MkdirAll(dir, 0755)

	dbPath := filepath.Join(dir, "test.db")
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	// Create table
	_, err = db.Exec(`CREATE TABLE browser_profiles (
		profile_id            TEXT PRIMARY KEY,
		profile_name          TEXT NOT NULL,
		user_data_dir         TEXT NOT NULL DEFAULT '',
		core_id               TEXT NOT NULL DEFAULT '',
		fingerprint_args      TEXT NOT NULL DEFAULT '[]',
		proxy_id              TEXT NOT NULL DEFAULT '',
		proxy_config          TEXT NOT NULL DEFAULT '',
		proxy_bind_source_id  TEXT NOT NULL DEFAULT '',
		proxy_bind_source_url TEXT NOT NULL DEFAULT '',
		proxy_bind_name       TEXT NOT NULL DEFAULT '',
		proxy_bind_updated_at TEXT NOT NULL DEFAULT '',
		launch_args           TEXT NOT NULL DEFAULT '[]',
		tags                  TEXT NOT NULL DEFAULT '[]',
		keywords              TEXT NOT NULL DEFAULT '[]',
		group_id              TEXT NOT NULL DEFAULT '',
		behavior_profile_id   TEXT NOT NULL DEFAULT '',
		created_at            DATETIME NOT NULL,
		updated_at            DATETIME NOT NULL
	)`)
	if err != nil {
		t.Fatalf("create table: %v", err)
	}

	dao := NewSQLiteProfileDAO(db)

	// Test Upsert
	profile := &Profile{
		ProfileId:         "test-001",
		ProfileName:       "测试实例",
		CoreId:            "default",
		FingerprintArgs:   []string{"--fingerprint-brand=Chrome"},
		LaunchArgs:        []string{"--disable-sync"},
		Tags:              []string{"test"},
		Keywords:          []string{},
		BehaviorProfileID: "office-worker",
	}
	err = dao.Upsert(profile)
	if err != nil {
		t.Fatalf("Upsert failed: %v", err)
	}

	// Test List
	list, err := dao.List()
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}
	if len(list) != 1 {
		t.Fatalf("expected 1 profile, got %d", len(list))
	}
	loaded := list[0]
	if loaded.ProfileName != "测试实例" {
		t.Errorf("name: got %q, want 测试实例", loaded.ProfileName)
	}
	if loaded.BehaviorProfileID != "office-worker" {
		t.Errorf("behavior_profile_id: got %q, want office-worker", loaded.BehaviorProfileID)
	}

	// Test GetById
	loaded2, err := dao.GetById("test-001")
	if err != nil {
		t.Fatalf("GetById failed: %v", err)
	}
	if loaded2.BehaviorProfileID != "office-worker" {
		t.Errorf("behavior_profile_id: got %q, want office-worker", loaded2.BehaviorProfileID)
	}

	// Test Delete
	err = dao.Delete("test-001")
	if err != nil {
		t.Fatalf("Delete failed: %v", err)
	}
	list, _ = dao.List()
	if len(list) != 0 {
		t.Errorf("expected empty list after delete, got %d", len(list))
	}
}
