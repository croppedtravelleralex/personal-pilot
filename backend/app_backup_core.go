package backend

import (
	"fmt"
	"os"
	"sort"
	"strings"
	"time"

	"personal-pilot/backend/internal/backup"
	"personal-pilot/backend/internal/config"
	"personal-pilot/backend/internal/events"

	wailsruntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

var validBackupTables = map[string]bool{
	"browser_profiles": true, "browser_groups": true,
	"browser_proxies": true, "browser_cores": true,
	"browser_bookmarks": true, "launch_codes": true,
}

func isValidTableName(name string) bool {
	return validBackupTables[name]
}

type backupProgressMeta struct {
	ComponentID   string
	ComponentName string
	EntryIndex    int
	EntryTotal    int
}

type backupProgressEvent struct {
	Phase         string `json:"phase"`
	Progress      int    `json:"progress"`
	Message       string `json:"message"`
	ComponentID   string `json:"componentId,omitempty"`
	ComponentName string `json:"componentName,omitempty"`
	EntryIndex    int    `json:"entryIndex,omitempty"`
	EntryTotal    int    `json:"entryTotal,omitempty"`
	Timestamp     string `json:"timestamp,omitempty"`
}

// BackupInitializeSystem 初始化系统到最开始状态。
func (a *App) BackupInitializeSystem() (map[string]interface{}, error) {
	a.maintenanceMu.Lock()
	defer a.maintenanceMu.Unlock()

	return nil, backupDestructiveConfirmationRequired("backup initialize")
}

func (a *App) BackupInitializeSystemPreflight() (backup.DestructivePreflight, error) {
	a.maintenanceMu.Lock()
	defer a.maintenanceMu.Unlock()

	return a.backupInitializePreflightLocked(), nil
}

func (a *App) BackupInitializeSystemConfirmed(confirmation backup.DestructiveConfirmation) (map[string]interface{}, error) {
	a.maintenanceMu.Lock()
	defer a.maintenanceMu.Unlock()

	preflight := a.backupInitializePreflightLocked()
	if err := preflight.ValidateConfirmation(confirmation); err != nil {
		return nil, err
	}
	return a.backupInitializeLocked(true)
}

// BackupExportPackage 导出全量配置与数据到 ZIP。
func (a *App) BackupExportPackage() (map[string]interface{}, error) {
	a.maintenanceMu.Lock()
	defer a.maintenanceMu.Unlock()

	if a.ctx == nil {
		return nil, fmt.Errorf("应用上下文未初始化")
	}
	if events.HasFrontendEmitter() {
		return nil, fmt.Errorf("backup export requires a native file dialog bridge in Tauri mode")
	}
	a.backupEmitExportProgress("starting", 0, "等待选择导出路径...")

	defaultName := fmt.Sprintf("personal-pilot-backup-%s.zip", time.Now().Format("20060102-150405"))
	savePath, err := wailsruntime.SaveFileDialog(a.ctx, wailsruntime.SaveDialogOptions{
		Title:           "导出配置",
		DefaultFilename: defaultName,
		Filters: []wailsruntime.FileFilter{
			{DisplayName: "ZIP 文件 (*.zip)", Pattern: "*.zip"},
		},
	})
	if err != nil {
		return nil, fmt.Errorf("打开保存对话框失败: %w", err)
	}
	if strings.TrimSpace(savePath) == "" {
		a.backupEmitExportProgress("cancelled", 0, "已取消导出")
		return map[string]interface{}{
			"cancelled": true,
			"message":   "已取消导出",
		}, nil
	}
	return a.backupExportPackageToPathLocked(savePath)
}

// BackupExportPackageToPath exports a backup to an explicit path. Tauri uses
// this after selecting the file path through its native dialog bridge.
func (a *App) BackupExportPackageToPath(savePath string) (map[string]interface{}, error) {
	a.maintenanceMu.Lock()
	defer a.maintenanceMu.Unlock()

	if a.ctx == nil {
		return nil, fmt.Errorf("application context is not initialized")
	}
	return a.backupExportPackageToPathLocked(savePath)
}

func (a *App) backupExportPackageToPathLocked(savePath string) (map[string]interface{}, error) {
	savePath = strings.TrimSpace(savePath)
	if savePath == "" {
		a.backupEmitExportProgress("cancelled", 0, "export cancelled")
		return map[string]interface{}{
			"cancelled": true,
			"message":   "export cancelled",
		}, nil
	}
	savePath = backupEnsureZipSuffix(savePath)
	a.backupEmitExportProgress("preparing", 8, "正在收集导出范围...")

	scope, err := backup.BuildScope(backup.BuildOptions{AppRoot: a.appRoot, Config: a.config})
	if err != nil {
		a.backupEmitExportProgress("error", 100, fmt.Sprintf("导出失败: %v", err))
		return nil, err
	}
	manifest := backup.BuildManifest(scope, a.appName(), a.appVersion(), time.Now())
	a.backupEmitExportProgress("preparing", 15, "开始写入备份包...")

	includedEntries, skippedEntries, fileCount, err := backupWritePackageZip(savePath, scope, manifest, a.backupEmitExportProgressMeta)
	if err != nil {
		a.backupEmitExportProgress("error", 100, fmt.Sprintf("导出失败: %v", err))
		return nil, err
	}

	encPath := savePath + ".enc"
	a.backupEmitExportProgress("encrypting", 95, "正在加密导出文件...")
	if err := backup.EncryptFile(savePath, encPath); err != nil {
		a.backupEmitExportProgress("error", 100, fmt.Sprintf("加密失败: %v", err))
		return nil, fmt.Errorf("backup encrypt: %w", err)
	}
	os.Remove(savePath)

	return map[string]interface{}{
		"cancelled":       false,
		"zipPath":         encPath,
		"includedEntries": includedEntries,
		"skippedEntries":  skippedEntries,
		"fileCount":       fileCount,
		"message":         "导出完成",
	}, nil
}

func (a *App) backupEmitExportProgress(phase string, progress int, message string) {
	a.backupEmitExportProgressMeta(phase, progress, message, nil)
}

func (a *App) backupEmitExportProgressMeta(phase string, progress int, message string, meta *backupProgressMeta) {
	a.backupEmitProgress("backup:export:progress", phase, progress, message, meta)
}

func (a *App) backupEmitImportProgress(phase string, progress int, message string) {
	a.backupEmitImportProgressMeta(phase, progress, message, nil)
}

func (a *App) backupEmitImportProgressMeta(phase string, progress int, message string, meta *backupProgressMeta) {
	a.backupEmitProgress("backup:import:progress", phase, progress, message, meta)
}

func (a *App) backupEmitProgress(eventName, phase string, progress int, message string, meta *backupProgressMeta) {
	if a == nil || a.ctx == nil {
		return
	}
	if progress < 0 {
		progress = 0
	}
	if progress > 100 {
		progress = 100
	}
	evt := backupProgressEvent{
		Phase:     strings.TrimSpace(phase),
		Progress:  progress,
		Message:   strings.TrimSpace(message),
		Timestamp: time.Now().Format("15:04:05"),
	}
	if meta != nil {
		evt.ComponentID = strings.TrimSpace(meta.ComponentID)
		evt.ComponentName = strings.TrimSpace(meta.ComponentName)
		evt.EntryIndex = meta.EntryIndex
		evt.EntryTotal = meta.EntryTotal
	}
	a.emit(eventName, backupProgressEvent{
		Phase:         evt.Phase,
		Progress:      evt.Progress,
		Message:       evt.Message,
		ComponentID:   evt.ComponentID,
		ComponentName: evt.ComponentName,
		EntryIndex:    evt.EntryIndex,
		EntryTotal:    evt.EntryTotal,
		Timestamp:     evt.Timestamp,
	})
}

func backupDestructiveConfirmationRequired(operation string) error {
	return fmt.Errorf("%s requires destructive preflight confirmation", operation)
}

func (a *App) backupRunningProfileIDsLocked() []string {
	if a == nil || a.browserMgr == nil {
		return nil
	}
	a.browserMgr.Mutex.Lock()
	defer a.browserMgr.Mutex.Unlock()

	ids := make([]string, 0)
	for profileID, profile := range a.browserMgr.Profiles {
		if profile == nil || !profile.Running {
			continue
		}
		id := strings.TrimSpace(profile.ProfileId)
		if id == "" {
			id = strings.TrimSpace(profileID)
		}
		if id != "" {
			ids = append(ids, id)
		}
	}
	sort.Strings(ids)
	return ids
}

func (a *App) backupInitializePreflightLocked() backup.DestructivePreflight {
	oldCfg := a.config
	if oldCfg == nil {
		oldCfg = config.DefaultConfig()
	}
	defaultCfg := config.DefaultConfig()
	dataRoot := a.resolveAppPath("data")
	oldUserRoot := a.backupResolveUserDataRoot(oldCfg)
	newUserRoot := a.backupResolveUserDataRoot(defaultCfg)

	preflight := backup.DestructivePreflight{
		Operation:            "backup_initialize",
		RequiresConfirmation: true,
		Overwrites:           []string{a.resolveAppPath("config.yaml"), a.resolveAppPath("proxies.yaml"), dataRoot},
		DestructivePaths:     []string{dataRoot},
		ConfirmationPrompt:   "Reset application data to defaults. This may delete profile data and Cookie assets.",
	}
	for _, p := range backupUniqueNonEmpty([]string{oldUserRoot, newUserRoot}) {
		if backupSamePath(p, dataRoot) {
			continue
		}
		preflight.Overwrites = append(preflight.Overwrites, p)
		preflight.DestructivePaths = append(preflight.DestructivePaths, p)
	}
	if running := a.backupRunningProfileIDsLocked(); len(running) > 0 {
		preflight.RequiresStop = true
		preflight.AddBlocker("profile_running", "running profiles must be stopped before backup initialize: "+strings.Join(running, ","))
	}
	preflight.Finalize()
	return preflight
}
