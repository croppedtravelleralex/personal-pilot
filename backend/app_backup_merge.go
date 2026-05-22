package backend

import (
	"archive/zip"
	"encoding/json"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"personal-pilot/backend/internal/backup"
	"personal-pilot/backend/internal/config"
)

type backupMergeStats struct {
	Imported  int
	Skipped   int
	Conflicts int
}

func backupWritePackageZip(zipPath string, scope backup.Scope, manifest backup.Manifest, emitProgress func(phase string, progress int, message string, meta *backupProgressMeta)) (int, int, int, error) {
	emit := func(phase string, progress int, message string, meta *backupProgressMeta) {
		if emitProgress != nil {
			emitProgress(phase, progress, message, meta)
		}
	}
	if err := os.MkdirAll(filepath.Dir(zipPath), 0755); err != nil {
		return 0, 0, 0, fmt.Errorf("创建导出目录失败: %w", err)
	}
	emit("writing", 18, "正在创建导出文件...", nil)

	tmpPath := zipPath + ".tmp"
	f, err := os.Create(tmpPath)
	if err != nil {
		return 0, 0, 0, fmt.Errorf("创建导出文件失败: %w", err)
	}
	w := zip.NewWriter(f)

	includedEntries := 0
	skippedEntries := 0
	fileCount := 0

	writeErr := func() error {
		emit("writing", 20, "正在写入备份清单...", nil)
		manifestData, err := json.MarshalIndent(manifest, "", "  ")
		if err != nil {
			return err
		}
		mw, err := w.Create("manifest.json")
		if err != nil {
			return err
		}
		if _, err := mw.Write(manifestData); err != nil {
			return err
		}
		fileCount++

		totalEntries := len(scope.Entries)
		if totalEntries == 0 {
			emit("writing", 90, "没有可导出的目录条目", nil)
		}
		for i, entry := range scope.Entries {
			meta := &backupProgressMeta{
				ComponentID:   entry.ID,
				ComponentName: backupResolveEntryComponentName(entry),
				EntryIndex:    i + 1,
				EntryTotal:    totalEntries,
			}
			startProgress := 20 + int(float64(i)/float64(totalEntries)*70)
			emit("writing", startProgress, fmt.Sprintf("开始处理组件 %d/%d：%s", i+1, totalEntries, meta.ComponentName), meta)

			info, err := os.Stat(entry.SourcePath)
			if err != nil {
				if os.IsNotExist(err) && !entry.Required {
					skippedEntries++
					progress := 20 + int(float64(i+1)/float64(totalEntries)*70)
					emit("writing", progress, fmt.Sprintf("组件跳过：%s（源路径不存在）", meta.ComponentName), meta)
					continue
				}
				return fmt.Errorf("读取导出源失败(%s): %w", entry.ID, err)
			}
			entryAddedFiles := 0
			if info.IsDir() {
				n, err := backupZipAddDir(w, entry.SourcePath, entry.ArchivePath, zipPath)
				if err != nil {
					return fmt.Errorf("写入目录失败(%s): %w", entry.ID, err)
				}
				fileCount += n
				entryAddedFiles = n
			} else {
				if backupSamePath(entry.SourcePath, zipPath) {
					skippedEntries++
					progress := 20 + int(float64(i+1)/float64(totalEntries)*70)
					emit("writing", progress, fmt.Sprintf("组件跳过：%s（导出文件本身）", meta.ComponentName), meta)
					continue
				}
				if err := backupZipAddFile(w, entry.SourcePath, strings.TrimSuffix(entry.ArchivePath, "/")); err != nil {
					return fmt.Errorf("写入文件失败(%s): %w", entry.ID, err)
				}
				fileCount++
				entryAddedFiles = 1
			}
			includedEntries++
			progress := 20 + int(float64(i+1)/float64(totalEntries)*70)
			emit("writing", progress, fmt.Sprintf("组件完成：%s（新增 %d 个文件）", meta.ComponentName, entryAddedFiles), meta)
		}
		return nil
	}()

	closeErr := w.Close()
	fileCloseErr := f.Close()
	if writeErr != nil {
		emit("error", 100, writeErr.Error(), nil)
		_ = os.Remove(tmpPath)
		return 0, 0, 0, writeErr
	}
	if closeErr != nil {
		emit("error", 100, closeErr.Error(), nil)
		_ = os.Remove(tmpPath)
		return 0, 0, 0, closeErr
	}
	if fileCloseErr != nil {
		emit("error", 100, fileCloseErr.Error(), nil)
		_ = os.Remove(tmpPath)
		return 0, 0, 0, fileCloseErr
	}
	if err := os.Rename(tmpPath, zipPath); err != nil {
		emit("error", 100, err.Error(), nil)
		_ = os.Remove(tmpPath)
		return 0, 0, 0, fmt.Errorf("写入导出文件失败: %w", err)
	}
	emit("done", 100, "导出完成", nil)
	return includedEntries, skippedEntries, fileCount, nil
}

