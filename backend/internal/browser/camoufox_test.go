package browser

import (
	"os"
	"path/filepath"
	"personal-pilot/backend/internal/config"
	"testing"
)

func TestCamoufoxExecutableCandidates(t *testing.T) {
	candidates := CamoufoxExecutableCandidates()
	if len(candidates) == 0 {
		t.Fatal("CamoufoxExecutableCandidates should return at least one candidate")
	}
	for _, candidate := range candidates {
		if candidate == "" {
			t.Fatal("candidate should not be empty")
		}
	}
}

func TestFindCamoufoxExecutable_Found(t *testing.T) {
	dir := t.TempDir()
	exeName := CamoufoxExecutableCandidates()[0]
	exePath := filepath.Join(dir, filepath.FromSlash(exeName))
	if err := os.MkdirAll(filepath.Dir(exePath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	gotPath, gotName, ok := FindCamoufoxExecutable(dir)
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

func TestResolveBrowserBinary_Camoufox(t *testing.T) {
	dir := t.TempDir()
	exeName := CamoufoxExecutableCandidates()[0]
	exePath := filepath.Join(dir, filepath.FromSlash(exeName))
	if err := os.MkdirAll(filepath.Dir(exePath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	mgr := NewManager(&config.Config{}, dir)
	core := Core{CoreId: "cf-1", CoreName: "Camoufox", CorePath: dir, Kind: config.CoreKindCamoufox}

	got, err := mgr.ResolveBrowserBinary(core)
	if err != nil {
		t.Fatalf("ResolveBrowserBinary: %v", err)
	}
	if got != exePath {
		t.Fatalf("path = %q, want %q", got, exePath)
	}
}

func TestValidateCorePathForKind_Camoufox(t *testing.T) {
	dir := t.TempDir()
	exeName := CamoufoxExecutableCandidates()[0]
	exePath := filepath.Join(dir, filepath.FromSlash(exeName))
	if err := os.MkdirAll(filepath.Dir(exePath), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(exePath, []byte("fake"), 0755); err != nil {
		t.Fatal(err)
	}

	mgr := NewManager(&config.Config{}, dir)
	result := mgr.ValidateCorePathForKind(dir, config.CoreKindCamoufox)
	if !result.Valid {
		t.Fatalf("ValidateCorePathForKind should be valid: %s", result.Message)
	}
}

func TestBuildCamoufoxLaunchArgs(t *testing.T) {
	args := BuildCamoufoxLaunchArgs(9222, "data/profile", []string{"--private-window"})
	assertContainsArg(t, args, "--remote-debugging-port=9222")
	assertContainsArg(t, args, "--profile")
	assertContainsArg(t, args, "data/profile")
	assertContainsArg(t, args, "--private-window")
}

func TestBuildCoreLaunchArgs_CamoufoxUsesProfileFlag(t *testing.T) {
	args := BuildCoreLaunchArgs(config.CoreKindCamoufox, 9222, "data/profile")
	assertContainsArg(t, args, "--remote-debugging-port=9222")
	assertContainsArg(t, args, "--profile")
	assertContainsArg(t, args, "data/profile")
	for _, arg := range args {
		if arg == "--user-data-dir=data/profile" {
			t.Fatalf("camoufox launch args should not use chromium user-data-dir: %v", args)
		}
	}
}

func TestBuildCoreLaunchArgs_ChromiumUsesUserDataDir(t *testing.T) {
	args := BuildCoreLaunchArgs(config.CoreKindChromium, 9222, "data/profile")
	assertContainsArg(t, args, "--remote-debugging-port=9222")
	assertContainsArg(t, args, "--user-data-dir=data/profile")
	assertContainsArg(t, args, "--disable-session-crashed-bubble")
}

func TestIsCamoufoxKind_NormalizesWhitespaceAndCase(t *testing.T) {
	if !IsCamoufoxKind(" Camoufox ") {
		t.Fatal("expected Camoufox kind to normalize")
	}
}

func assertContainsArg(t *testing.T, args []string, want string) {
	t.Helper()
	for _, arg := range args {
		if arg == want {
			return
		}
	}
	t.Fatalf("missing arg %q in %v", want, args)
}
