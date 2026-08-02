package browser

import (
	"fmt"
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/config"
	goruntime "runtime"
	"strings"
)

func CamoufoxExecutableCandidates() []string {
	switch goruntime.GOOS {
	case "windows":
		return []string{"camoufox.exe", "Camoufox.exe", "firefox.exe"}
	case "linux":
		return []string{"camoufox", "firefox"}
	case "darwin":
		return []string{"Camoufox.app/Contents/MacOS/camoufox", "Camoufox.app/Contents/MacOS/firefox", "camoufox"}
	default:
		return []string{"camoufox"}
	}
}

func FindCamoufoxExecutable(baseDir string) (string, string, bool) {
	baseDir = strings.TrimSpace(baseDir)
	if baseDir == "" {
		return "", "", false
	}
	for _, candidate := range CamoufoxExecutableCandidates() {
		p := filepath.Join(baseDir, filepath.FromSlash(candidate))
		if _, err := os.Stat(p); err == nil {
			return p, candidate, true
		}
	}
	return "", "", false
}

func (m *Manager) ResolveCamoufoxExecutable(core Core) (string, error) {
	corePath := strings.TrimSpace(core.CorePath)
	if corePath == "" {
		return "", fmt.Errorf("Camoufox 内核路径为空，请在\"内核管理\"中补充内核目录")
	}
	baseDir := m.ResolveRelativePath(corePath)
	exePath, _, ok := FindCamoufoxExecutable(baseDir)
	if !ok {
		return "", fmt.Errorf("Camoufox 内核目录无效：未找到可执行文件（候选：%s）", strings.Join(CamoufoxExecutableCandidates(), ", "))
	}
	return exePath, nil
}

func BuildCamoufoxLaunchArgs(debugPort int, profileRoot string, extraArgs []string) []string {
	args := []string{fmt.Sprintf("--remote-debugging-port=%d", debugPort)}
	if strings.TrimSpace(profileRoot) != "" {
		args = append(args, "--profile", profileRoot)
	}
	args = append(args, extraArgs...)
	return args
}

func IsCamoufoxKind(kind string) bool {
	return normalizeCoreKind(kind) == config.CoreKindCamoufox
}

func BuildCoreLaunchArgs(kind string, debugPort int, profileRoot string) []string {
	switch normalizeCoreKind(kind) {
	case config.CoreKindCamoufox:
		return BuildCamoufoxLaunchArgs(debugPort, profileRoot, nil)
	default:
		return []string{
			fmt.Sprintf("--user-data-dir=%s", profileRoot),
			fmt.Sprintf("--remote-debugging-port=%d", debugPort),
			"--disable-session-crashed-bubble",
		}
	}
}
