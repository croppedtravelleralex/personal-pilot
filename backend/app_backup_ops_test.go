package backend

import (
	"archive/zip"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/backup"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/config"
	"personal-pilot/backend/internal/database"
	"strings"
	"testing"
)

func TestBackupEnsureZipSuffix(t *testing.T) {
	if got := backupEnsureZipSuffix("c:/tmp/a.zip"); got != "c:/tmp/a.zip" {
		t.Fatalf("zip 后缀重复追加: %s", got)
	}
	if got := backupEnsureZipSuffix("c:/tmp/a"); got != "c:/tmp/a.zip" {
		t.Fatalf("zip 后缀追加失败: %s", got)
	}
}

func TestBackupExportPackageToPathWritesZip(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "config.yaml"), []byte("app:\n  name: test\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "data"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "data", "app.db"), []byte("test-db"), 0644); err != nil {
		t.Fatal(err)
	}

	app := NewApp(root, "test")
	app.ctx = context.Background()
	app.config = config.DefaultConfig()

	zipPath := filepath.Join(root, "export")
	result, err := app.BackupExportPackageToPath(zipPath)
	if err != nil {
		t.Fatalf("export to explicit path failed: %v", err)
	}
	if result["cancelled"] == true {
		t.Fatalf("export should not be cancelled: %+v", result)
	}
	expectedPath := zipPath + ".zip"
	if result["zipPath"] != expectedPath {
		t.Fatalf("zip path mismatch: got=%v want=%s", result["zipPath"], expectedPath)
	}
	if info, err := os.Stat(expectedPath); err != nil || info.Size() == 0 {
		t.Fatalf("backup zip was not written: info=%v err=%v", info, err)
	}
}

func TestBackupImportPackageFromPathRejectsEmptyPath(t *testing.T) {
	app := NewApp(t.TempDir(), "test")
	app.ctx = context.Background()
	app.config = config.DefaultConfig()

	result, err := app.BackupImportPackageFromPath("  ", false)
	if err != nil {
		t.Fatalf("empty import path should be a cancellation, got error: %v", err)
	}
	if result["cancelled"] != true {
		t.Fatalf("empty import path should return cancelled result: %+v", result)
	}
}

func TestBackupInitializeSystemRequiresPreflightConfirmation(t *testing.T) {
	app, cleanup := newBackupTestApp(t)
	defer cleanup()

	sentinel := filepath.Join(app.appRoot, "data", "profile-1", "Default", "Network", "Cookies")
	if err := os.MkdirAll(filepath.Dir(sentinel), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sentinel, []byte("existing-cookie-db"), 0644); err != nil {
		t.Fatal(err)
	}

	_, err := app.BackupInitializeSystem()
	if err == nil || !strings.Contains(err.Error(), "confirmation") {
		t.Fatalf("unconfirmed initialize should require confirmation, got err=%v", err)
	}
	got, readErr := os.ReadFile(sentinel)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if string(got) != "existing-cookie-db" {
		t.Fatalf("unconfirmed initialize modified cookie file: %s", string(got))
	}
}