func backupResolveEntryComponentName(entry backup.ScopeEntry) string {
	if desc := strings.TrimSpace(entry.Description); desc != "" {
		return desc
	}
	if entry.ID != "" {
		return entry.ID
	}
	switch entry.Category {
	case backup.CategorySystemConfig:
		return "系统配置"
	case backup.CategoryAppData:
		return "应用数据"
	case backup.CategoryBrowserData:
		return "浏览器数据"
	case backup.CategoryCoreData:
		return "内核数据"
	case backup.CategoryLogs:
		return "日志数据"
	default:
		return "未知组件"
	}
}

func backupZipAddDir(w *zip.Writer, srcDir, archiveBase, outputZipPath string) (int, error) {
	base := strings.TrimSuffix(filepath.ToSlash(strings.TrimSpace(archiveBase)), "/")
	if base == "" {
		return 0, fmt.Errorf("archive base 不能为空")
	}
	fileCount := 0
	err := filepath.WalkDir(srcDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if backupSamePath(path, outputZipPath) {
			return nil
		}
		if d.Type()&os.ModeSymlink != 0 {
			return nil
		}
		rel, err := filepath.Rel(srcDir, path)
		if err != nil {
			return err
		}
		if rel == "." {
			return nil
		}
		rel = filepath.ToSlash(rel)
		targetName := base + "/" + rel
		if d.IsDir() {
			_, err := w.Create(strings.TrimSuffix(targetName, "/") + "/")
			return err
		}
		if err := backupZipAddFile(w, path, targetName); err != nil {
			return err
		}
		fileCount++
		return nil
	})
	return fileCount, err
}

func backupZipAddFile(w *zip.Writer, srcFile, archivePath string) error {
	info, err := os.Stat(srcFile)
	if err != nil {
		return err
	}
	if info.IsDir() {
		return fmt.Errorf("不支持将目录按文件写入: %s", srcFile)
	}
	header, err := zip.FileInfoHeader(info)
	if err != nil {
		return err
	}
	header.Name = strings.TrimPrefix(filepath.ToSlash(strings.TrimSpace(archivePath)), "/")
	header.Method = zip.Deflate
	if header.Name == "" {
		return fmt.Errorf("archivePath 不能为空")
	}
	writer, err := w.CreateHeader(header)
	if err != nil {
		return err
	}
	in, err := os.Open(srcFile)
	if err != nil {
		return err
	}
	defer in.Close()
	_, err = io.Copy(writer, in)
	return err
}

func backupExtractAndValidate(zipPath string) (string, backup.Manifest, error) {
	tmpDir, err := os.MkdirTemp("", "personal-pilot-import-*")
	if err != nil {
		return "", backup.Manifest{}, err
	}
	if err := unzipTo(zipPath, tmpDir); err != nil {
		_ = os.RemoveAll(tmpDir)
		return "", backup.Manifest{}, fmt.Errorf("解压备份包失败: %w", err)
	}

	manifestPath := filepath.Join(tmpDir, "manifest.json")
	data, err := os.ReadFile(manifestPath)
	if err != nil {
		_ = os.RemoveAll(tmpDir)
		return "", backup.Manifest{}, fmt.Errorf("备份包缺少 manifest.json")
	}
	var manifest backup.Manifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		_ = os.RemoveAll(tmpDir)
		return "", backup.Manifest{}, fmt.Errorf("manifest.json 解析失败: %w", err)
	}
	if manifest.Format != backup.PackageFormat {
		_ = os.RemoveAll(tmpDir)
		return "", backup.Manifest{}, fmt.Errorf("不支持的备份格式: %s", manifest.Format)
	}
	if manifest.ManifestVersion != backup.ManifestVersion {
		_ = os.RemoveAll(tmpDir)
		return "", backup.Manifest{}, fmt.Errorf("不支持的 manifest 版本: %d", manifest.ManifestVersion)
	}
	if _, err := os.Stat(filepath.Join(tmpDir, "payload")); err != nil {
		_ = os.RemoveAll(tmpDir)
		return "", backup.Manifest{}, fmt.Errorf("备份包缺少 payload 目录")
	}
	return tmpDir, manifest, nil
}

