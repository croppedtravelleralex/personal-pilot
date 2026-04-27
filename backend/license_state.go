package backend

import (
	appconfig "ant-chrome/backend/internal/config"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const localLicenseStateFilename = ".ant-license.json"

type localLicenseState struct {
	MaxProfileLimit int      `json:"maxProfileLimit"`
	UsedCDKeys      []string `json:"usedCdKeys,omitempty"`
}

func localLicenseStatePath(configPath string) string {
	configPath = strings.TrimSpace(configPath)
	if configPath == "" {
		return localLicenseStateFilename
	}

	dir := filepath.Dir(configPath)
	if dir == "." || dir == "" {
		if cwd, err := os.Getwd(); err == nil {
			dir = cwd
		}
	}
	return filepath.Join(dir, localLicenseStateFilename)
}

func loadLocalLicenseState(configPath string) (*localLicenseState, bool, error) {
	statePath := localLicenseStatePath(configPath)
	data, err := os.ReadFile(statePath)
	if err != nil {
		if os.IsNotExist(err) {
			return &localLicenseState{}, false, nil
		}
		return nil, false, fmt.Errorf("read local license state failed: %w", err)
	}

	var state localLicenseState
	if err := json.Unmarshal(data, &state); err != nil {
		// Keep startup resilient on corrupted legacy state.
		return &localLicenseState{}, false, nil
	}

	normalizeLocalLicenseState(&state)
	return &state, true, nil
}

func saveLocalLicenseState(configPath string, state *localLicenseState) error {
	if state == nil {
		state = &localLicenseState{}
	}

	cloned := *state
	normalizeLocalLicenseState(&cloned)

	data, err := json.MarshalIndent(cloned, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal local license state failed: %w", err)
	}
	if err := os.WriteFile(localLicenseStatePath(configPath), data, 0o644); err != nil {
		return fmt.Errorf("write local license state failed: %w", err)
	}
	return nil
}

func reconcileConfigWithLocalLicense(configPath string, cfg *Config) (bool, bool, error) {
	if cfg == nil {
		return false, false, nil
	}

	originalMax := cfg.App.MaxProfileLimit
	originalUsed := normalizeUsedCDKeys(cfg.App.UsedCDKeys)

	state, stateExists, err := loadLocalLicenseState(configPath)
	if err != nil {
		return false, false, err
	}

	cfg.App.MaxProfileLimit = appconfig.DefaultMaxProfileLimit
	cfg.App.UsedCDKeys = []string{}

	desiredState := &localLicenseState{
		MaxProfileLimit: appconfig.DefaultMaxProfileLimit,
		UsedCDKeys:      []string{},
	}
	normalizeLocalLicenseState(desiredState)

	stateChanged := state.MaxProfileLimit != desiredState.MaxProfileLimit || !sameStringSlice(state.UsedCDKeys, desiredState.UsedCDKeys)
	persisted := false
	if stateExists && stateChanged {
		if err := saveLocalLicenseState(configPath, desiredState); err != nil {
			return false, false, err
		}
		persisted = true
	}

	configChanged := originalMax != appconfig.DefaultMaxProfileLimit || len(originalUsed) > 0
	return configChanged, persisted, nil
}

func normalizeLocalLicenseState(state *localLicenseState) {
	if state == nil {
		return
	}
	state.MaxProfileLimit = appconfig.DefaultMaxProfileLimit
	state.UsedCDKeys = []string{}
}

func normalizeUsedCDKeys(keys []string) []string {
	result := make([]string, 0, len(keys))
	seen := make(map[string]struct{}, len(keys))
	for _, key := range keys {
		normalized := strings.ToUpper(strings.TrimSpace(key))
		if normalized == "" {
			continue
		}
		if _, ok := seen[normalized]; ok {
			continue
		}
		seen[normalized] = struct{}{}
		result = append(result, normalized)
	}
	return result
}

func sameStringSlice(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
