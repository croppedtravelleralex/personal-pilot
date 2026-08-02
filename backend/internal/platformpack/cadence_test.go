package platformpack

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestLoadCadenceXHS(t *testing.T) {
	root := findRepoRoot(t)
	cfg, err := LoadCadence(root, "xhs")
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Nurture.MaxActionsPerSession != 12 {
		t.Fatalf("maxActions=%d", cfg.Nurture.MaxActionsPerSession)
	}
	if len(cfg.Nurture.SessionMinutes) < 2 {
		t.Fatalf("sessionMinutes=%v", cfg.Nurture.SessionMinutes)
	}
	budget, minD, maxD := cfg.ApplyToBudget(20, time.Minute, 20*time.Minute)
	if budget != 12 {
		t.Fatalf("budget=%d want 12", budget)
	}
	if minD != 8*time.Minute || maxD != 15*time.Minute {
		t.Fatalf("durations min=%v max=%v", minD, maxD)
	}
}

func findRepoRoot(t *testing.T) string {
	t.Helper()
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	dir := wd
	for i := 0; i < 6; i++ {
		if _, err := os.Stat(filepath.Join(dir, "platform-packs", "xhs", "cadence.yaml")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	t.Fatal("repo root with platform-packs/xhs/cadence.yaml not found")
	return ""
}
