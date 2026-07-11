package backend_test

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"personal-pilot/backend/internal/behavior"
)

func TestCapabilityScenarioManifestValid(t *testing.T) {
	root := findRepoRoot(t)
	manifestPath := filepath.Join(root, "scripts", "scenarios", "manifest.json")
	raw, err := os.ReadFile(manifestPath)
	if err != nil {
		t.Fatalf("read manifest: %v", err)
	}
	var doc struct {
		Schema    string `json:"schema"`
		Scenarios []struct {
			ID       string   `json:"id"`
			Tier     string   `json:"tier"`
			Runner   string   `json:"runner"`
			PlanFile string   `json:"planFile"`
			Command  string   `json:"command"`
			Dims     []string `json:"dimensions"`
		} `json:"scenarios"`
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatalf("parse manifest: %v", err)
	}
	if doc.Schema != "capability_scenario_manifest_v2" {
		t.Fatalf("unexpected schema: %s", doc.Schema)
	}
	if len(doc.Scenarios) < 30 {
		t.Fatalf("expected >=30 scenarios, got %d", len(doc.Scenarios))
	}
	seen := map[string]bool{}
	for _, sc := range doc.Scenarios {
		if sc.ID == "" || seen[sc.ID] {
			t.Fatalf("duplicate or empty id: %q", sc.ID)
		}
		seen[sc.ID] = true
		if sc.PlanFile != "" {
			planPath := filepath.Join(root, filepath.FromSlash(sc.PlanFile))
			if _, err := os.Stat(planPath); err != nil {
				t.Fatalf("plan file missing for %s: %v", sc.ID, err)
			}
		}
	}
}

func TestXHSFeedScrapePlanUsesShippedPrimitives(t *testing.T) {
	root := findRepoRoot(t)
	planPath := filepath.Join(root, "scripts", "scenarios", "plans", "xhs-feed-scrape.json")
	raw, err := os.ReadFile(planPath)
	if err != nil {
		t.Fatalf("read plan: %v", err)
	}
	var plan struct {
		Steps []struct {
			Primitive string `json:"primitive"`
		} `json:"steps"`
	}
	if err := json.Unmarshal(raw, &plan); err != nil {
		t.Fatalf("parse plan: %v", err)
	}
	for _, step := range plan.Steps {
		if !behavior.IsShippedPrimitive(step.Primitive) {
			t.Fatalf("plan uses unknown primitive: %s", step.Primitive)
		}
	}
}

func TestMultiPageScrapePlanUsesShippedPrimitives(t *testing.T) {
	root := findRepoRoot(t)
	planPath := filepath.Join(root, "scripts", "scenarios", "plans", "multi-page-scrape.json")
	raw, err := os.ReadFile(planPath)
	if err != nil {
		t.Fatalf("read plan: %v", err)
	}
	var plan struct {
		Steps []struct {
			Primitive string `json:"primitive"`
		} `json:"steps"`
	}
	if err := json.Unmarshal(raw, &plan); err != nil {
		t.Fatalf("parse plan: %v", err)
	}
	for _, step := range plan.Steps {
		if !behavior.IsShippedPrimitive(step.Primitive) {
			t.Fatalf("plan uses unknown primitive: %s", step.Primitive)
		}
	}
}

func findRepoRoot(t *testing.T) string {
	t.Helper()
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	dir := wd
	for i := 0; i < 8; i++ {
		if _, err := os.Stat(filepath.Join(dir, "scripts", "scenarios", "manifest.json")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	if strings.HasSuffix(wd, "backend") {
		return filepath.Dir(wd)
	}
	t.Fatal("repo root not found")
	return ""
}
