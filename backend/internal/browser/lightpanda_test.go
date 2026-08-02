package browser

import (
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/config"
	"testing"
)

func TestLightpandaExecutableCandidates(t *testing.T) {
	candidates := LightpandaExecutableCandidates()
	if len(candidates) == 0 {
		t.Fatal("LightpandaExecutableCandidates should return at least one candidate")
	}
	// On Windows, should include lightpanda.exe
	for _, c := range candidates {
		if c == "" {
			t.Fatal("candidate should not be empty")
		}
	}
}

func TestFindLightpandaExecutable_NotFound(t *testing.T) {
	_, _, ok := FindLightpandaExecutable("/nonexistent/path")
	if ok {
		t.Fatal("Should not find executable in nonexistent path")
	}
}

func TestFindLightpandaExecutable_Found(t *testing.T) {
	dir := t.TempDir()
	candidates := LightpandaExecutableCandidates()
	if len(candidates) == 0 {
		t.Skip("no candidates")
	}
	exeName := candidates[0]
	exePath := filepath.Join(dir, exeName)
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}
	gotPath, gotName, ok := FindLightpandaExecutable(dir)
	if !ok {
		t.Fatalf("Should find %s in %s", exeName, dir)
	}
	if gotName != exeName {
		t.Fatalf("name = %q, want %q", gotName, exeName)
	}
	if gotPath != exePath {
		t.Fatalf("path = %q, want %q", gotPath, exePath)
	}
}

func TestResolveBrowserBinary_Lightpanda(t *testing.T) {
	dir := t.TempDir()
	candidates := LightpandaExecutableCandidates()
	if len(candidates) == 0 {
		t.Skip("no lightpanda candidates")
	}
	exePath := filepath.Join(dir, candidates[0])
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	mgr := NewManager(&config.Config{}, dir)
	core := Core{
		CoreId:   "lp-1",
		CoreName: "Lightpanda",
		CorePath: dir,
		Kind:     config.CoreKindLightpanda,
	}

	got, err := mgr.ResolveBrowserBinary(core)
	if err != nil {
		t.Fatalf("ResolveBrowserBinary: %v", err)
	}
	if got != exePath {
		t.Fatalf("path = %q, want %q", got, exePath)
	}
}

func TestResolveBrowserBinary_Chromium(t *testing.T) {
	dir := t.TempDir()
	exePath := filepath.Join(dir, "chrome.exe")
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	mgr := NewManager(&config.Config{}, dir)
	core := Core{
		CoreId:   "ch-1",
		CoreName: "Chromium",
		CorePath: dir,
		Kind:     config.CoreKindChromium,
	}

	got, err := mgr.ResolveBrowserBinary(core)
	if err != nil {
		t.Fatalf("ResolveBrowserBinary: %v", err)
	}
	if got != exePath {
		t.Fatalf("path = %q, want %q", got, exePath)
	}
}

func TestResolveBrowserBinary_DefaultKind(t *testing.T) {
	dir := t.TempDir()
	exePath := filepath.Join(dir, "chrome.exe")
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	mgr := NewManager(&config.Config{}, dir)
	core := Core{
		CoreId:   "ch-2",
		CoreName: "Default",
		CorePath: dir,
		Kind:     "", // empty should default to chromium
	}

	got, err := mgr.ResolveBrowserBinary(core)
	if err != nil {
		t.Fatalf("ResolveBrowserBinary: %v", err)
	}
	if got != exePath {
		t.Fatalf("path = %q, want %q", got, exePath)
	}
}

func TestBuildLightpandaLaunchArgs(t *testing.T) {
	args := BuildLightpandaLaunchArgs(9222)
	if len(args) < 2 {
		t.Fatalf("expected at least 2 args, got %d: %v", len(args), args)
	}
	hasPort := false
	hasHeadless := false
	for _, a := range args {
		if a == "--port=9222" {
			hasPort = true
		}
		if a == "--headless" {
			hasHeadless = true
		}
	}
	if !hasPort {
		t.Fatal("missing --port flag")
	}
	if !hasHeadless {
		t.Fatal("missing --headless flag")
	}
}

func TestValidateCorePathForKind_Lightpanda(t *testing.T) {
	dir := t.TempDir()
	candidates := LightpandaExecutableCandidates()
	if len(candidates) == 0 {
		t.Skip("no lightpanda candidates")
	}
	exePath := filepath.Join(dir, candidates[0])
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	mgr := NewManager(&config.Config{}, dir)
	result := mgr.ValidateCorePathForKind(dir, config.CoreKindLightpanda)
	if !result.Valid {
		t.Fatalf("ValidateCorePathForKind should be valid: %s", result.Message)
	}
}

func TestCoreExecutableCandidatesForKind(t *testing.T) {
	chromiumCandidates := CoreExecutableCandidatesForKind("chromium")
	if len(chromiumCandidates) == 0 {
		t.Fatal("chromium candidates should not be empty")
	}

	lpCandidates := CoreExecutableCandidatesForKind("lightpanda")
	if len(lpCandidates) == 0 {
		t.Fatal("lightpanda candidates should not be empty")
	}

	cfCandidates := CoreExecutableCandidatesForKind("camoufox")
	if len(cfCandidates) == 0 {
		t.Fatal("camoufox candidates should not be empty")
	}

	// The engine-specific lists should differ from the Chromium defaults.
	if len(chromiumCandidates) == len(lpCandidates) && chromiumCandidates[0] == lpCandidates[0] {
		t.Fatal("chromium and lightpanda candidates should differ")
	}
	if len(chromiumCandidates) == len(cfCandidates) && chromiumCandidates[0] == cfCandidates[0] {
		t.Fatal("chromium and camoufox candidates should differ")
	}
}