func backupLoadIncomingConfig(payloadRoot string) (*config.Config, bool, error) {
	cfgPath := filepath.Join(payloadRoot, "system", "config.yaml")
	if _, err := os.Stat(cfgPath); err != nil {
		if os.IsNotExist(err) {
			return nil, false, nil
		}
		return nil, false, err
	}
	cfg, err := config.Load(cfgPath)
	if err != nil {
		return nil, false, err
	}
	return cfg, true, nil
}

func backupDetectPresentManifestEntries(extractRoot string, manifest backup.Manifest) map[string]backup.ManifestEntry {
	result := make(map[string]backup.ManifestEntry, len(manifest.Entries))
	for _, entry := range manifest.Entries {
		id := strings.TrimSpace(entry.ID)
		if id == "" {
			continue
		}
		archivePath := strings.TrimSpace(strings.TrimSuffix(entry.ArchivePath, "/"))
		if archivePath == "" {
			continue
		}
		absPath := filepath.Join(extractRoot, filepath.FromSlash(archivePath))
		if _, err := os.Stat(absPath); err == nil {
			result[id] = entry
		}
	}
	return result
}

func backupResolveManifestComponentName(entry backup.ManifestEntry) string {
	if desc := strings.TrimSpace(entry.Description); desc != "" {
		return desc
	}
	if id := strings.TrimSpace(entry.ID); id != "" {
		return id
	}
	return "未知模块"
}

func (a *App) backupApplyIncomingConfig(incoming *config.Config, resetFirst bool) error {
	if incoming == nil {
		return nil
	}
	current := a.config
	if current == nil {
		current = config.DefaultConfig()
	}

	var target *config.Config
	if resetFirst {
		cloned := *incoming
		target = &cloned
	} else {
		target = backupMergeConfig(current, incoming)
	}
	target.Database = current.Database
	target.App.MaxProfileLimit = current.App.MaxProfileLimit
	target.App.UsedCDKeys = append([]string{}, current.App.UsedCDKeys...)

	if err := target.Save(a.resolveAppPath("config.yaml")); err != nil {
		return fmt.Errorf("保存导入配置失败: %w", err)
	}
	a.config = target
	a.applyRuntimeConfig(target.Runtime)
	return nil
}

func backupMergeConfig(current, incoming *config.Config) *config.Config {
	if current == nil {
		cp := *incoming
		return &cp
	}
	if incoming == nil {
		cp := *current
		return &cp
	}
	merged := *current
	if strings.TrimSpace(merged.App.Name) == "" {
		merged.App.Name = incoming.App.Name
	}
	merged.Browser.DefaultBookmarks = backupMergeBookmarks(merged.Browser.DefaultBookmarks, incoming.Browser.DefaultBookmarks)
	merged.Browser.Cores = backupMergeCores(merged.Browser.Cores, incoming.Browser.Cores)
	merged.Browser.Proxies = backupMergeProxies(merged.Browser.Proxies, incoming.Browser.Proxies)
	merged.Browser.Profiles = backupMergeProfiles(merged.Browser.Profiles, incoming.Browser.Profiles)
	return &merged
}

func (a *App) backupMergeProxiesFile(payloadRoot string, resetFirst bool, stats *backupMergeStats) error {
	srcPath := filepath.Join(payloadRoot, "system", "proxies.yaml")
	dstPath := a.resolveAppPath("proxies.yaml")

	if _, err := os.Stat(srcPath); err != nil {
		if os.IsNotExist(err) {
			if resetFirst {
				_ = os.Remove(dstPath)
			}
			return nil
		}
		return err
	}

	if resetFirst {
		return backupCopyFile(srcPath, dstPath)
	}

	incoming, err := config.LoadProxies(srcPath)
	if err != nil {
		return err
	}
	current, err := config.LoadProxies(dstPath)
	if err != nil {
		return err
	}

	merged := append([]config.BrowserProxy{}, current...)
	existingID := make(map[string]struct{}, len(current))
	existingCfg := make(map[string]struct{}, len(current))
	for _, p := range current {
		existingID[strings.ToLower(strings.TrimSpace(p.ProxyId))] = struct{}{}
		existingCfg[strings.ToLower(strings.TrimSpace(p.ProxyConfig))] = struct{}{}
	}
	for _, p := range incoming {
		idKey := strings.ToLower(strings.TrimSpace(p.ProxyId))
		cfgKey := strings.ToLower(strings.TrimSpace(p.ProxyConfig))
		if _, ok := existingID[idKey]; ok {
			stats.Skipped++
			continue
		}
		if cfgKey != "" {
			if _, ok := existingCfg[cfgKey]; ok {
				stats.Skipped++
				continue
			}
		}
		merged = append(merged, p)
		existingID[idKey] = struct{}{}
		if cfgKey != "" {
			existingCfg[cfgKey] = struct{}{}
		}
		stats.Imported++
	}

	return config.SaveProxies(dstPath, merged)
}

