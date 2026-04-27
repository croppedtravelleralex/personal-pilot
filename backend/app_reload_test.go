package backend

import (
	"ant-chrome/backend/internal/config"
	"path/filepath"
	"testing"
)

func TestReloadConfigLoadsFromDisk(t *testing.T) {
	root := t.TempDir()

	cfg := config.DefaultConfig()
	cfg.App.Name = "Reload-Test-App"
	if err := cfg.Save(filepath.Join(root, "config.yaml")); err != nil {
		t.Fatalf("save test config failed: %v", err)
	}

	app := NewApp(root)
	app.config = config.DefaultConfig()

	if err := app.ReloadConfig(); err != nil {
		t.Fatalf("ReloadConfig failed: %v", err)
	}

	if app.config == nil {
		t.Fatalf("ReloadConfig left config nil")
	}
	if app.config.App.Name != "Reload-Test-App" {
		t.Fatalf("ReloadConfig did not load latest app name, got=%q", app.config.App.Name)
	}
}

func TestReloadConfigNormalizesLegacyLocalLicenseState(t *testing.T) {
	root := t.TempDir()
	configPath := filepath.Join(root, "config.yaml")

	cfg := config.DefaultConfig()
	cfg.App.MaxProfileLimit = 50
	cfg.App.UsedCDKeys = []string{"LEGACY-ONE", "LEGACY-TWO"}
	if err := cfg.Save(configPath); err != nil {
		t.Fatalf("save test config failed: %v", err)
	}
	if err := saveLocalLicenseState(configPath, &localLicenseState{
		MaxProfileLimit: 120,
		UsedCDKeys:      []string{"OLD-KEY"},
	}); err != nil {
		t.Fatalf("save local license state failed: %v", err)
	}

	app := NewApp(root)
	app.config = config.DefaultConfig()

	if err := app.ReloadConfig(); err != nil {
		t.Fatalf("ReloadConfig failed: %v", err)
	}

	if app.config.App.MaxProfileLimit != config.DefaultMaxProfileLimit {
		t.Fatalf("ReloadConfig should normalize max profile limit to local unlimited mode, got=%d", app.config.App.MaxProfileLimit)
	}
	if len(app.config.App.UsedCDKeys) != 0 {
		t.Fatalf("ReloadConfig should clear used cd keys, got=%+v", app.config.App.UsedCDKeys)
	}

	state, exists, err := loadLocalLicenseState(configPath)
	if err != nil {
		t.Fatalf("load local license state failed: %v", err)
	}
	if !exists {
		t.Fatalf("existing local license file should still exist")
	}
	if state.MaxProfileLimit != config.DefaultMaxProfileLimit {
		t.Fatalf("local license file max profile limit should be normalized, got=%d", state.MaxProfileLimit)
	}
	if len(state.UsedCDKeys) != 0 {
		t.Fatalf("local license file used keys should be cleared, got=%+v", state.UsedCDKeys)
	}
}
