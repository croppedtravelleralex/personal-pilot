package browser

import (
	"database/sql"
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/config"
	"testing"

	_ "modernc.org/sqlite"
)

func createProfileTableForTest(t *testing.T, db *sql.DB, includeHumanizeSeed bool) {
	t.Helper()
	humanizeSeedColumn := ""
	if includeHumanizeSeed {
		humanizeSeedColumn = "humanize_seed TEXT NOT NULL DEFAULT '',"
	}
	_, err := db.Exec(`CREATE TABLE browser_profiles (
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
		` + humanizeSeedColumn + `
		created_at            DATETIME NOT NULL,
		updated_at            DATETIME NOT NULL
	)`)
	if err != nil {
		t.Fatalf("create table: %v", err)
	}
}

func TestSQLiteProfileDAO_UpsertAndList(t *testing.T) {
	dir := filepath.Join(os.TempDir(), "personal-pilot-test-profile-dao")
	defer os.RemoveAll(dir)
	os.MkdirAll(dir, 0755)

	dbPath := filepath.Join(dir, "test.db")
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	createProfileTableForTest(t, db, true)

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
		HumanizeSeed:      "humanize-seed-001",
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
	if loaded.HumanizeSeed != "humanize-seed-001" {
		t.Errorf("humanize_seed: got %q, want humanize-seed-001", loaded.HumanizeSeed)
	}

	// Test GetById
	loaded2, err := dao.GetById("test-001")
	if err != nil {
		t.Fatalf("GetById failed: %v", err)
	}
	if loaded2.BehaviorProfileID != "office-worker" {
		t.Errorf("behavior_profile_id: got %q, want office-worker", loaded2.BehaviorProfileID)
	}
	if loaded2.HumanizeSeed != "humanize-seed-001" {
		t.Errorf("humanize_seed: got %q, want humanize-seed-001", loaded2.HumanizeSeed)
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

func TestManagerProfileHumanizeSeedLifecycle(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()
	createProfileTableForTest(t, db, true)

	mgr := NewManager(config.DefaultConfig(), t.TempDir())
	mgr.ProfileDAO = NewSQLiteProfileDAO(db)

	created, err := mgr.Create(ProfileInput{ProfileName: "source"})
	if err != nil {
		t.Fatalf("create profile: %v", err)
	}
	if created.HumanizeSeed == "" {
		t.Fatalf("new profile should get a humanize seed")
	}

	copied, err := mgr.Copy(created.ProfileId, "copy")
	if err != nil {
		t.Fatalf("copy profile: %v", err)
	}
	if copied.HumanizeSeed != created.HumanizeSeed {
		t.Fatalf("copy should preserve humanize seed: got %q, want %q", copied.HumanizeSeed, created.HumanizeSeed)
	}

	updated, err := mgr.Update(created.ProfileId, ProfileInput{ProfileName: "renamed"})
	if err != nil {
		t.Fatalf("update profile: %v", err)
	}
	if updated.HumanizeSeed != created.HumanizeSeed {
		t.Fatalf("empty update input should not clear humanize seed: got %q, want %q", updated.HumanizeSeed, created.HumanizeSeed)
	}
}

func TestSQLiteProfileDAO_OldTableAddsHumanizeSeedWithoutReadWriteLoss(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()
	createProfileTableForTest(t, db, false)

	_, err = db.Exec(`INSERT INTO browser_profiles
		(profile_id, profile_name, user_data_dir, core_id, fingerprint_args,
		 proxy_id, proxy_config, launch_args, tags, keywords, group_id,
		 behavior_profile_id, created_at, updated_at)
		VALUES ('legacy-001', 'legacy', 'legacy-001', '', '[]', '', '', '[]', '[]', '[]', '', '', '2026-04-29T00:00:00Z', '2026-04-29T00:00:00Z')`)
	if err != nil {
		t.Fatalf("insert legacy row: %v", err)
	}

	dao := NewSQLiteProfileDAO(db)
	list, err := dao.List()
	if err != nil {
		t.Fatalf("list old table: %v", err)
	}
	if len(list) != 1 {
		t.Fatalf("expected 1 profile, got %d", len(list))
	}
	if list[0].HumanizeSeed == "" {
		t.Fatalf("legacy profile should get an in-memory humanize seed")
	}

	if err := dao.Upsert(list[0]); err != nil {
		t.Fatalf("upsert legacy profile: %v", err)
	}
	var storedSeed string
	if err := db.QueryRow(`SELECT humanize_seed FROM browser_profiles WHERE profile_id = 'legacy-001'`).Scan(&storedSeed); err != nil {
		t.Fatalf("read migrated humanize_seed: %v", err)
	}
	if storedSeed != list[0].HumanizeSeed {
		t.Fatalf("persisted seed mismatch: got %q, want %q", storedSeed, list[0].HumanizeSeed)
	}
}

func TestSQLiteProfileDAO_UpsertEmptyHumanizeSeedKeepsExisting(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()
	createProfileTableForTest(t, db, true)

	dao := NewSQLiteProfileDAO(db)
	if err := dao.Upsert(&Profile{
		ProfileId:    "seeded-001",
		ProfileName:  "seeded",
		UserDataDir:  "seeded-001",
		HumanizeSeed: "keep-me",
	}); err != nil {
		t.Fatalf("initial upsert: %v", err)
	}

	if err := dao.Upsert(&Profile{
		ProfileId:   "seeded-001",
		ProfileName: "seeded updated",
		UserDataDir: "seeded-001",
	}); err != nil {
		t.Fatalf("empty seed upsert: %v", err)
	}

	loaded, err := dao.GetById("seeded-001")
	if err != nil {
		t.Fatalf("get updated profile: %v", err)
	}
	if loaded.HumanizeSeed != "keep-me" {
		t.Fatalf("empty seed update should keep existing seed: got %q", loaded.HumanizeSeed)
	}
}

func TestSQLiteProfileDAO_ListByGroupIncludesHumanizeSeed(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()
	createProfileTableForTest(t, db, true)

	dao := NewSQLiteProfileDAO(db)
	if err := dao.Upsert(&Profile{
		ProfileId:         "grouped-001",
		ProfileName:       "grouped",
		UserDataDir:       "grouped-001",
		GroupId:           "group-a",
		BehaviorProfileID: "office-worker",
		HumanizeSeed:      "group-seed",
	}); err != nil {
		t.Fatalf("upsert grouped profile: %v", err)
	}

	list, err := dao.ListByGroup("group-a", false, nil)
	if err != nil {
		t.Fatalf("list by group: %v", err)
	}
	if len(list) != 1 {
		t.Fatalf("expected 1 profile, got %d", len(list))
	}
	if list[0].HumanizeSeed != "group-seed" {
		t.Fatalf("humanize seed = %q, want group-seed", list[0].HumanizeSeed)
	}
	if list[0].BehaviorProfileID != "office-worker" {
		t.Fatalf("behavior profile = %q, want office-worker", list[0].BehaviorProfileID)
	}
}

func TestManagerSaveProfilesConfigPreservesBehaviorIdentity(t *testing.T) {
	cfg := config.DefaultConfig()
	mgr := NewManager(cfg, t.TempDir())
	mgr.Profiles = map[string]*Profile{
		"profile-identity": {
			ProfileId:         "profile-identity",
			ProfileName:       "identity",
			UserDataDir:       "profile-identity",
			BehaviorProfileID: "office-worker",
			HumanizeSeed:      "stable-seed",
		},
	}

	if err := mgr.SaveProfiles(); err != nil {
		t.Fatalf("save profiles: %v", err)
	}
	if len(cfg.Browser.Profiles) != 1 {
		t.Fatalf("saved profiles = %d, want 1", len(cfg.Browser.Profiles))
	}
	saved := cfg.Browser.Profiles[0]
	if saved.BehaviorProfileID != "office-worker" {
		t.Fatalf("behavior profile = %q, want office-worker", saved.BehaviorProfileID)
	}
	if saved.HumanizeSeed != "stable-seed" {
		t.Fatalf("humanize seed = %q, want stable-seed", saved.HumanizeSeed)
	}
}
