package browser

import (
	"ant-chrome/backend/internal/logger"
	"encoding/json"
	"os"
	"path/filepath"
)

// EnsurePreferences deep-merges overrides into the Chrome Default/Preferences JSON file.
// If overrides is nil or empty, this is a no-op and returns nil immediately.
// If the Preferences file does not exist (first launch), it is created from scratch.
func EnsurePreferences(userDataDir string, overrides map[string]interface{}) error {
	if len(overrides) == 0 {
		return nil
	}

	log := logger.New("Browser")
	prefsPath := filepath.Join(userDataDir, "Default", "Preferences")

	var existing map[string]interface{}

	data, err := os.ReadFile(prefsPath)
	if err != nil {
		if os.IsNotExist(err) {
			existing = make(map[string]interface{})
		} else {
			log.Error("读取 Chrome Preferences 失败", logger.F("path", prefsPath), logger.F("error", err))
			return err
		}
	} else {
		if err := json.Unmarshal(data, &existing); err != nil {
			log.Warn("Preferences JSON 解析失败，使用空配置覆盖", logger.F("path", prefsPath), logger.F("error", err))
			existing = make(map[string]interface{})
		}
	}

	deepMerge(existing, overrides)

	out, err := json.MarshalIndent(existing, "", "  ")
	if err != nil {
		log.Error("序列化 Preferences 失败", logger.F("error", err))
		return err
	}

	// Ensure the Default directory exists
	dir := filepath.Dir(prefsPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		log.Error("创建 Preferences 目录失败", logger.F("dir", dir), logger.F("error", err))
		return err
	}

	if err := os.WriteFile(prefsPath, out, 0644); err != nil {
		log.Error("写入 Preferences 失败", logger.F("path", prefsPath), logger.F("error", err))
		return err
	}

	log.Info("Chrome Preferences 已注入", logger.F("path", prefsPath))
	return nil
}

// deepMerge merges src into dst. Both maps are treated as mutable copies.
// For scalar values, src wins. For nested maps, recursion is used.
// Arrays in src replace arrays in dst entirely (no concatenation).
func deepMerge(dst, src map[string]interface{}) {
	for key, srcVal := range src {
		dstVal, exists := dst[key]
		if !exists {
			dst[key] = srcVal
			continue
		}

		dstMap, dstIsMap := dstVal.(map[string]interface{})
		srcMap, srcIsMap := srcVal.(map[string]interface{})

		if dstIsMap && srcIsMap {
			deepMerge(dstMap, srcMap)
		} else {
			dst[key] = srcVal
		}
	}
}