func backupFindDatabaseFile(payloadRoot string) string {
	candidates := []string{
		filepath.Join(payloadRoot, "app", "database", "app.db"),
		filepath.Join(payloadRoot, "app", "data", "app.db"),
	}
	for _, p := range candidates {
		if st, err := os.Stat(p); err == nil && !st.IsDir() {
			return p
		}
	}
	return ""
}

func (a *App) backupMergeDatabaseFromSource(srcDBPath string, resetFirst bool, stats *backupMergeStats) error {
	if a.db == nil || a.db.GetConn() == nil {
		return fmt.Errorf("数据库未初始化")
	}
	tx, err := a.db.GetConn().Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()

	if _, err := tx.Exec(`ATTACH DATABASE ? AS src`, srcDBPath); err != nil {
		return fmt.Errorf("挂载备份数据库失败: %w", err)
	}
	defer tx.Exec(`DETACH DATABASE src`)

	mergeTables := []struct {
		name       string
		insertAll  string
		insertSafe string
	}{
		{
			name: "browser_groups",
			insertAll: `INSERT INTO browser_groups (group_id, group_name, parent_id, sort_order, created_at, updated_at)
SELECT group_id, group_name, parent_id, sort_order, created_at, updated_at FROM src.browser_groups`,
			insertSafe: `INSERT INTO browser_groups (group_id, group_name, parent_id, sort_order, created_at, updated_at)
SELECT s.group_id, s.group_name, s.parent_id, s.sort_order, s.created_at, s.updated_at
FROM src.browser_groups s
WHERE NOT EXISTS (
  SELECT 1 FROM browser_groups t
  WHERE t.group_id = s.group_id OR (t.parent_id = s.parent_id AND lower(t.group_name) = lower(s.group_name))
)`,
		},
		{
			name: "browser_cores",
			insertAll: `INSERT INTO browser_cores (core_id, core_name, core_path, is_default, sort_order, created_at)
SELECT core_id, core_name, core_path, is_default, sort_order, created_at FROM src.browser_cores`,
			insertSafe: `INSERT INTO browser_cores (core_id, core_name, core_path, is_default, sort_order, created_at)
SELECT s.core_id, s.core_name, s.core_path, s.is_default, s.sort_order, s.created_at
FROM src.browser_cores s
WHERE NOT EXISTS (
  SELECT 1 FROM browser_cores t
  WHERE t.core_id = s.core_id OR lower(t.core_path) = lower(s.core_path)
)`,
		},
		{
			name: "browser_proxies",
			insertAll: `INSERT INTO browser_proxies (proxy_id, proxy_name, proxy_config, dns_servers, group_name, source_id, source_url, source_name_prefix, source_auto_refresh, source_refresh_interval_m, source_last_refresh_at, last_latency_ms, last_test_ok, last_tested_at, last_ip_health_json, sort_order, created_at)
SELECT proxy_id, proxy_name, proxy_config, dns_servers, COALESCE(group_name,''), COALESCE(source_id,''), COALESCE(source_url,''), COALESCE(source_name_prefix,''), COALESCE(source_auto_refresh,0), COALESCE(source_refresh_interval_m,0), COALESCE(source_last_refresh_at,''), COALESCE(last_latency_ms,-1), COALESCE(last_test_ok,0), COALESCE(last_tested_at,''), COALESCE(last_ip_health_json,''), sort_order, created_at
FROM src.browser_proxies`,
			insertSafe: `INSERT INTO browser_proxies (proxy_id, proxy_name, proxy_config, dns_servers, group_name, source_id, source_url, source_name_prefix, source_auto_refresh, source_refresh_interval_m, source_last_refresh_at, last_latency_ms, last_test_ok, last_tested_at, last_ip_health_json, sort_order, created_at)
SELECT s.proxy_id, s.proxy_name, s.proxy_config, s.dns_servers, COALESCE(s.group_name,''), COALESCE(s.source_id,''), COALESCE(s.source_url,''), COALESCE(s.source_name_prefix,''), COALESCE(s.source_auto_refresh,0), COALESCE(s.source_refresh_interval_m,0), COALESCE(s.source_last_refresh_at,''), COALESCE(s.last_latency_ms,-1), COALESCE(s.last_test_ok,0), COALESCE(s.last_tested_at,''), COALESCE(s.last_ip_health_json,''), s.sort_order, s.created_at
FROM src.browser_proxies s
WHERE NOT EXISTS (
  SELECT 1 FROM browser_proxies t
  WHERE t.proxy_id = s.proxy_id OR lower(t.proxy_config) = lower(s.proxy_config)
)`,
		},
		{
			name: "browser_profiles",
			insertAll: `INSERT INTO browser_profiles (profile_id, profile_name, user_data_dir, core_id, fingerprint_args, proxy_id, proxy_config, launch_args, tags, keywords, group_id, created_at, updated_at)
SELECT profile_id, profile_name, user_data_dir, core_id, fingerprint_args, proxy_id, proxy_config, launch_args, tags, keywords, COALESCE(group_id,''), created_at, updated_at
FROM src.browser_profiles`,
			insertSafe: `INSERT INTO browser_profiles (profile_id, profile_name, user_data_dir, core_id, fingerprint_args, proxy_id, proxy_config, launch_args, tags, keywords, group_id, created_at, updated_at)
SELECT s.profile_id, s.profile_name, s.user_data_dir, s.core_id, s.fingerprint_args, s.proxy_id, s.proxy_config, s.launch_args, s.tags, s.keywords, COALESCE(s.group_id,''), s.created_at, s.updated_at
FROM src.browser_profiles s
WHERE NOT EXISTS (
  SELECT 1 FROM browser_profiles t
  WHERE t.profile_id = s.profile_id OR lower(t.user_data_dir) = lower(s.user_data_dir)
)`,
		},
		{
			name: "browser_bookmarks",
			insertAll: `INSERT INTO browser_bookmarks (name, url, sort_order)
SELECT name, url, sort_order FROM src.browser_bookmarks`,
			insertSafe: `INSERT INTO browser_bookmarks (name, url, sort_order)
SELECT s.name, s.url, s.sort_order
FROM src.browser_bookmarks s
WHERE NOT EXISTS (
  SELECT 1 FROM browser_bookmarks t WHERE lower(t.url) = lower(s.url)
)`,
		},
		{
			name: "launch_codes",
			insertAll: `INSERT INTO launch_codes (profile_id, code, created_at, updated_at)
SELECT profile_id, code, created_at, updated_at FROM src.launch_codes`,
			insertSafe: `INSERT INTO launch_codes (profile_id, code, created_at, updated_at)
SELECT s.profile_id, s.code, s.created_at, s.updated_at
FROM src.launch_codes s
WHERE NOT EXISTS (
  SELECT 1 FROM launch_codes t
  WHERE t.profile_id = s.profile_id OR t.code = s.code
)`,
		},
	}

	for _, item := range mergeTables {
		exists, err := backupSrcTableExists(tx, item.name)
		if err != nil {
			return err
		}
		if !exists {
			continue
		}

		total, err := backupCountRows(tx, "src."+item.name)
		if err != nil {
			return err
		}
		if total == 0 {
			continue
		}

		sqlText := item.insertAll
		if !resetFirst {
			sqlText = item.insertSafe
		}
		res, err := tx.Exec(sqlText)
		if err != nil {
			return fmt.Errorf("导入数据表失败(%s): %w", item.name, err)
		}
		affected, _ := res.RowsAffected()
		inserted := int(affected)
		if inserted < 0 {
			inserted = total
		}
		stats.Imported += inserted
		if !resetFirst && total > inserted {
			stats.Skipped += total - inserted
		}
	}

	return tx.Commit()
}

