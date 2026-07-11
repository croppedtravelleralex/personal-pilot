package asymmetric

import (
	"testing"
	"time"
)

func TestHumanTimeWindow(t *testing.T) {
	p := DefaultHumanTimePolicy()
	p.Timezone = "UTC"
	at, _ := time.Parse(time.RFC3339, "2026-06-29T14:00:00Z")
	if !p.InHumanWindow(at) {
		t.Fatal("expected active hour")
	}
	night, _ := time.Parse(time.RFC3339, "2026-06-29T03:00:00Z")
	if p.InHumanWindow(night) {
		t.Fatal("expected blocked overnight")
	}
}

func TestEntropyHumanBand(t *testing.T) {
	var counts [24]int
	for h := 8; h < 18; h++ {
		counts[h] = 3
	}
	h := ActivityEntropyInput{HourlyCounts: counts}.ShannonEntropyH()
	assess := AssessEntropy(h)
	if assess.Level != "human_like" {
		t.Fatalf("expected human_like got %s h=%f", assess.Level, h)
	}
}

func TestCostAsymmetryAPIFirst(t *testing.T) {
	report := EvaluateCostAsymmetry(CostAsymmetryInput{
		HasTrustBundle: true, HasGraphToken: true, ResidentialProxy: true,
		SingleAccountMode: true, LowFrequencyOps: true, InHumanWindow: true,
		EntropyHumanLike: true, DetectionScore: 90, AccountSuccessRate: 90,
	})
	if report.Score < 90 {
		t.Fatalf("score=%d strategy=%s", report.Score, report.Strategy)
	}
	if report.Strategy != "api_first_trust_inheritance" {
		t.Fatalf("strategy=%s", report.Strategy)
	}
}
