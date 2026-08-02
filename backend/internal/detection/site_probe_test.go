package detection

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestParseCreepJSProbePayloadStructuredTrustScore(t *testing.T) {
	trust := 92.5
	raw, err := json.Marshal(map[string]interface{}{
		"webdriver":      false,
		"bodySnippet":    "Trust Score 92.5\nGrade: A\nFP ID: abcdef123456\nplatform lie\n",
		"trustScore":     trust,
		"grade":          "A",
		"fingerprintId":  "abcdef123456",
		"lies":           1,
		"lieLines":       []string{"platform lie"},
		"headlessHints":  []string{},
		"workerMismatch": false,
		"structured":     true,
		"computing":      false,
	})
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	got := ParseCreepJSProbePayload(raw)
	if !got.ParseOK {
		t.Fatalf("ParseOK=false msg=%q", got.Message)
	}
	if got.Source != "structured" {
		t.Fatalf("Source=%q want structured", got.Source)
	}
	if got.TrustScore != 92.5 {
		t.Fatalf("TrustScore=%v", got.TrustScore)
	}
	if got.Grade != "A" {
		t.Fatalf("Grade=%q", got.Grade)
	}
	if got.FingerprintID != "abcdef123456" {
		t.Fatalf("FingerprintID=%q", got.FingerprintID)
	}
	if got.LiesDetected != 1 || len(got.Lies) == 0 {
		t.Fatalf("lies=%d lines=%v", got.LiesDetected, got.Lies)
	}
}

func TestParseCreepJSProbePayloadParsedTrustScoreWithLies(t *testing.T) {
	body := "Fingerprint\nTrust Score: 65.5\nLies detected: 2\nplatform lie detected\n"
	raw, err := json.Marshal(map[string]interface{}{
		"webdriver":      false,
		"bodySnippet":    body,
		"lies":           2,
		"lieLines":       []string{"platform lie detected"},
		"headlessHints":  []string{},
		"workerMismatch": false,
	})
	if err != nil {
		t.Fatalf("marshal payload: %v", err)
	}

	got := ParseCreepJSProbePayload(raw)
	if !got.ParseOK {
		t.Fatalf("ParseOK = false, message=%q", got.Message)
	}
	if got.Source != "parsed" {
		t.Fatalf("Source = %q, want parsed", got.Source)
	}
	if got.TrustScore != 65.5 {
		t.Fatalf("TrustScore = %v, want 65.5", got.TrustScore)
	}
	if len(got.Lies) == 0 {
		t.Fatal("Lies should be non-empty")
	}
	if got.LiesDetected != 2 {
		t.Fatalf("LiesDetected = %d, want 2", got.LiesDetected)
	}
	if got.WorkerConsistent == nil || !*got.WorkerConsistent {
		t.Fatalf("WorkerConsistent = %v, want true", got.WorkerConsistent)
	}
}

func TestParseCreepJSProbePayloadHeuristicFallback(t *testing.T) {
	raw, err := json.Marshal(map[string]interface{}{
		"webdriver":      true,
		"bodySnippet":    "no trust score here",
		"lies":           3,
		"lieLines":       []string{"ua lie"},
		"headlessHints":  []string{"headless"},
		"workerMismatch": true,
	})
	if err != nil {
		t.Fatalf("marshal payload: %v", err)
	}

	got := ParseCreepJSProbePayload(raw)
	if got.Source != "heuristic" {
		t.Fatalf("Source = %q, want heuristic", got.Source)
	}
	if !got.ParseOK {
		t.Fatalf("ParseOK = false")
	}
	// 100 - 40(webdriver) - 9(lies*3) - 8(headless) = 43
	if got.TrustScore != 43 {
		t.Fatalf("TrustScore = %v, want 43", got.TrustScore)
	}
	if got.WorkerConsistent == nil || *got.WorkerConsistent {
		t.Fatalf("WorkerConsistent = %v, want false", got.WorkerConsistent)
	}
	if len(got.HeadlessHints) != 1 || got.HeadlessHints[0] != "headless" {
		t.Fatalf("HeadlessHints = %v", got.HeadlessHints)
	}
}

func TestCreepJSProbeJSIncludesStructuredFields(t *testing.T) {
	js := CreepJSProbeJS()
	for _, needle := range []string{"lieLines", "headlessHints", "workerMismatch", "trustScore", "structured", "fingerprintId", "grade"} {
		if !strings.Contains(js, needle) {
			t.Fatalf("probe JS missing %q", needle)
		}
	}
}
