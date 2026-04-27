package backend

import (
	"ant-chrome/backend/internal/browser"
	"ant-chrome/backend/internal/config"
	"testing"
)

func TestProfileCreateIgnoresLegacyMaxProfileLimit(t *testing.T) {
	root := t.TempDir()
	cfg := config.DefaultConfig()
	cfg.App.MaxProfileLimit = 1

	mgr := browser.NewManager(cfg, root)

	first, err := mgr.Create(browser.ProfileInput{ProfileName: "p1"})
	if err != nil {
		t.Fatalf("first create failed: %v", err)
	}
	if first == nil {
		t.Fatalf("first create returned nil profile")
	}

	second, err := mgr.Create(browser.ProfileInput{ProfileName: "p2"})
	if err != nil {
		t.Fatalf("second create should not be blocked by profile limit in local edition: %v", err)
	}
	if second == nil {
		t.Fatalf("second create returned nil profile")
	}
}

func TestProfileCopyIgnoresLegacyMaxProfileLimit(t *testing.T) {
	root := t.TempDir()
	cfg := config.DefaultConfig()
	cfg.App.MaxProfileLimit = 1

	mgr := browser.NewManager(cfg, root)

	created, err := mgr.Create(browser.ProfileInput{ProfileName: "source"})
	if err != nil {
		t.Fatalf("create source profile failed: %v", err)
	}

	cloned, err := mgr.Copy(created.ProfileId, "clone-1")
	if err != nil {
		t.Fatalf("copy should not be blocked by profile limit in local edition: %v", err)
	}
	if cloned == nil {
		t.Fatalf("copy returned nil profile")
	}
}
