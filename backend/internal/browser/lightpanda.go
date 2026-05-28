package browser

import (
	"fmt"
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/config"
	goruntime "runtime"
	"strings"
)

// LightpandaExecutableCandidates returns platform-specific Lightpanda executable names.
func LightpandaExecutableCandidates() []string {
	switch goruntime.GOOS {
	case "windows":
		return []string{"lightpanda.exe", "lightpanda-windows-amd64.exe"}
	case "linux":
		return []string{"lightpanda", "lightpanda-linux-amd64"}
	case "darwin":
		return []string{"lightpanda", "lightpanda-macos-amd64"}
	default:
		return []string{"lightpanda"}
	}
}

// FindLightpandaExecutable searches a directory for a Lightpanda binary.
func FindLightpandaExecutable(baseDir string) (string, string, bool) {
	baseDir = strings.TrimSpace(baseDir)
	if baseDir == "" {
		return "", "", false
	}
	for _, candidate := range LightpandaExecutableCandidates() {
		p := filepath.Join(baseDir, filepath.FromSlash(candidate))
		if _, err := os.Stat(p); err == nil {
			return p, candidate, true
		}
	}
	return "", "", false
}

// ResolveLightpandaExecutable resolves a Lightpanda Core to its executable path.
func (m *Manager) ResolveLightpandaExecutable(core Core) (string, error) {
	corePath := strings.TrimSpace(core.CorePath)
	if corePath == "" {
		return "", fmt.Errorf("Lightpanda 内核路径为空，请在\"内核管理\"中补充内核目录")
	}

	baseDir := m.ResolveRelativePath(corePath)
	exePath, _, ok := FindLightpandaExecutable(baseDir)
	if !ok {
		return "", fmt.Errorf("Lightpanda 内核目录无效：未找到可执行文件（候选：%s）", strings.Join(LightpandaExecutableCandidates(), ", "))
	}

	return exePath, nil
}

// ResolveBrowserBinary resolves the correct binary for a given core based on its Kind.
func (m *Manager) ResolveBrowserBinary(core Core) (string, error) {
	kind := normalizeCoreKind(strings.TrimSpace(core.Kind))

	switch kind {
	case config.CoreKindLightpanda:
		return m.ResolveLightpandaExecutable(core)
	case config.CoreKindCamoufox:
		return m.ResolveCamoufoxExecutable(core)
	default:
		return m.ResolveCoreExecutable(core)
	}
}

// BuildLightpandaLaunchArgs builds launch arguments for a Lightpanda process.
// Lightpanda uses --port instead of --remote-debugging-port and has simpler arg structure.
func BuildLightpandaLaunchArgs(debugPort int) []string {
	return []string{
		fmt.Sprintf("--port=%d", debugPort),
		"--headless",
	}
}
