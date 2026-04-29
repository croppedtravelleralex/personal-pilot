package browser

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"
)

const cdpOwnershipRejected = "CDP ownership rejected"

// LaunchAuditSnapshot is the runtime evidence captured when Antbrowser starts a
// browser process. It is intentionally hash-based for launch inputs so CDP
// checks can compare ownership without exposing full args in API payloads.
type LaunchAuditSnapshot struct {
	BrowserExe           string `json:"browserExe"`
	ProfileID            string `json:"profileId"`
	CanonicalUserDataDir string `json:"canonicalUserDataDir"`
	ProxyHash            string `json:"proxyHash"`
	FingerprintArgsHash  string `json:"fingerprintArgsHash"`
	LaunchArgsHash       string `json:"launchArgsHash"`
	DebugPort            int    `json:"debugPort"`
	PID                  int    `json:"pid"`
	Timestamp            string `json:"timestamp"`
	AppMode              string `json:"appMode"`
}

type LaunchAuditInput struct {
	BrowserExe      string
	ProfileID       string
	UserDataDir     string
	ProxyHash       string
	FingerprintArgs []string
	LaunchArgs      []string
	DebugPort       int
	PID             int
	Timestamp       time.Time
	AppMode         string
}

type LaunchAuditExpectation struct {
	BrowserExe           string
	ProfileID            string
	CanonicalUserDataDir string
	ProxyHash            string
	FingerprintArgsHash  string
	LaunchArgsHash       string
	DebugPort            int
	PID                  int
}

func NewLaunchAuditSnapshot(input LaunchAuditInput) *LaunchAuditSnapshot {
	ts := input.Timestamp
	if ts.IsZero() {
		ts = time.Now()
	}
	appMode := strings.TrimSpace(input.AppMode)
	if appMode == "" {
		appMode = "unknown"
	}
	proxyHash := strings.TrimSpace(input.ProxyHash)
	if proxyHash == "" {
		proxyHash = HashLaunchAuditValue("")
	}

	return &LaunchAuditSnapshot{
		BrowserExe:           CanonicalAuditPath(input.BrowserExe),
		ProfileID:            strings.TrimSpace(input.ProfileID),
		CanonicalUserDataDir: CanonicalAuditPath(input.UserDataDir),
		ProxyHash:            proxyHash,
		FingerprintArgsHash:  HashLaunchAuditArgs(input.FingerprintArgs),
		LaunchArgsHash:       HashLaunchAuditArgs(input.LaunchArgs),
		DebugPort:            input.DebugPort,
		PID:                  input.PID,
		Timestamp:            ts.Format(time.RFC3339Nano),
		AppMode:              appMode,
	}
}

func CanonicalAuditPath(raw string) string {
	value := strings.TrimSpace(raw)
	if value == "" {
		return ""
	}
	if abs, err := filepath.Abs(value); err == nil {
		value = abs
	}
	if evaluated, err := filepath.EvalSymlinks(value); err == nil {
		value = evaluated
	}
	return filepath.Clean(value)
}

func HashLaunchAuditValue(value string) string {
	return hashLaunchAuditPayload(strings.TrimSpace(value))
}

func HashLaunchAuditArgs(args []string) string {
	normalized := make([]string, 0, len(args))
	for _, arg := range args {
		arg = strings.TrimSpace(arg)
		if arg != "" {
			normalized = append(normalized, arg)
		}
	}
	return hashLaunchAuditPayload(normalized)
}

func ProfileProxyAuditHash(profile *Profile, resolvedProxyConfig string) string {
	if profile == nil {
		return HashLaunchAuditValue("")
	}
	resolvedProxyConfig = strings.TrimSpace(resolvedProxyConfig)
	if resolvedProxyConfig == "" {
		resolvedProxyConfig = strings.TrimSpace(profile.ProxyConfig)
	}
	material := struct {
		ProxyID            string `json:"proxyId"`
		ProxyConfig        string `json:"proxyConfig"`
		ProxyBindSourceID  string `json:"proxyBindSourceId"`
		ProxyBindSourceURL string `json:"proxyBindSourceUrl"`
		ProxyBindName      string `json:"proxyBindName"`
		ResolvedProxy      string `json:"resolvedProxy"`
	}{
		ProxyID:            strings.TrimSpace(profile.ProxyId),
		ProxyConfig:        strings.TrimSpace(profile.ProxyConfig),
		ProxyBindSourceID:  strings.TrimSpace(profile.ProxyBindSourceID),
		ProxyBindSourceURL: strings.TrimSpace(profile.ProxyBindSourceURL),
		ProxyBindName:      strings.TrimSpace(profile.ProxyBindName),
		ResolvedProxy:      resolvedProxyConfig,
	}
	return hashLaunchAuditPayload(material)
}

