package backend

import (
	"ant-chrome/backend/internal/browser"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestCookieAssetVerificationReadOnlyDoesNotWriteCookieDB(t *testing.T) {
	userDataDir := t.TempDir()
	cookiePath := filepath.Join(userDataDir, "Default", "Network", "Cookies")
	if err := os.MkdirAll(filepath.Dir(cookiePath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cookiePath, []byte("cookie-db"), 0644); err != nil {
		t.Fatal(err)
	}
	oldTime := time.Now().Add(-2 * time.Hour)
	if err := os.Chtimes(cookiePath, oldTime, oldTime); err != nil {
		t.Fatal(err)
	}

	result := browser.VerifyCookieAssetReadOnly("profile-1", userDataDir)
	if len(result.Blockers) != 0 {
		t.Fatalf("read-only verification should not block a readable cookie DB: %+v", result)
	}

	got, err := os.ReadFile(cookiePath)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "cookie-db" {
		t.Fatalf("cookie verifier modified contents: %s", string(got))
	}
	info, err := os.Stat(cookiePath)
	if err != nil {
		t.Fatal(err)
	}
	if !info.ModTime().Equal(oldTime) {
		t.Fatalf("cookie verifier modified mtime: got=%s want=%s", info.ModTime(), oldTime)
	}
}

func TestCookieAssetVerificationWarnsWhenCookieDBMissing(t *testing.T) {
	result := browser.VerifyCookieAssetReadOnly("profile-1", t.TempDir())
	if len(result.Blockers) != 0 {
		t.Fatalf("missing cookie DB should warn, not block: %+v", result)
	}
	if len(result.Warnings) == 0 {
		t.Fatalf("missing cookie DB should produce a warning: %+v", result)
	}
}

func TestCookieAssetVerificationBlocksEmptyProfilePath(t *testing.T) {
	result := browser.VerifyCookieAssetReadOnly("profile-1", " ")
	if len(result.Blockers) == 0 {
		t.Fatalf("empty user data dir should block: %+v", result)
	}
}
