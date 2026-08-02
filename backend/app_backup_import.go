package backend

import (
	"context"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"personal-pilot/backend/internal/backup"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/config"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/proxy"

	wailsruntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

type backupImportIssue struct {
	ComponentID   string `json:"componentId"`
	ComponentName string `json:"componentName"`
	Error         string `json:"error"`
}

// BackupImportPackage 从 ZIP 加载配置与数据。
// resetFirst=true: 先初始化，再全量导入。
// resetFirst=false: 直接导入并执行判重合并。
func (a *App) BackupImportPackage(resetFirst bool) (map[string]interface{}, error) {
	a.maintenanceMu.Lock()
	defer a.maintenanceMu.Unlock()

	if a.ctx == nil {
		return nil, fmt.Errorf("应用上下文未初始化")
	}
	if events.HasFrontendEmitter() {
		return nil, fmt.Errorf("backup import requires a native file dialog bridge in Tauri mode")
	}
	a.backupEmitImportProgress("starting", 0, "等待选择 ZIP 配置文件...")

	zipPath, err := wailsruntime.OpenFileDialog(a.ctx, wailsruntime.OpenDialogOptions{
		Title: "加载配置",
		Filters: []wailsruntime.FileFilter{
			{DisplayName: "ZIP 文件 (*.zip)", Pattern: "*.zip"},
		},
	})
	if err != nil {
		a.backupEmitImportProgress("error", 100, fmt.Sprintf("打开文件对话框失败: %v", err))
		return nil, fmt.Errorf("打开文件对话框失败: %w", err)
	}
	if strings.TrimSpace(zipPath) == "" {
		a.backupEmitImportProgress("cancelled", 0, "已取消加载")
		return map[string]interface{}{
			"cancelled": true,
			"message":   "已取消加载",
		}, nil
	}
	a.backupEmitImportProgress("preparing", 5, "正在校验备份包...")

	return nil, backupDestructiveConfirmationRequired("backup import")
}

// BackupImportPackageFromPath imports a backup from an explicit ZIP path.
// Tauri uses this after selecting the file through its native dialog bridge.
func (a *App) BackupImportPackageFromPath(zipPath string, resetFirst bool) (map[string]interface{}, error) {
	a.maintenanceMu.Lock()
	defer a.maintenanceMu.Unlock()

	if a.ctx == nil {
		return nil, fmt.Errorf("application context is not initialized")
	}
	zipPath = strings.TrimSpace(zipPath)
	if zipPath == "" {
		a.backupEmitImportProgress("cancelled", 0, "import cancelled")
		return map[string]interface{}{
			"cancelled": true,
			"message":   "import cancelled",
		}, nil
	}
	a.backupEmitImportProgress("preparing", 5, "validating backup package...")

	return nil, backupDestructiveConfirmationRequired("backup import")
}

func (a *App) BackupImportPackagePreflightFromPath(zipPath string, resetFirst bool) (backup.DestructivePreflight, error) {
	a.maintenanceMu.Lock()
	defer a.maintenanceMu.Unlock()

	zipPath = strings.TrimSpace(zipPath)
	if zipPath == "" {
		return backup.DestructivePreflight{}, fmt.Errorf("import path is empty")
	}
	return a.backupImportPreflightFromPathLocked(zipPath, resetFirst)
}

func (a *App) BackupImportPackageFromPathConfirmed(zipPath string, resetFirst bool, confirmation backup.DestructiveConfirmation) (map[string]interface{}, error) {
	a.maintenanceMu.Lock()
	defer a.maintenanceMu.Unlock()

	if a.ctx == nil {
		return nil, fmt.Errorf("application context is not initialized")
	}
	zipPath = strings.TrimSpace(zipPath)
	if zipPath == "" {
		a.backupEmitImportProgress("cancelled", 0, "import cancelled")
		return map[string]interface{}{
			"cancelled": true,
			"message":   "import cancelled",
		}, nil
	}
	preflight, err := a.backupImportPreflightFromPathLocked(zipPath, resetFirst)
	if err != nil {
		return nil, err
	}
	if err := preflight.ValidateConfirmation(confirmation); err != nil {
		return nil, err
	}
	a.backupEmitImportProgress("preparing", 5, "validating backup package...")

	result, importErr := a.backupImportFromPathLocked(zipPath, resetFirst)
	if importErr != nil {
		a.backupEmitImportProgress("error", 100, fmt.Sprintf("import failed: %v", importErr))
		return nil, importErr
	}
	return result, nil
}

func backupPayloadContainsCookie(payloadRoot string) bool {
	payloadRoot = strings.TrimSpace(payloadRoot)
	if payloadRoot == "" {
		return false
	}
	found := false
	_ = filepath.WalkDir(payloadRoot, func(path string, d fs.DirEntry, err error) error {
		if err != nil || found {
			return nil
		}
		if d.IsDir() {
			return nil
		}
		rel, relErr := filepath.Rel(payloadRoot, path)
		if relErr != nil {
			return nil
		}
		if backupIsCookieAssetPath(rel) {
			found = true
		}
		return nil
	})
	return found
}

func backupIsCookieAssetPath(path string) bool {
	rel := strings.ToLower(strings.Trim(filepath.ToSlash(strings.TrimSpace(path)), "/"))
	if rel == "" {
		return false
	}
	parts := strings.Split(rel, "/")
	name := parts[len(parts)-1]
	if name != "cookies" && name != "cookies-journal" && name != "cookies-wal" && name != "cookies-shm" {
		return false
	}
	for _, part := range parts {
		if part == "default" || part == "network" {
			return true
		}
	}
	return false
}

func (a *App) backupCheckCookieCrossProfileRisk(preflight *backup.DestructivePreflight, incomingCfg *config.Config) {
	if a == nil || a.browserMgr == nil || preflight == nil || incomingCfg == nil {
		return
	}

	a.browserMgr.Mutex.Lock()
	existingProfiles := make([]*BrowserProfile, 0, len(a.browserMgr.Profiles))
	for _, profile := range a.browserMgr.Profiles {
		if profile == nil {
			continue
		}
		snapshot := *profile
		existingProfiles = append(existingProfiles, &snapshot)
	}
	a.browserMgr.Mutex.Unlock()

	for _, incoming := range incomingCfg.Browser.Profiles {
		incomingProfile := &BrowserProfile{
			ProfileId:   strings.TrimSpace(incoming.ProfileId),
			ProfileName: strings.TrimSpace(incoming.ProfileName),
			UserDataDir: strings.TrimSpace(incoming.UserDataDir),
		}
		if incomingProfile.ProfileId == "" {
			continue
		}
		incomingDir, err := a.browserMgr.ResolveCanonicalUserDataDir(incomingProfile)
		if err != nil {
			preflight.AddBlocker("incoming_profile_path_ambiguous", err.Error())
			continue
		}
		for _, existing := range existingProfiles {
			if existing == nil || strings.TrimSpace(existing.ProfileId) == "" || existing.ProfileId == incomingProfile.ProfileId {
				continue
			}
			existingDir, err := a.browserMgr.ResolveCanonicalUserDataDir(existing)
			if err != nil {
				preflight.AddBlocker("existing_profile_path_ambiguous", err.Error())
				continue
			}
			if backupSamePath(existingDir, incomingDir) {
				preflight.AddBlocker(
					"cookie_cross_profile",
					fmt.Sprintf("incoming profile %s would write Cookie assets into user-data-dir owned by existing profile %s", incomingProfile.ProfileId, existing.ProfileId),
				)
			}
		}
	}
}

func (a *App) backupImportPreflightFromPathLocked(zipPath string, resetFirst bool) (backup.DestructivePreflight, error) {
	extractRoot, manifest, err := backupExtractAndValidate(zipPath)
	if err != nil {
		return backup.DestructivePreflight{}, err
	}
	defer os.RemoveAll(extractRoot)

	payloadRoot := filepath.Join(extractRoot, "payload")
	componentEntries := backupDetectPresentManifestEntries(extractRoot, manifest)
	incomingCfg, hasIncomingCfg, cfgErr := backupLoadIncomingConfig(payloadRoot)

	userDataRoot := a.backupResolveUserDataRoot(a.config)
	dataRoot := a.resolveAppPath("data")
	preflight := backup.DestructivePreflight{
		Operation:            "backup_import",
		TargetUserDataDir:    userDataRoot,
		RequiresConfirmation: true,
		WritesCookie:         backupPayloadContainsCookie(payloadRoot),
		ConfirmationPrompt:   "Import backup data only after confirming target profile data and Cookie writes.",
	}
	if cfgErr != nil {
		preflight.AddBlocker("backup_config_parse_failed", "backup config parse failed: "+cfgErr.Error())
	}
	if resetFirst {
		preflight.Overwrites = append(preflight.Overwrites, a.resolveAppPath("config.yaml"), a.resolveAppPath("proxies.yaml"), dataRoot, userDataRoot)
		preflight.DestructivePaths = append(preflight.DestructivePaths, dataRoot, userDataRoot)
	} else {
		preflight.Adds = append(preflight.Adds, dataRoot, userDataRoot)
		if hasIncomingCfg {
			preflight.Overwrites = append(preflight.Overwrites, a.resolveAppPath("config.yaml"))
		}
	}
	if _, ok := componentEntries["browser_core_root"]; ok {
		if resetFirst {
			preflight.Overwrites = append(preflight.Overwrites, a.resolveAppPath("chrome"))
			preflight.DestructivePaths = append(preflight.DestructivePaths, a.resolveAppPath("chrome"))
		} else {
			preflight.Adds = append(preflight.Adds, a.resolveAppPath("chrome"))
		}
	}
	if incomingCfg != nil {
		for _, p := range a.backupCollectExternalCorePaths(incomingCfg) {
			if resetFirst {
				preflight.Overwrites = append(preflight.Overwrites, p)
				preflight.DestructivePaths = append(preflight.DestructivePaths, p)
			} else {
				preflight.Adds = append(preflight.Adds, p)
			}
		}
	}
	if running := a.backupRunningProfileIDsLocked(); len(running) > 0 {
		preflight.RequiresStop = true
		preflight.AddBlocker("profile_running", "running profiles must be stopped before backup import: "+strings.Join(running, ","))
	}
	if preflight.WritesCookie && incomingCfg != nil && !resetFirst {
		a.backupCheckCookieCrossProfileRisk(&preflight, incomingCfg)
	}
	if !preflight.WritesCookie {
		preflight.Skips = append(preflight.Skips, "cookie_assets")
	}
	preflight.Finalize()
	return preflight, nil
}

func (a *App) backupImportFromPathLocked(zipPath string, resetFirst bool) (map[string]interface{}, error) {
	a.backupStopRuntimeForMaintenance()
	a.backupEmitImportProgress("preparing", 10, "正在解压并校验备份包...")

	extractRoot, manifest, err := backupExtractAndValidate(zipPath)
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(extractRoot)
	a.backupEmitImportProgress("preparing", 20, "备份包校验通过，开始加载数据...")

	componentEntries := backupDetectPresentManifestEntries(extractRoot, manifest)
	componentUniverse := make(map[string]struct{}, len(componentEntries))
	for id := range componentEntries {
		componentUniverse[id] = struct{}{}
	}
	failedComponentIDs := map[string]struct{}{}
	issues := make([]backupImportIssue, 0)
	recordIssue := func(componentID, componentName string, err error) {
		if err == nil {
			return
		}
		componentID = strings.TrimSpace(componentID)
		componentName = strings.TrimSpace(componentName)
		if componentID != "" {
			componentUniverse[componentID] = struct{}{}
			failedComponentIDs[componentID] = struct{}{}
			if componentName == "" {
				if entry, ok := componentEntries[componentID]; ok {
					componentName = backupResolveManifestComponentName(entry)
				}
			}
		}
		if componentName == "" {
			componentName = "未知模块"
		}
		issues = append(issues, backupImportIssue{
			ComponentID:   componentID,
			ComponentName: componentName,
			Error:         err.Error(),
		})
	}

	stats := &backupMergeStats{}

	if resetFirst {
		a.backupEmitImportProgress("preparing", 30, "正在初始化系统数据...")
		if _, err := a.backupInitializeLocked(false); err != nil {
			return nil, err
		}
		a.backupEmitImportProgress("preparing", 40, "初始化完成，继续加载备份内容...")
	}

	payloadRoot := filepath.Join(extractRoot, "payload")
	a.backupEmitImportProgress("importing", 50, "正在解析备份配置...")
	incomingCfg, hasIncomingCfg, err := backupLoadIncomingConfig(payloadRoot)
	if err != nil {
		recordIssue("system_config_main", "主配置文件", fmt.Errorf("解析配置失败: %w", err))
		incomingCfg = nil
		hasIncomingCfg = false
	}
	if resetFirst && !hasIncomingCfg {
		recordIssue("system_config_main", "主配置文件", fmt.Errorf("备份包缺少 payload/system/config.yaml，已保留默认配置继续加载其余模块"))
	}

	if hasIncomingCfg {
		a.backupEmitImportProgress("importing", 58, "正在应用系统配置...")
		if err := a.backupApplyIncomingConfig(incomingCfg, resetFirst); err != nil {
			recordIssue("system_config_main", "主配置文件", err)
		}
	}

	a.backupEmitImportProgress("importing", 66, "正在合并代理配置...")
	if err := a.backupMergeProxiesFile(payloadRoot, resetFirst, stats); err != nil {
		recordIssue("system_config_proxies", "代理配置文件", err)
	}

	if dbSrc := backupFindDatabaseFile(payloadRoot); dbSrc != "" {
		a.backupEmitImportProgress("importing", 76, "正在合并数据库数据...")
		if err := a.backupMergeDatabaseFromSource(dbSrc, resetFirst, stats); err != nil {
			recordIssue("database_sqlite_main", "SQLite 主数据库", err)
		}
	} else if _, ok := componentEntries["database_sqlite_main"]; ok {
		recordIssue("database_sqlite_main", "SQLite 主数据库", fmt.Errorf("备份包缺少数据库文件"))
	}

	a.backupEmitImportProgress("importing", 86, "正在同步文件数据...")
	a.backupImportFileTrees(payloadRoot, incomingCfg, resetFirst, stats, recordIssue)

	a.backupEmitImportProgress("importing", 94, "正在刷新运行时配置...")
	if err := a.backupReloadAfterMutation(); err != nil {
		return nil, err
	}

	totalComponents := len(componentUniverse)
	failedCount := len(failedComponentIDs)
	successCount := totalComponents - failedCount
	if successCount < 0 {
		successCount = 0
	}
	partial := failedCount > 0
	message := "加载完成"
	if partial {
		message = fmt.Sprintf("加载完成（部分成功）：成功 %d 个模块，异常 %d 个模块", successCount, failedCount)
	}
	a.backupEmitImportProgress("done", 100, message)

	failedComponents := make([]map[string]string, 0, len(issues))
	for _, item := range issues {
		failedComponents = append(failedComponents, map[string]string{
			"componentId":   item.ComponentID,
			"componentName": item.ComponentName,
			"error":         item.Error,
		})
	}

	return map[string]interface{}{
		"cancelled":        false,
		"zipPath":          zipPath,
		"resetFirst":       resetFirst,
		"imported":         stats.Imported,
		"skipped":          stats.Skipped,
		"conflicts":        stats.Conflicts,
		"partial":          partial,
		"componentTotal":   totalComponents,
		"componentSuccess": successCount,
		"componentFailed":  failedCount,
		"failedComponents": failedComponents,
		"message":          message,
	}, nil
}

func (a *App) backupStopRuntimeForMaintenance() {
	if a.browserMgr != nil {
		a.browserMgr.Mutex.Lock()
		for _, cmd := range a.browserMgr.BrowserProcesses {
			if cmd != nil && cmd.Process != nil {
				_ = a.stopProcessCmd(cmd)
			}
		}
		a.browserMgr.BrowserProcesses = make(map[string]*exec.Cmd)
		a.browserMgr.Mutex.Unlock()
	}

	if a.xrayMgr != nil {
		a.xrayMgr.StopAll()
	}
	a.clearProfileXrayBridges()
	if a.singboxMgr != nil {
		a.singboxMgr.StopAll()
	}
	if a.speedScheduler != nil {
		a.speedScheduler.Stop()
		a.speedScheduler = nil
	}
}

func (a *App) backupReloadAfterMutation() error {
	if err := a.ReloadConfig(); err != nil {
		return err
	}

	if a.browserMgr != nil {
		a.browserMgr.Config = a.config
		a.browserMgr.Mutex.Lock()
		a.browserMgr.Profiles = make(map[string]*browser.Profile)
		a.browserMgr.BrowserProcesses = make(map[string]*exec.Cmd)
		a.browserMgr.XrayBridges = make(map[string]*browser.XrayBridge)
		a.browserMgr.Mutex.Unlock()
	}
	if a.xrayMgr != nil {
		a.xrayMgr.Config = a.config
	}
	if a.clashMgr != nil {
		a.clashMgr.Config = a.config
	}
	if a.singboxMgr != nil {
		a.singboxMgr.Config = a.config
	}

	a.migrateToSQLite()
	if a.browserMgr != nil {
		a.browserMgr.InitData()
	}
	a.autoDetectCores()
	a.loadProxies()

	if a.launchCodeSvc != nil {
		_ = a.launchCodeSvc.LoadAll()
	}
	if a.browserMgr != nil {
		a.browserMgr.CodeProvider = a.launchCodeSvc
	}

	if a.browserMgr != nil && a.browserMgr.ProxyDAO != nil {
		a.speedScheduler = browser.NewProxySpeedScheduler(
			a.browserMgr.ProxyDAO,
			func(proxyID string) (bool, int64, string) {
				r := proxy.SpeedTest(context.Background(), proxyID, a.config.Browser.Proxies, a.bridgeManagers(), nil)
				return r.Ok, r.LatencyMs, r.Error
			},
			5*time.Minute,
			5,
		)
		a.speedScheduler.Start()
	}
	return nil
}

func (a *App) backupResolveDBPath(cfg *config.Config) string {
	if cfg == nil {
		return a.resolveAppPath("data/app.db")
	}
	path := strings.TrimSpace(cfg.Database.SQLite.Path)
	if path == "" {
		path = "data/app.db"
	}
	return a.resolveAppPath(path)
}

func (a *App) backupResolveUserDataRoot(cfg *config.Config) string {
	if cfg == nil {
		return a.resolveAppPath("data")
	}
	root := strings.TrimSpace(cfg.Browser.UserDataRoot)
	if root == "" {
		root = "data"
	}
	return a.resolveAppPath(root)
}

func (a *App) backupClearBusinessTables() error {
	if a.db == nil || a.db.GetConn() == nil {
		return fmt.Errorf("数据库未初始化")
	}
	tx, err := a.db.GetConn().Begin()
	if err != nil {
		return fmt.Errorf("开启事务失败: %w", err)
	}
	defer tx.Rollback()

	tables := []string{"launch_codes", "browser_profiles", "browser_proxies", "browser_cores", "browser_bookmarks", "browser_groups"}
	for _, table := range tables {
		if _, err := tx.Exec("DELETE FROM " + table); err != nil && !backupIsNoSuchTableError(err) {
			return fmt.Errorf("清空数据表失败(%s): %w", table, err)
		}
	}
	_, _ = tx.Exec(`DELETE FROM sqlite_sequence WHERE name IN ('browser_bookmarks')`)
	return tx.Commit()
}