func (m *Manager) ProfileProxyAuditHash(profile *Profile) string {
	if profile == nil {
		return HashLaunchAuditValue("")
	}
	resolvedProxyConfig := strings.TrimSpace(profile.ProxyConfig)
	if strings.TrimSpace(profile.ProxyId) != "" {
		if cfg, ok := m.GetProxyConfigById(profile.ProxyId); ok {
			resolvedProxyConfig = cfg
		}
	}
	return ProfileProxyAuditHash(profile, resolvedProxyConfig)
}

func (m *Manager) ValidateProfileLaunchAudit(profile *Profile) error {
	if m == nil {
		return ownershipError("browser manager unavailable")
	}
	if profile == nil {
		return ownershipError("profile is nil")
	}
	if !profile.Running {
		return ownershipError("profile %s is not running", profile.ProfileId)
	}
	if !profile.DebugReady || profile.DebugPort <= 0 {
		return ownershipError("debug port is not ready for profile %s", profile.ProfileId)
	}
	if profile.LaunchAudit == nil {
		return ownershipError("launch audit snapshot is missing for profile %s", profile.ProfileId)
	}
	if err := ValidateLaunchAuditSnapshot(profile.LaunchAudit, LaunchAuditExpectation{
		ProfileID: profile.ProfileId,
		DebugPort: profile.DebugPort,
		PID:       profile.Pid,
	}); err != nil {
		return err
	}

	browserExe, err := m.ResolveChromeBinary(profile)
	if err != nil {
		return ownershipError("executable path cannot be resolved for profile %s: %v", profile.ProfileId, err)
	}
	userDataDir, err := m.ResolveCanonicalUserDataDir(profile)
	if err != nil {
		return ownershipError("user-data-dir cannot be resolved for profile %s: %v", profile.ProfileId, err)
	}
	expected := LaunchAuditExpectation{
		BrowserExe:           browserExe,
		ProfileID:            profile.ProfileId,
		CanonicalUserDataDir: userDataDir,
		ProxyHash:            m.ProfileProxyAuditHash(profile),
		FingerprintArgsHash:  HashLaunchAuditArgs(profile.FingerprintArgs),
		DebugPort:            profile.DebugPort,
	}
	if profile.Pid > 0 {
		expected.PID = profile.Pid
	}
	return ValidateLaunchAuditSnapshot(profile.LaunchAudit, expected)
}

