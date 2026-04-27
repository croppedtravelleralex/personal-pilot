package backend

import (
	appconfig "ant-chrome/backend/internal/config"
	"path/filepath"
	"strings"
	"testing"
)

func TestGetLicenseStatusReturnsLocalUnlimitedCompatibility(t *testing.T) {
	app := NewApp(t.TempDir())
	app.config = DefaultConfig()
	app.config.App.MaxProfileLimit = 99
	app.config.App.UsedCDKeys = []string{"LEGACY-A"}

	status := app.GetLicenseStatus()

	if status.MaxLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("GetLicenseStatus maxLimit should be local unlimited default, got=%d", status.MaxLimit)
	}
	if status.UsedCount != 0 {
		t.Fatalf("GetLicenseStatus usedCount should be 0 when browser manager is not initialized, got=%d", status.UsedCount)
	}
	if len(status.UsedKeys) != 0 {
		t.Fatalf("GetLicenseStatus usedKeys should be empty, got=%+v", status.UsedKeys)
	}
	if app.config.App.MaxProfileLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("in-memory config max profile limit should be normalized, got=%d", app.config.App.MaxProfileLimit)
	}
	if len(app.config.App.UsedCDKeys) != 0 {
		t.Fatalf("in-memory config used cd keys should be cleared, got=%+v", app.config.App.UsedCDKeys)
	}
}

func TestRedeemCDKeyNoDependencyInLocalEdition(t *testing.T) {
	root := t.TempDir()
	configPath := filepath.Join(root, "config.yaml")

	cfg := appconfig.DefaultConfig()
	cfg.App.MaxProfileLimit = 77
	cfg.App.UsedCDKeys = []string{"LEGACY-B"}

	app := NewApp(root)
	app.config = cfg

	if err := app.RedeemCDKey(""); err != nil {
		t.Fatalf("RedeemCDKey should be a no-op compatibility API in local edition: %v", err)
	}
	if err := app.RedeemCDKey("ANY-INPUT-IS-IGNORED"); err != nil {
		t.Fatalf("RedeemCDKey should keep returning success in local edition: %v", err)
	}

	if app.config.App.MaxProfileLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("in-memory config max profile limit should be normalized, got=%d", app.config.App.MaxProfileLimit)
	}
	if len(app.config.App.UsedCDKeys) != 0 {
		t.Fatalf("in-memory config used cd keys should be cleared, got=%+v", app.config.App.UsedCDKeys)
	}

	persisted, err := appconfig.Load(configPath)
	if err != nil {
		t.Fatalf("load persisted config failed: %v", err)
	}
	if persisted.App.MaxProfileLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("persisted config max profile limit should be normalized, got=%d", persisted.App.MaxProfileLimit)
	}
	if len(persisted.App.UsedCDKeys) != 0 {
		t.Fatalf("persisted config used cd keys should be cleared, got=%+v", persisted.App.UsedCDKeys)
	}

	_, exists, err := loadLocalLicenseState(configPath)
	if err != nil {
		t.Fatalf("load local license state failed: %v", err)
	}
	if exists {
		t.Fatalf("RedeemCDKey should not create local license file when one does not exist")
	}
}

func TestRedeemGithubStarNoDependencyInLocalEdition(t *testing.T) {
	root := t.TempDir()
	configPath := filepath.Join(root, "config.yaml")

	cfg := appconfig.DefaultConfig()
	cfg.App.MaxProfileLimit = 77
	cfg.App.UsedCDKeys = []string{"LEGACY-C"}

	app := NewApp(root)
	app.config = cfg

	if err := app.RedeemGithubStar(); err != nil {
		t.Fatalf("RedeemGithubStar should be a no-op compatibility API in local edition: %v", err)
	}

	if app.config.App.MaxProfileLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("in-memory config max profile limit should be normalized, got=%d", app.config.App.MaxProfileLimit)
	}
	if len(app.config.App.UsedCDKeys) != 0 {
		t.Fatalf("in-memory config used cd keys should be cleared, got=%+v", app.config.App.UsedCDKeys)
	}

	persisted, err := appconfig.Load(configPath)
	if err != nil {
		t.Fatalf("load persisted config failed: %v", err)
	}
	if persisted.App.MaxProfileLimit != appconfig.DefaultMaxProfileLimit {
		t.Fatalf("persisted config max profile limit should be normalized, got=%d", persisted.App.MaxProfileLimit)
	}
	if len(persisted.App.UsedCDKeys) != 0 {
		t.Fatalf("persisted config used cd keys should be cleared, got=%+v", persisted.App.UsedCDKeys)
	}

	_, exists, err := loadLocalLicenseState(configPath)
	if err != nil {
		t.Fatalf("load local license state failed: %v", err)
	}
	if exists {
		t.Fatalf("RedeemGithubStar should not create local license file when one does not exist")
	}
}

func TestGenerateCDKeysReturnsLocalPlaceholders(t *testing.T) {
	app := NewApp(t.TempDir())

	keys, err := app.GenerateCDKeys(3)
	if err != nil {
		t.Fatalf("GenerateCDKeys failed: %v", err)
	}
	if len(keys) != 3 {
		t.Fatalf("GenerateCDKeys should return 3 keys, got=%d", len(keys))
	}
	for _, key := range keys {
		if !strings.HasPrefix(key, localUnlimitedCDKeyPrefix+"-") {
			t.Fatalf("unexpected generated key format: %s", key)
		}
	}

	if _, err := app.GenerateCDKeys(0); err == nil {
		t.Fatalf("GenerateCDKeys should reject non-positive count")
	}
	if _, err := app.GenerateCDKeys(1001); err == nil {
		t.Fatalf("GenerateCDKeys should reject count over 1000")
	}
}