func (a *App) backupImportFileTrees(payloadRoot string, incomingCfg *config.Config, resetFirst bool, stats *backupMergeStats, onIssue func(componentID, componentName string, err error)) {
	report := func(componentID, componentName string, err error) {
		if onIssue != nil && err != nil {
			onIssue(componentID, componentName, err)
		}
	}

	appDataSrc := filepath.Join(payloadRoot, "app", "data")
	appDataDst := a.resolveAppPath("data")
	dbPath := a.backupResolveDBPath(a.config)
	keepDB := map[string]struct{}{
		backupNormalizePath(dbPath):          {},
		backupNormalizePath(dbPath + "-wal"): {},
		backupNormalizePath(dbPath + "-shm"): {},
	}

	if backupPathExists(appDataSrc) {
		if resetFirst {
			if err := backupRemoveContentsExcept(appDataDst, keepDB); err != nil {
				report("app_data_root", "应用数据目录（含数据库、快照及默认浏览器数据）", err)
			} else if err := backupSyncDir(appDataSrc, appDataDst, true, stats, backupShouldSkipAppDBFile); err != nil {
				report("app_data_root", "应用数据目录（含数据库、快照及默认浏览器数据）", err)
			}
		} else {
			if err := backupSyncDir(appDataSrc, appDataDst, false, stats, backupShouldSkipAppDBFile); err != nil {
				report("app_data_root", "应用数据目录（含数据库、快照及默认浏览器数据）", err)
			}
		}
	}

	userDataSrc := filepath.Join(payloadRoot, "browser", "user-data")
	userDataDst := a.backupResolveUserDataRoot(a.config)
	if backupPathExists(userDataSrc) {
		if resetFirst {
			_ = os.RemoveAll(userDataDst)
			if err := os.MkdirAll(userDataDst, 0755); err != nil {
				report("browser_user_data_root", "浏览器用户数据根目录（若与 data 重合则自动去重）", err)
			} else if err := backupSyncDir(userDataSrc, userDataDst, true, stats, nil); err != nil {
				report("browser_user_data_root", "浏览器用户数据根目录（若与 data 重合则自动去重）", err)
			}
		} else {
			if err := backupSyncDir(userDataSrc, userDataDst, false, stats, nil); err != nil {
				report("browser_user_data_root", "浏览器用户数据根目录（若与 data 重合则自动去重）", err)
			}
		}
	}

	chromeSrc := filepath.Join(payloadRoot, "browser", "cores", "chrome")
	chromeDst := a.resolveAppPath("chrome")
	if backupPathExists(chromeSrc) {
		if resetFirst {
			_ = os.RemoveAll(chromeDst)
			if err := os.MkdirAll(chromeDst, 0755); err != nil {
				report("browser_core_root", "默认内核目录", err)
			} else if err := backupSyncDir(chromeSrc, chromeDst, true, stats, nil); err != nil {
				report("browser_core_root", "默认内核目录", err)
			}
		} else {
			if err := backupSyncDir(chromeSrc, chromeDst, false, stats, nil); err != nil {
				report("browser_core_root", "默认内核目录", err)
			}
		}
	}

	externalSrcRoot := filepath.Join(payloadRoot, "browser", "cores", "external")
	if backupPathExists(externalSrcRoot) {
		sourceExternal := make([]string, 0)
		entries, err := os.ReadDir(externalSrcRoot)
		if err != nil {
			report("browser_core_external", "额外内核目录（来自配置 cores）", err)
			return
		}
		for _, entry := range entries {
			if !entry.IsDir() {
				continue
			}
			sourceExternal = append(sourceExternal, entry.Name())
		}
		sort.Strings(sourceExternal)

		if incomingCfg == nil {
			for _, folder := range sourceExternal {
				componentID := "browser_core_external_" + folder
				report(componentID, "额外内核目录（来自配置 cores）", fmt.Errorf("缺少可用配置，无法映射目标路径"))
			}
			return
		}

		targetExternal := a.backupCollectExternalCorePaths(incomingCfg)
		for i, folder := range sourceExternal {
			src := filepath.Join(externalSrcRoot, folder)
			componentID := "browser_core_external_" + folder
			if i >= len(targetExternal) {
				stats.Skipped++
				report(componentID, "额外内核目录（来自配置 cores）", fmt.Errorf("目标配置缺失，无法导入该外部内核目录"))
				continue
			}
			dst := targetExternal[i]
			if resetFirst {
				_ = os.RemoveAll(dst)
				if err := os.MkdirAll(dst, 0755); err != nil {
					report(componentID, "额外内核目录（来自配置 cores）", err)
					continue
				}
				if err := backupSyncDir(src, dst, true, stats, nil); err != nil {
					report(componentID, "额外内核目录（来自配置 cores）", err)
					continue
				}
			} else {
				if err := backupSyncDir(src, dst, false, stats, nil); err != nil {
					report(componentID, "额外内核目录（来自配置 cores）", err)
					continue
				}
			}
		}
	}
}
