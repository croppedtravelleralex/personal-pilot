package backend

import (
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"personal-pilot/backend/internal/config"
)

func (a *App) backupCollectExternalCorePaths(cfg *config.Config) []string {
	if cfg == nil {
		return nil
	}
	defaultChromeRoot := a.resolveAppPath("chrome")
	seen := map[string]struct{}{}
	result := make([]string, 0)
	for _, core := range cfg.Browser.Cores {
		p := strings.TrimSpace(core.CorePath)
		if p == "" {
			continue
		}
		abs := a.resolveAppPath(p)
		if backupPathWithin(abs, defaultChromeRoot) {
			continue
		}
		norm := backupNormalizePath(abs)
		if _, ok := seen[norm]; ok {
			continue
		}
		seen[norm] = struct{}{}
		result = append(result, abs)
	}
	sort.Strings(result)
	return result
}

func backupSyncDir(src, dst string, overwrite bool, stats *backupMergeStats, shouldSkip func(rel string) bool) error {
	if !backupPathExists(src) {
		return nil
	}
	if err := os.MkdirAll(dst, 0755); err != nil {
		return err
	}

	return filepath.WalkDir(src, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.Type()&os.ModeSymlink != 0 {
			stats.Skipped++
			if d.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		if rel == "." {
			return nil
		}
		rel = filepath.ToSlash(rel)

		if shouldSkip != nil && shouldSkip(rel) {
			if d.IsDir() {
				return filepath.SkipDir
			}
			stats.Skipped++
			return nil
		}

		target := filepath.Join(dst, filepath.FromSlash(rel))
		if d.IsDir() {
			return os.MkdirAll(target, 0755)
		}
		if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
			return err
		}

		if overwrite {
			if err := backupCopyFile(path, target); err != nil {
				return err
			}
			stats.Imported++
			return nil
		}

		if _, err := os.Stat(target); os.IsNotExist(err) {
			if err := backupCopyFile(path, target); err != nil {
				return err
			}
			stats.Imported++
			return nil
		} else if err != nil {
			return err
		}

		same, err := backupFilesSame(path, target)
		if err != nil {
			return err
		}
		if same {
			stats.Skipped++
		} else {
			stats.Conflicts++
		}
		return nil
	})
}

func backupCopyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()
	info, err := in.Stat()
	if err != nil {
		return err
	}
	tmpPath := dst + ".tmp"
	out, err := os.OpenFile(tmpPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, info.Mode())
	if err != nil {
		return err
	}
	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		_ = os.Remove(tmpPath)
		return err
	}
	if err := out.Close(); err != nil {
		_ = os.Remove(tmpPath)
		return err
	}
	if err := os.Rename(tmpPath, dst); err != nil {
		_ = os.Remove(tmpPath)
		return err
	}
	return nil
}

func backupFilesSame(a, b string) (bool, error) {
	ainfo, err := os.Stat(a)
	if err != nil {
		return false, err
	}
	binfo, err := os.Stat(b)
	if err != nil {
		return false, err
	}
	if ainfo.Size() != binfo.Size() {
		return false, nil
	}
	ah, err := backupSHA256File(a)
	if err != nil {
		return false, err
	}
	bh, err := backupSHA256File(b)
	if err != nil {
		return false, err
	}
	return ah == bh, nil
}

func backupSHA256File(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer f.Close()
	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return "", err
	}
	return hex.EncodeToString(h.Sum(nil)), nil
}

func backupShouldSkipAppDBFile(rel string) bool {
	r := strings.TrimSpace(filepath.ToSlash(rel))
	return r == "app.db" || r == "app.db-wal" || r == "app.db-shm"
}

func backupRemoveContentsExcept(dir string, keep map[string]struct{}) error {
	dir = strings.TrimSpace(dir)
	if dir == "" {
		return nil
	}
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		p := filepath.Join(dir, entry.Name())
		if backupPathInSet(p, keep) {
			continue
		}
		if err := os.RemoveAll(p); err != nil {
			return err
		}
	}
	return nil
}

func backupPathInSet(path string, set map[string]struct{}) bool {
	if len(set) == 0 {
		return false
	}
	_, ok := set[backupNormalizePath(path)]
	return ok
}

func backupNormalizePath(path string) string {
	return strings.ToLower(filepath.Clean(strings.TrimSpace(path)))
}

func backupEnsureZipSuffix(path string) string {
	if strings.EqualFold(filepath.Ext(path), ".zip") {
		return path
	}
	return path + ".zip"
}

func backupPathExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func backupSamePath(a, b string) bool {
	return backupNormalizePath(a) == backupNormalizePath(b)
}

func backupPathWithin(path, root string) bool {
	p := backupNormalizePath(path)
	r := backupNormalizePath(root)
	if p == r {
		return true
	}
	if !strings.HasSuffix(r, string(filepath.Separator)) {
		r += string(filepath.Separator)
	}
	return strings.HasPrefix(p, r)
}

func backupIsNoSuchTableError(err error) bool {
	if err == nil {
		return false
	}
	return strings.Contains(strings.ToLower(err.Error()), "no such table")
}

