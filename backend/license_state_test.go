package backend

import (
	appconfig "ant-chrome/backend/internal/config"
	"path/filepath"
	"testing"
)

func TestLoadConfigNormalizesLegacyLocalLicenseState(t *testing.T) {
	root := t.TempDir()
	configPath := filepath.Join(root, "config.yaml")

	cfg := appconfig.DefaultConfig()
	cfg.App.MaxProfileLimit = 88
	cfg.App.UsedCDKeys = []string{"LEGACY-A", "LEGACY-B"}
	if err := cfg.Save(configPath); err != nil {
		t.Fatalf("save config failed: %v", err)
	}
	if err := saveLocalLicenseState(configPath, &localLicenseState{
		MaxProfileLimit: 188,
		UsedCDKeys:      []string{"LOCAL-AAA", "LOCAL-BBB"},
	}); err != nil {
		t.Fatalf("save local license state failed: %v", err)
	}

	loaded, err := LoadConfig(configPath)
	if err != nil {
		t.Fatalf("LoadConfig failed: %v", err)
	}

	if loaded.App.MaxProfileLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("max profile limit should be normalized to local unlimited mode, got=%d", loaded.App.MaxProfileLimit)
	}
	if len(loaded.App.UsedCDKeys) != 0 {
		t.Fatalf("used cd keys should be cleared, got=%+v", loaded.App.UsedCDKeys)
	}

	state, exists, err := loadLocalLicenseState(configPath)
	if err != nil {
		t.Fatalf("load local license state failed: %v", err)
	}
	if !exists {
		t.Fatalf("existing local license file should be kept and normalized")
	}
	if state.MaxProfileLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("local license file max profile limit should be normalized, got=%d", state.MaxProfileLimit)
	}
	if len(state.UsedCDKeys) != 0 {
		t.Fatalf("local license file used keys should be cleared, got=%+v", state.UsedCDKeys)
	}
}

func TestLoadConfigDoesNotCreateLocalLicenseStateWhenMissing(t *testing.T) {
	root := t.TempDir()
	configPath := filepath.Join(root, "config.yaml")

	cfg := appconfig.DefaultConfig()
	cfg.App.MaxProfileLimit = 66
	cfg.App.UsedCDKeys = []string{"LEGACY-C"}
	if err := cfg.Save(configPath); err != nil {
		t.Fatalf("save config failed: %v", err)
	}

	loaded, err := LoadConfig(configPath)
	if err != nil {
		t.Fatalf("LoadConfig failed: %v", err)
	}
	if loaded.App.MaxProfileLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("max profile limit should be normalized to local unlimited mode, got=%d", loaded.App.MaxProfileLimit)
	}
	if len(loaded.App.UsedCDKeys) != 0 {
		t.Fatalf("used cd keys should be cleared, got=%+v", loaded.App.UsedCDKeys)
	}

	state, exists, err := loadLocalLicenseState(configPath)
	if err != nil {
		t.Fatalf("load local license state failed: %v", err)
	}
	if exists {
		t.Fatalf("local unlimited mode should not create local license file on read when it does not exist: %+v", state)
	}
}

func TestRedeemGithubStarNormalizesLegacyLocalLicenseState(t *testing.T) {
	root := t.TempDir()
	configPath := filepath.Join(root, "config.yaml")

	cfg := appconfig.DefaultConfig()
	cfg.App.MaxProfileLimit = 30
	cfg.App.UsedCDKeys = []string{"LEGACY-D"}
	if err := cfg.Save(configPath); err != nil {
		t.Fatalf("save config failed: %v", err)
	}
	if err := saveLocalLicenseState(configPath, &localLicenseState{
		MaxProfileLimit: 90,
		UsedCDKeys:      []string{"OLD-KEY"},
	}); err != nil {
		t.Fatalf("save local license state failed: %v", err)
	}

	app := NewApp(root)
	app.config = cfg

	if err := app.RedeemGithubStar(); err != nil {
		t.Fatalf("RedeemGithubStar failed: %v", err)
	}

	state, exists, err := loadLocalLicenseState(configPath)
	if err != nil {
		t.Fatalf("load local license state failed: %v", err)
	}
	if !exists {
		t.Fatalf("existing local license file should still exist after redeem compatibility call")
	}
	if state.MaxProfileLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("local license file max profile limit should be normalized, got=%d", state.MaxProfileLimit)
	}
	if len(state.UsedCDKeys) != 0 {
		t.Fatalf("local license file used keys should be cleared, got=%+v", state.UsedCDKeys)
	}
}
