package browser

import (
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/config"
	goruntime "runtime"
	"strings"
	"testing"
)

func TestIdentityGuardRejectsDuplicateCanonicalUserDataDir(t *testing.T) {
	root := t.TempDir()
	cfg := config.DefaultConfig()
	cfg.Browser.UserDataRoot = filepath.Join(root, "data")
	mgr := NewManager(cfg, root)

	absProfileDir := filepath.Join(root, "data", "profile-a")
	mgr.Profiles = map[string]*Profile{
		"profile-a": {ProfileId: "profile-a", UserDataDir: "profile-a"},
		"profile-b": {ProfileId: "profile-b", UserDataDir: absProfileDir},
	}

	err := mgr.NormalizeProfileIdentityBindings()
	if err == nil {
		t.Fatal("expected duplicate canonical user-data-dir to be rejected")
	}
	if !strings.Contains(strings.ToLower(err.Error()), "duplicate") {
		t.Fatalf("expected duplicate error, got %v", err)
	}
}

func TestIdentityGuardRejectsPathTraversal(t *testing.T) {
	root := t.TempDir()
	cfg := config.DefaultConfig()
	cfg.Browser.UserDataRoot = filepath.Join(root, "data")
	mgr := NewManager(cfg, root)

	_, err := mgr.ResolveCanonicalUserDataDir(&Profile{
		ProfileId:   "escape",
		UserDataDir: filepath.Join("..", "escape"),
	})
	if err == nil {
		t.Fatal("expected path traversal to be rejected")
	}
	if !strings.Contains(strings.ToLower(err.Error()), "traversal") {
		t.Fatalf("expected traversal error, got %v", err)
	}
}

func TestIdentityGuardRejectsAbsolutePathOutsideRoot(t *testing.T) {
	root := t.TempDir()
	cfg := config.DefaultConfig()
	cfg.Browser.UserDataRoot = filepath.Join(root, "data")
	mgr := NewManager(cfg, root)

	_, err := mgr.ResolveCanonicalUserDataDir(&Profile{
		ProfileId:   "outside",
		UserDataDir: filepath.Join(root, "outside-profile"),
	})
	if err == nil {
		t.Fatal("expected absolute path outside user-data root to be rejected")
	}
	if !strings.Contains(strings.ToLower(err.Error()), "escapes") {
		t.Fatalf("expected escapes-root error, got %v", err)
	}
}

func TestIdentityGuardRejectsSymlinkUserDataDir(t *testing.T) {
	root := t.TempDir()
	dataRoot := filepath.Join(root, "data")
	if err := os.MkdirAll(dataRoot, 0755); err != nil {
		t.Fatalf("mkdir data root: %v", err)
	}
	outside := filepath.Join(root, "outside")
	if err := os.MkdirAll(outside, 0755); err != nil {
		t.Fatalf("mkdir outside: %v", err)
	}
	link := filepath.Join(dataRoot, "linked-profile")
	if err := os.Symlink(outside, link); err != nil {
		t.Skipf("symlink unavailable in this environment: %v", err)
	}

	cfg := config.DefaultConfig()
	cfg.Browser.UserDataRoot = dataRoot
	mgr := NewManager(cfg, root)

	_, err := mgr.ResolveCanonicalUserDataDir(&Profile{
		ProfileId:   "linked",
		UserDataDir: "linked-profile",
	})
	if err == nil {
		t.Fatal("expected symlink user-data-dir to be rejected")
	}
	if !strings.Contains(strings.ToLower(err.Error()), "ambiguous") {
		t.Fatalf("expected ambiguous ownership error, got %v", err)
	}
}

func TestIdentityGuardRejectsCaseOnlyDuplicateOnWindows(t *testing.T) {
	if goruntime.GOOS != "windows" {
		t.Skip("case-only duplicate identity is a Windows path rule")
	}

	root := t.TempDir()
	cfg := config.DefaultConfig()
	cfg.Browser.UserDataRoot = filepath.Join(root, "data")
	mgr := NewManager(cfg, root)

	absProfileDir := filepath.Join(root, "data", "CaseProfile")
	mgr.Profiles = map[string]*Profile{
		"profile-a": {ProfileId: "profile-a", UserDataDir: absProfileDir},
		"profile-b": {ProfileId: "profile-b", UserDataDir: strings.ToUpper(absProfileDir)},
	}

	err := mgr.NormalizeProfileIdentityBindings()
	if err == nil {
		t.Fatal("expected case-only duplicate canonical user-data-dir to be rejected on Windows")
	}
	if !strings.Contains(strings.ToLower(err.Error()), "duplicate") {
		t.Fatalf("expected duplicate error, got %v", err)
	}
}

func TestIdentityGuardRejectsConcurrentProfileStartLock(t *testing.T) {
	mgr := NewManager(config.DefaultConfig(), t.TempDir())

	release, err := mgr.AcquireProfileStartLock("profile-a")
	if err != nil {
		t.Fatalf("acquire first start lock: %v", err)
	}
	defer release()

	if _, err := mgr.AcquireProfileStartLock("profile-a"); err == nil {
		t.Fatal("expected second start lock for same profile to be rejected")
	} else if !strings.Contains(strings.ToLower(err.Error()), "already in progress") {
		t.Fatalf("expected in-progress error, got %v", err)
	}

	release()
	reacquired, err := mgr.AcquireProfileStartLock("profile-a")
	if err != nil {
		t.Fatalf("expected lock to be reusable after release: %v", err)
	}
	reacquired()
}