func TestBackupImportPackageFromPathRequiresPreflightConfirmation(t *testing.T) {
	app, cleanup := newBackupTestApp(t)
	defer cleanup()

	zipPath := writeBackupPackage(t, filepath.Join(t.TempDir(), "import.zip"), map[string]string{
		"payload/browser/user-data/profile-1/Default/Network/Cookies": "incoming-cookie-db",
	})
	existingCookie := filepath.Join(app.appRoot, "data", "profile-1", "Default", "Network", "Cookies")
	if err := os.MkdirAll(filepath.Dir(existingCookie), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(existingCookie, []byte("existing-cookie-db"), 0644); err != nil {
		t.Fatal(err)
	}

	_, err := app.BackupImportPackageFromPath(zipPath, false)
	if err == nil || !strings.Contains(err.Error(), "confirmation") {
		t.Fatalf("unconfirmed import should require confirmation, got err=%v", err)
	}
	got, readErr := os.ReadFile(existingCookie)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if string(got) != "existing-cookie-db" {
		t.Fatalf("unconfirmed import modified cookie file: %s", string(got))
	}
}

func TestBackupImportPackageFromPathConfirmedAllowsAfterPreflight(t *testing.T) {
	app, cleanup := newBackupTestApp(t)
	defer cleanup()

	zipPath := writeBackupPackage(t, filepath.Join(t.TempDir(), "import.zip"), map[string]string{
		"payload/browser/user-data/profile-1/Default/Network/Cookies": "incoming-cookie-db",
	})

	preflight, err := app.BackupImportPackagePreflightFromPath(zipPath, false)
	if err != nil {
		t.Fatalf("preflight failed: %v", err)
	}
	if !preflight.WritesCookie {
		t.Fatalf("preflight should report cookie writes: %+v", preflight)
	}

	result, err := app.BackupImportPackageFromPathConfirmed(zipPath, false, backup.DestructiveConfirmation{
		Confirmed:         true,
		ConfirmationToken: preflight.ConfirmationToken,
	})
	if err != nil {
		t.Fatalf("confirmed import failed: %v", err)
	}
	if result["cancelled"] == true {
		t.Fatalf("confirmed import should not be cancelled: %+v", result)
	}
	importedCookie := filepath.Join(app.appRoot, "data", "profile-1", "Default", "Network", "Cookies")
	got, readErr := os.ReadFile(importedCookie)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if string(got) != "incoming-cookie-db" {
		t.Fatalf("confirmed import did not copy cookie file: %s", string(got))
	}
}

func TestBackupImportPackageConfirmedRejectsRunningProfile(t *testing.T) {
	app, cleanup := newBackupTestApp(t)
	defer cleanup()
	app.browserMgr.Profiles["profile-1"] = &BrowserProfile{
		ProfileId:   "profile-1",
		ProfileName: "Profile 1",
		UserDataDir: "profile-1",
		Running:     true,
	}

	zipPath := writeBackupPackage(t, filepath.Join(t.TempDir(), "import.zip"), map[string]string{
		"payload/browser/user-data/profile-1/Default/Network/Cookies": "incoming-cookie-db",
	})
	preflight, err := app.BackupImportPackagePreflightFromPath(zipPath, false)
	if err != nil {
		t.Fatalf("preflight failed: %v", err)
	}
	if len(preflight.Blockers) == 0 {
		t.Fatalf("running profile should be a preflight blocker: %+v", preflight)
	}

	_, err = app.BackupImportPackageFromPathConfirmed(zipPath, false, backup.DestructiveConfirmation{
		Confirmed:         true,
		ConfirmationToken: preflight.ConfirmationToken,
	})
	if err == nil || !strings.Contains(strings.ToLower(err.Error()), "running") {
		t.Fatalf("confirmed import should still reject running profile, got err=%v", err)
	}
}

func TestBackupMergeConfigDedup(t *testing.T) {
	current := config.DefaultConfig()
	current.App.MaxProfileLimit = 12
	current.App.UsedCDKeys = []string{"A1", "B2"}
	current.Browser.DefaultBookmarks = []config.BrowserBookmark{
		{Name: "Google", URL: "https://www.google.com/"},
	}
	current.Browser.Proxies = []config.BrowserProxy{
		{ProxyId: "p1", ProxyName: "P1", ProxyConfig: "http://127.0.0.1:7890"},
	}
	current.Browser.Cores = []config.BrowserCore{
		{CoreId: "c1", CoreName: "C1", CorePath: "chrome/c1"},
	}
	current.Browser.Profiles = []config.BrowserProfileConfig{
		{ProfileId: "u1", ProfileName: "U1", UserDataDir: "u1"},
	}

	incoming := config.DefaultConfig()
	incoming.App.UsedCDKeys = []string{"b2", "C3"}
	incoming.Browser.DefaultBookmarks = []config.BrowserBookmark{
		{Name: "Google Dup", URL: "https://www.google.com/"},
		{Name: "ChatGPT", URL: "https://chatgpt.com/"},
	}
	incoming.Browser.Proxies = []config.BrowserProxy{
		{ProxyId: "p1", ProxyName: "P1 Dup", ProxyConfig: "http://127.0.0.1:7890"},
		{ProxyId: "p2", ProxyName: "P2", ProxyConfig: "socks5://127.0.0.1:1080"},
	}
	incoming.Browser.Cores = []config.BrowserCore{
		{CoreId: "c1", CoreName: "C1 Dup", CorePath: "chrome/c1"},
		{CoreId: "c2", CoreName: "C2", CorePath: "chrome/c2"},
	}
	incoming.Browser.Profiles = []config.BrowserProfileConfig{
		{ProfileId: "u1", ProfileName: "U1 Dup", UserDataDir: "u1"},
		{ProfileId: "u2", ProfileName: "U2", UserDataDir: "u2"},
	}

	merged := backupMergeConfig(current, incoming)
	if merged == nil {
		t.Fatalf("merged 为空")
	}

	if merged.App.MaxProfileLimit != 12 {
		t.Fatalf("license limit 不应被导入配置改写: got=%d", merged.App.MaxProfileLimit)
	}
	if len(merged.App.UsedCDKeys) != 2 {
		t.Fatalf("used cd keys 不应被导入配置改写: %+v", merged.App.UsedCDKeys)
	}
	if len(merged.Browser.DefaultBookmarks) != 2 {
		t.Fatalf("bookmarks 判重失败: %+v", merged.Browser.DefaultBookmarks)
	}
	if len(merged.Browser.Proxies) != 2 {
		t.Fatalf("proxies 判重失败: %+v", merged.Browser.Proxies)
	}
	if len(merged.Browser.Cores) != 2 {
		t.Fatalf("cores 判重失败: %+v", merged.Browser.Cores)
	}
	if len(merged.Browser.Profiles) != 2 {
		t.Fatalf("profiles 判重失败: %+v", merged.Browser.Profiles)
	}
}

func TestBackupSyncDirConflictAndOverwrite(t *testing.T) {
	src := filepath.Join(t.TempDir(), "src")
	dst := filepath.Join(t.TempDir(), "dst")
	if err := os.MkdirAll(src, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(dst, 0755); err != nil {
		t.Fatal(err)
	}

	srcFile := filepath.Join(src, "a.txt")
	dstFile := filepath.Join(dst, "a.txt")
	if err := os.WriteFile(srcFile, []byte("new-content"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(dstFile, []byte("old-content"), 0644); err != nil {
		t.Fatal(err)
	}

	stats := &backupMergeStats{}
	if err := backupSyncDir(src, dst, false, stats, nil); err != nil {
		t.Fatal(err)
	}
	if stats.Conflicts != 1 || stats.Imported != 0 {
		t.Fatalf("非覆盖模式统计异常: %+v", stats)
	}
	got, err := os.ReadFile(dstFile)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "old-content" {
		t.Fatalf("非覆盖模式不应改写目标文件: %s", string(got))
	}

	stats2 := &backupMergeStats{}
	if err := backupSyncDir(src, dst, true, stats2, nil); err != nil {
		t.Fatal(err)
	}
	if stats2.Imported != 1 {
		t.Fatalf("覆盖模式导入统计异常: %+v", stats2)
	}
	got2, err := os.ReadFile(dstFile)
	if err != nil {
		t.Fatal(err)
	}
	if string(got2) != "new-content" {
		t.Fatalf("覆盖模式应改写目标文件: %s", string(got2))
	}
}

func newBackupTestApp(t *testing.T) (*App, func()) {
	t.Helper()

	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "data"), 0755); err != nil {
		t.Fatal(err)
	}
	cfg := config.DefaultConfig()
	if err := cfg.Save(filepath.Join(root, "config.yaml")); err != nil {
		t.Fatal(err)
	}
	db, err := database.NewDB(filepath.Join(root, "data", "app.db"))
	if err != nil {
		t.Fatal(err)
	}
	if err := db.Migrate(); err != nil {
		_ = db.Close()
		t.Fatal(err)
	}

	app := NewApp(root, "test")
	app.ctx = context.Background()
	app.config = cfg
	app.db = db
	app.browserMgr = browser.NewManager(cfg, root)
	conn := db.GetConn()
	app.browserMgr.ProfileDAO = browser.NewSQLiteProfileDAO(conn)
	app.browserMgr.ProxyDAO = browser.NewSQLiteProxyDAO(conn)
	app.browserMgr.CoreDAO = browser.NewSQLiteCoreDAO(conn)
	app.browserMgr.BookmarkDAO = browser.NewSQLiteBookmarkDAO(conn)
	app.browserMgr.GroupDAO = browser.NewSQLiteGroupDAO(conn)

	return app, func() {
		_ = db.Close()
	}
}

func writeBackupPackage(t *testing.T, zipPath string, files map[string]string) string {
	t.Helper()

	if err := os.MkdirAll(filepath.Dir(zipPath), 0755); err != nil {
		t.Fatal(err)
	}
	f, err := os.Create(zipPath)
	if err != nil {
		t.Fatal(err)
	}
	zw := zip.NewWriter(f)

	manifest := backup.Manifest{
		Format:          backup.PackageFormat,
		ManifestVersion: backup.ManifestVersion,
		Entries: []backup.ManifestEntry{
			{
				ID:          "browser_user_data_root",
				Category:    backup.CategoryBrowserData,
				EntryType:   backup.EntryTypeDir,
				Required:    true,
				ArchivePath: "payload/browser/user-data/",
			},
		},
	}
	data, err := json.Marshal(manifest)
	if err != nil {
		t.Fatal(err)
	}
	mw, err := zw.Create("manifest.json")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := mw.Write(data); err != nil {
		t.Fatal(err)
	}
	for name, content := range files {
		w, err := zw.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := w.Write([]byte(content)); err != nil {
			t.Fatal(err)
		}
	}
	if err := zw.Close(); err != nil {
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}
	return zipPath
}