func backupUniqueNonEmpty(list []string) []string {
	seen := map[string]struct{}{}
	out := make([]string, 0, len(list))
	for _, item := range list {
		item = strings.TrimSpace(item)
		if item == "" {
			continue
		}
		key := backupNormalizePath(item)
		if _, ok := seen[key]; ok {
			continue
		}
		seen[key] = struct{}{}
		out = append(out, item)
	}
	return out
}

func backupUnionStrings(a, b []string) []string {
	seen := map[string]struct{}{}
	out := make([]string, 0, len(a)+len(b))
	for _, item := range append(append([]string{}, a...), b...) {
		item = strings.TrimSpace(item)
		if item == "" {
			continue
		}
		key := strings.ToLower(item)
		if _, ok := seen[key]; ok {
			continue
		}
		seen[key] = struct{}{}
		out = append(out, item)
	}
	return out
}

func backupMergeBookmarks(a, b []config.BrowserBookmark) []config.BrowserBookmark {
	seen := map[string]struct{}{}
	out := make([]config.BrowserBookmark, 0, len(a)+len(b))
	appendOne := func(item config.BrowserBookmark) {
		urlKey := strings.ToLower(strings.TrimSpace(item.URL))
		if urlKey == "" {
			return
		}
		if _, ok := seen[urlKey]; ok {
			return
		}
		seen[urlKey] = struct{}{}
		out = append(out, item)
	}
	for _, item := range a {
		appendOne(item)
	}
	for _, item := range b {
		appendOne(item)
	}
	return out
}

func backupMergeCores(a, b []config.BrowserCore) []config.BrowserCore {
	seenID := map[string]struct{}{}
	seenPath := map[string]struct{}{}
	out := make([]config.BrowserCore, 0, len(a)+len(b))
	appendOne := func(item config.BrowserCore) {
		idKey := strings.ToLower(strings.TrimSpace(item.CoreId))
		pathKey := strings.ToLower(strings.TrimSpace(item.CorePath))
		if idKey == "" && pathKey == "" {
			return
		}
		if idKey != "" {
			if _, ok := seenID[idKey]; ok {
				return
			}
		}
		if pathKey != "" {
			if _, ok := seenPath[pathKey]; ok {
				return
			}
		}
		if idKey != "" {
			seenID[idKey] = struct{}{}
		}
		if pathKey != "" {
			seenPath[pathKey] = struct{}{}
		}
		out = append(out, item)
	}
	for _, item := range a {
		appendOne(item)
	}
	for _, item := range b {
		appendOne(item)
	}
	return out
}

func backupMergeProxies(a, b []config.BrowserProxy) []config.BrowserProxy {
	seenID := map[string]struct{}{}
	seenCfg := map[string]struct{}{}
	out := make([]config.BrowserProxy, 0, len(a)+len(b))
	appendOne := func(item config.BrowserProxy) {
		idKey := strings.ToLower(strings.TrimSpace(item.ProxyId))
		cfgKey := strings.ToLower(strings.TrimSpace(item.ProxyConfig))
		if idKey == "" && cfgKey == "" {
			return
		}
		if idKey != "" {
			if _, ok := seenID[idKey]; ok {
				return
			}
		}
		if cfgKey != "" {
			if _, ok := seenCfg[cfgKey]; ok {
				return
			}
		}
		if idKey != "" {
			seenID[idKey] = struct{}{}
		}
		if cfgKey != "" {
			seenCfg[cfgKey] = struct{}{}
		}
		out = append(out, item)
	}
	for _, item := range a {
		appendOne(item)
	}
	for _, item := range b {
		appendOne(item)
	}
	return out
}

func backupMergeProfiles(a, b []config.BrowserProfileConfig) []config.BrowserProfileConfig {
	seenID := map[string]struct{}{}
	seenDir := map[string]struct{}{}
	out := make([]config.BrowserProfileConfig, 0, len(a)+len(b))
	appendOne := func(item config.BrowserProfileConfig) {
		idKey := strings.ToLower(strings.TrimSpace(item.ProfileId))
		dirKey := strings.ToLower(strings.TrimSpace(item.UserDataDir))
		if idKey == "" && dirKey == "" {
			return
		}
		if idKey != "" {
			if _, ok := seenID[idKey]; ok {
				return
			}
		}
		if dirKey != "" {
			if _, ok := seenDir[dirKey]; ok {
				return
			}
		}
		if idKey != "" {
			seenID[idKey] = struct{}{}
		}
		if dirKey != "" {
			seenDir[dirKey] = struct{}{}
		}
		out = append(out, item)
	}
	for _, item := range a {
		appendOne(item)
	}
	for _, item := range b {
		appendOne(item)
	}
	return out
}

func backupSrcTableExists(tx *sql.Tx, table string) (bool, error) {
	var cnt int
	err := tx.QueryRow(`SELECT COUNT(1) FROM src.sqlite_master WHERE type='table' AND name=?`, table).Scan(&cnt)
	if err != nil {
		return false, err
	}
	return cnt > 0, nil
}

func backupCountRows(tx *sql.Tx, tableName string) (int, error) {
	var cnt int
	if !isValidTableName(tableName) {
		return 0, fmt.Errorf("invalid table name: %s", tableName)
	}
	row := tx.QueryRow("SELECT COUNT(1) FROM " + tableName)
	if err := row.Scan(&cnt); err != nil {
		return 0, err
	}
	return cnt, nil
}
