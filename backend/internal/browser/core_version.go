package browser

import (
	"context"
	"os/exec"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"time"
)

const DefaultChromiumVersion = "139.0.7258.154"

var fourPartVersionPattern = regexp.MustCompile(`(?i)(\d{2,3})[._-](\d+)[._-](\d+)[._-](\d+)`)

type CoreVersionInfo struct {
	Major  int    `json:"major"`
	Full   string `json:"full"`
	Source string `json:"source"`
}

type cachedCoreVersion struct {
	info      CoreVersionInfo
	expiresAt time.Time
}

var coreVersionCache = struct {
	sync.Mutex
	items map[string]cachedCoreVersion
}{items: make(map[string]cachedCoreVersion)}

// ResolveChromiumVersion resolves one browser version truth source. A concrete
// binary/core identity outranks stale profile launch arguments.
func ResolveChromiumVersion(core *Core, binaryPath string, profile *Profile, argGroups ...[]string) CoreVersionInfo {
	if info, ok := resolveVersionFromBinary(binaryPath); ok {
		return info
	}
	if core != nil {
		if info, ok := coreVersionFromText(strings.Join([]string{core.CorePath, core.CoreName, core.CoreId}, " "), "core_identity"); ok {
			return info
		}
	}
	if profile != nil {
		if info, ok := coreVersionFromText(profile.CoreId, "core_identity"); ok {
			return info
		}
		argGroups = append([][]string{profile.FingerprintArgs, profile.LaunchArgs}, argGroups...)
	}
	for _, args := range argGroups {
		for _, arg := range args {
			if info, ok := coreVersionFromText(arg, "runtime_args"); ok {
				return info
			}
		}
	}
	info, _ := coreVersionFromText(DefaultChromiumVersion, "fallback")
	return info
}

func resolveVersionFromBinary(binaryPath string) (CoreVersionInfo, bool) {
	binaryPath = strings.TrimSpace(binaryPath)
	if binaryPath == "" {
		return CoreVersionInfo{}, false
	}
	if info, ok := coreVersionFromText(binaryPath, "binary_path"); ok {
		return info, true
	}

	coreVersionCache.Lock()
	cached, ok := coreVersionCache.items[binaryPath]
	if ok && time.Now().Before(cached.expiresAt) {
		coreVersionCache.Unlock()
		return cached.info, cached.info.Full != ""
	}
	coreVersionCache.Unlock()

	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	output, err := exec.CommandContext(ctx, binaryPath, "--version").CombinedOutput()
	if err != nil {
		coreVersionCache.Lock()
		coreVersionCache.items[binaryPath] = cachedCoreVersion{expiresAt: time.Now().Add(time.Minute)}
		coreVersionCache.Unlock()
		return CoreVersionInfo{}, false
	}
	info, ok := coreVersionFromText(string(output), "binary_probe")
	coreVersionCache.Lock()
	coreVersionCache.items[binaryPath] = cachedCoreVersion{info: info, expiresAt: time.Now().Add(10 * time.Minute)}
	coreVersionCache.Unlock()
	return info, ok
}

func coreVersionFromText(value, source string) (CoreVersionInfo, bool) {
	matches := fourPartVersionPattern.FindStringSubmatch(value)
	if len(matches) != 5 {
		return CoreVersionInfo{}, false
	}
	major, err := strconv.Atoi(matches[1])
	if err != nil || major <= 0 {
		return CoreVersionInfo{}, false
	}
	return CoreVersionInfo{
		Major:  major,
		Full:   strings.Join(matches[1:], "."),
		Source: source,
	}, true
}

func ChromiumUserAgent(version CoreVersionInfo) string {
	full := strings.TrimSpace(version.Full)
	if full == "" {
		full = DefaultChromiumVersion
	}
	return "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/" + full + " Safari/537.36"
}
