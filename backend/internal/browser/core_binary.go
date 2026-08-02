package browser

import (
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/config"
	goruntime "runtime"
	"strings"
)

// CoreExecutableCandidates 返回当前平台可接受的浏览器可执行文件候选名。
func CoreExecutableCandidates() []string {
	switch goruntime.GOOS {
	case "windows":
		return []string{"chrome.exe"}
	case "linux":
		return []string{"chrome", "chrome-bin", "chrome.exe"}
	case "darwin":
		return []string{
			"Google Chrome.app/Contents/MacOS/Google Chrome",
			"Chromium.app/Contents/MacOS/Chromium",
			"chrome",
		}
	default:
		return []string{"chrome"}
	}
}

// CoreExecutableCandidatesForKind returns executable candidates for a given core kind.
func CoreExecutableCandidatesForKind(kind string) []string {
	switch normalizeCoreKind(kind) {
	case config.CoreKindLightpanda:
		return LightpandaExecutableCandidates()
	case config.CoreKindCamoufox:
		return CamoufoxExecutableCandidates()
	default:
		return CoreExecutableCandidates()
	}
}

// FindCoreExecutable 在指定目录查找可执行文件，返回绝对路径和命中的候选名。
func FindCoreExecutable(baseDir string) (string, string, bool) {
	baseDir = strings.TrimSpace(baseDir)
	if baseDir == "" {
		return "", "", false
	}
	for _, candidate := range CoreExecutableCandidates() {
		p := filepath.Join(baseDir, filepath.FromSlash(candidate))
		if _, err := os.Stat(p); err == nil {
			return p, candidate, true
		}
	}
	return "", "", false
}

// FindCoreExecutableForKind searches for an executable of the given kind.
func FindCoreExecutableForKind(baseDir, kind string) (string, string, bool) {
	baseDir = strings.TrimSpace(baseDir)
	if baseDir == "" {
		return "", "", false
	}
	for _, candidate := range CoreExecutableCandidatesForKind(kind) {
		p := filepath.Join(baseDir, filepath.FromSlash(candidate))
		if _, err := os.Stat(p); err == nil {
			return p, candidate, true
		}
	}
	return "", "", false
}