func ValidateLaunchAuditSnapshot(audit *LaunchAuditSnapshot, expected LaunchAuditExpectation) error {
	if audit == nil {
		return ownershipError("launch audit snapshot is missing for profile %s", expected.ProfileID)
	}
	if strings.TrimSpace(audit.ProfileID) == "" {
		return ownershipError("launch audit snapshot has empty profileId")
	}
	if expected.ProfileID != "" && audit.ProfileID != expected.ProfileID {
		return ownershipError("profileId mismatch: expected %s, audit %s", expected.ProfileID, audit.ProfileID)
	}
	if strings.TrimSpace(audit.BrowserExe) == "" {
		return ownershipError("launch audit snapshot has empty browser executable for profile %s", audit.ProfileID)
	}
	if expected.BrowserExe != "" && !sameAuditPath(audit.BrowserExe, expected.BrowserExe) {
		return ownershipError("browser executable mismatch for profile %s: expected %s, audit %s", audit.ProfileID, CanonicalAuditPath(expected.BrowserExe), audit.BrowserExe)
	}
	if strings.TrimSpace(audit.CanonicalUserDataDir) == "" {
		return ownershipError("launch audit snapshot has empty user-data-dir for profile %s", audit.ProfileID)
	}
	if expected.CanonicalUserDataDir != "" && !sameAuditPath(audit.CanonicalUserDataDir, expected.CanonicalUserDataDir) {
		return ownershipError("user-data-dir mismatch for profile %s: expected %s, audit %s", audit.ProfileID, CanonicalAuditPath(expected.CanonicalUserDataDir), audit.CanonicalUserDataDir)
	}
	if audit.DebugPort <= 0 {
		return ownershipError("launch audit snapshot has invalid debugPort for profile %s", audit.ProfileID)
	}
	if expected.DebugPort > 0 && audit.DebugPort != expected.DebugPort {
		return ownershipError("debugPort mismatch for profile %s: expected %d, audit %d", audit.ProfileID, expected.DebugPort, audit.DebugPort)
	}
	if audit.PID <= 0 {
		return ownershipError("launch audit snapshot has invalid PID for profile %s", audit.ProfileID)
	}
	if expected.PID > 0 && audit.PID != expected.PID {
		return ownershipError("PID mismatch for profile %s: expected %d, audit %d", audit.ProfileID, expected.PID, audit.PID)
	}
	if strings.TrimSpace(audit.Timestamp) == "" {
		return ownershipError("launch audit snapshot has empty timestamp for profile %s", audit.ProfileID)
	}
	if strings.TrimSpace(audit.AppMode) == "" {
		return ownershipError("launch audit snapshot has empty app mode for profile %s", audit.ProfileID)
	}
	if strings.TrimSpace(audit.ProxyHash) == "" {
		return ownershipError("launch audit snapshot has empty proxy hash for profile %s", audit.ProfileID)
	}
	if expected.ProxyHash != "" && audit.ProxyHash != expected.ProxyHash {
		return ownershipError("proxy hash mismatch for profile %s", audit.ProfileID)
	}
	if strings.TrimSpace(audit.FingerprintArgsHash) == "" {
		return ownershipError("launch audit snapshot has empty fingerprint args hash for profile %s", audit.ProfileID)
	}
	if expected.FingerprintArgsHash != "" && audit.FingerprintArgsHash != expected.FingerprintArgsHash {
		return ownershipError("fingerprint args hash mismatch for profile %s", audit.ProfileID)
	}
	if strings.TrimSpace(audit.LaunchArgsHash) == "" {
		return ownershipError("launch audit snapshot has empty launch args hash for profile %s", audit.ProfileID)
	}
	if expected.LaunchArgsHash != "" && audit.LaunchArgsHash != expected.LaunchArgsHash {
		return ownershipError("launch args hash mismatch for profile %s", audit.ProfileID)
	}
	return nil
}

func ownershipError(format string, args ...interface{}) error {
	return fmt.Errorf(cdpOwnershipRejected+": "+format, args...)
}

func sameAuditPath(a, b string) bool {
	left := CanonicalAuditPath(a)
	right := CanonicalAuditPath(b)
	if runtime.GOOS == "windows" {
		return strings.EqualFold(left, right)
	}
	return left == right
}

func hashLaunchAuditPayload(value interface{}) string {
	data, err := json.Marshal(value)
	if err != nil {
		data = []byte(fmt.Sprint(value))
	}
	sum := sha256.Sum256(data)
	return hex.EncodeToString(sum[:])
}

func ProcessAppMode() string {
	exe := strings.ToLower(filepath.Base(os.Args[0]))
	if strings.HasSuffix(exe, ".test") || strings.HasSuffix(exe, ".test.exe") {
		return "test"
	}
	if strings.TrimSpace(os.Getenv("TAURI_DEV_HOST")) != "" ||
		strings.EqualFold(strings.TrimSpace(os.Getenv("TAURI_ENV")), "development") ||
		strings.EqualFold(strings.TrimSpace(os.Getenv("ANTBROWSER_APP_MODE")), "dev") {
		return "dev"
	}
	if mode := strings.TrimSpace(os.Getenv("ANTBROWSER_APP_MODE")); mode != "" {
		return mode
	}
	return "desktop"
}
