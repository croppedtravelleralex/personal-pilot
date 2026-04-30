package backend

import (
	"archive/zip"
	"encoding/json"
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/backup"
	"strings"
	"testing"
)

func TestBrowserSnapshotRestoreRequiresConfirmation(t *testing.T) {
	app, cleanup := newBackupTestApp(t)
	defer cleanup()
	profile := &BrowserProfile{
		ProfileId:   "profile-1",
		ProfileName: "Profile 1",
		UserDataDir: "profile-1",
	}
	app.browserMgr.Profiles[profile.ProfileId] = profile

	userDataDir := app.browserMgr.ResolveUserDataDir(profile)
	existingCookie := filepath.Join(userDataDir, "Default", "Network", "Cookies")
	if err := os.MkdirAll(filepath.Dir(existingCookie), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(existingCookie, []byte("existing-cookie-db"), 0644); err != nil {
		t.Fatal(err)
	}
	writeSnapshotPackage(t, app, "profile-1", "snap-1", "snapshot-cookie-db")

	err := app.BrowserSnapshotRestore("profile-1", "snap-1")
	if err == nil || !strings.Contains(err.Error(), "confirmation") {
		t.Fatalf("unconfirmed snapshot restore should require confirmation, got err=%v", err)
	}
	got, readErr := os.ReadFile(existingCookie)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if string(got) != "existing-cookie-db" {
		t.Fatalf("unconfirmed restore modified cookie file: %s", string(got))
	}
}

func TestBrowserSnapshotRestoreConfirmedReplacesProfileData(t *testing.T) {
	app, cleanup := newBackupTestApp(t)
	defer cleanup()
	profile := &BrowserProfile{
		ProfileId:   "profile-1",
		ProfileName: "Profile 1",
		UserDataDir: "profile-1",
	}
	app.browserMgr.Profiles[profile.ProfileId] = profile

	userDataDir := app.browserMgr.ResolveUserDataDir(profile)
	if err := os.MkdirAll(userDataDir, 0755); err != nil {
		t.Fatal(err)
	}
	writeSnapshotPackage(t, app, "profile-1", "snap-1", "snapshot-cookie-db")

	preflight, err := app.BrowserSnapshotRestorePreflight("profile-1", "snap-1")
	if err != nil {
		t.Fatalf("snapshot preflight failed: %v", err)
	}
	if !preflight.WritesCookie {
		t.Fatalf("snapshot preflight should report cookie writes: %+v", preflight)
	}
	if err := app.BrowserSnapshotRestoreConfirmed("profile-1", "snap-1", backup.DestructiveConfirmation{
		Confirmed:         true,
		ConfirmationToken: preflight.ConfirmationToken,
	}); err != nil {
		t.Fatalf("confirmed snapshot restore failed: %v", err)
	}

	restoredCookie := filepath.Join(userDataDir, "Default", "Network", "Cookies")
	got, readErr := os.ReadFile(restoredCookie)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if string(got) != "snapshot-cookie-db" {
		t.Fatalf("snapshot restore did not replace cookie file: %s", string(got))
	}
}

func TestBrowserSnapshotRestoreConfirmedRejectsRunningProfile(t *testing.T) {
	app, cleanup := newBackupTestApp(t)
	defer cleanup()
	app.browserMgr.Profiles["profile-1"] = &BrowserProfile{
		ProfileId:   "profile-1",
		ProfileName: "Profile 1",
		UserDataDir: "profile-1",
		Running:     true,
	}
	writeSnapshotPackage(t, app, "profile-1", "snap-1", "snapshot-cookie-db")

	preflight, err := app.BrowserSnapshotRestorePreflight("profile-1", "snap-1")
	if err != nil {
		t.Fatalf("snapshot preflight failed: %v", err)
	}
	if len(preflight.Blockers) == 0 {
		t.Fatalf("running profile should be a snapshot preflight blocker: %+v", preflight)
	}
	err = app.BrowserSnapshotRestoreConfirmed("profile-1", "snap-1", backup.DestructiveConfirmation{
		Confirmed:         true,
		ConfirmationToken: preflight.ConfirmationToken,
	})
	if err == nil || !strings.Contains(strings.ToLower(err.Error()), "running") {
		t.Fatalf("confirmed snapshot restore should reject running profile, got err=%v", err)
	}
}

func TestBrowserSnapshotCreateRejectsUnsafeProfilePath(t *testing.T) {
	app, cleanup := newBackupTestApp(t)
	defer cleanup()
	app.browserMgr.Profiles["profile-unsafe"] = &BrowserProfile{
		ProfileId:   "profile-unsafe",
		ProfileName: "Unsafe Profile",
		UserDataDir: "../outside",
	}

	_, err := app.BrowserSnapshotCreate("profile-unsafe", "unsafe")
	if err == nil || !strings.Contains(err.Error(), "identity safety gate") {
		t.Fatalf("unsafe snapshot path should be rejected, got err=%v", err)
	}
}

func TestBrowserSnapshotRestorePreflightRejectsUnsafeProfilePath(t *testing.T) {
	app, cleanup := newBackupTestApp(t)
	defer cleanup()
	app.browserMgr.Profiles["profile-unsafe"] = &BrowserProfile{
		ProfileId:   "profile-unsafe",
		ProfileName: "Unsafe Profile",
		UserDataDir: "../outside",
	}
	writeSnapshotPackage(t, app, "profile-unsafe", "snap-unsafe", "snapshot-cookie-db")

	_, err := app.BrowserSnapshotRestorePreflight("profile-unsafe", "snap-unsafe")
	if err == nil || !strings.Contains(err.Error(), "identity safety gate") {
		t.Fatalf("unsafe snapshot restore path should be rejected, got err=%v", err)
	}
}

func writeSnapshotPackage(t *testing.T, app *App, profileId, snapshotId, cookieContent string) {
	t.Helper()
	snapDir, err := app.snapshotDir(profileId)
	if err != nil {
		t.Fatal(err)
	}
	zipPath := filepath.Join(snapDir, snapshotId+"_test.zip")
	f, err := os.Create(zipPath)
	if err != nil {
		t.Fatal(err)
	}
	zw := zip.NewWriter(f)
	w, err := zw.Create("Default/Network/Cookies")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := w.Write([]byte(cookieContent)); err != nil {
		t.Fatal(err)
	}
	if err := zw.Close(); err != nil {
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}

	info := SnapshotInfo{
		SnapshotId: snapshotId,
		ProfileId:  profileId,
		Name:       "test",
		FilePath:   zipPath,
	}
	data, err := json.Marshal(info)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(snapDir, snapshotId+"_test.meta.json"), data, 0644); err != nil {
		t.Fatal(err)
	}
}
