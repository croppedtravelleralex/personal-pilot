package backend

import (
	appconfig "ant-chrome/backend/internal/config"
	"fmt"
)

const localUnlimitedCDKeyPrefix = "LOCAL-UNLIMITED"

// LicenseStatus represents the frontend-compatible license payload.
type LicenseStatus struct {
	MaxLimit  int      `json:"maxLimit"`
	UsedCount int      `json:"usedCount"`
	UsedKeys  []string `json:"usedKeys"`
}

func (a *App) normalizeLocalUnlimitedLicenseInMemory() {
	if a.config == nil {
		a.config = DefaultConfig()
	}
	a.config.App.MaxProfileLimit = appconfig.DefaultMaxProfileLimit
	a.config.App.UsedCDKeys = []string{}
}

func (a *App) persistLocalUnlimitedLicense() error {
	a.normalizeLocalUnlimitedLicenseInMemory()
	configPath := a.resolveAppPath("config.yaml")
	if _, _, err := reconcileConfigWithLocalLicense(configPath, a.config); err != nil {
		return fmt.Errorf("sync local license state failed: %w", err)
	}
	if err := a.config.Save(configPath); err != nil {
		return fmt.Errorf("save config failed: %w", err)
	}
	return nil
}

// GetLicenseStatus returns local-edition unlimited status for compatibility.
func (a *App) GetLicenseStatus() LicenseStatus {
	a.normalizeLocalUnlimitedLicenseInMemory()

	profilesCount := 0
	if a.browserMgr != nil {
		profilesCount = len(a.browserMgr.List())
	}

	return LicenseStatus{
		MaxLimit:  appconfig.DefaultMaxProfileLimit,
		UsedCount: profilesCount,
		UsedKeys:  []string{},
	}
}

// RedeemCDKey keeps API compatibility in local edition.
func (a *App) RedeemCDKey(_ string) error {
	return a.persistLocalUnlimitedLicense()
}

// RedeemGithubStar keeps API compatibility in local edition.
func (a *App) RedeemGithubStar() error {
	return a.persistLocalUnlimitedLicense()
}

// GenerateCDKeys keeps API compatibility by returning local placeholders.
func (a *App) GenerateCDKeys(count int) ([]string, error) {
	if count <= 0 || count > 1000 {
		return nil, fmt.Errorf("invalid key count: expected 1-1000")
	}

	keys := make([]string, 0, count)
	for i := 0; i < count; i++ {
		keys = append(keys, fmt.Sprintf("%s-%04d", localUnlimitedCDKeyPrefix, i+1))
	}

	return keys, nil
}
