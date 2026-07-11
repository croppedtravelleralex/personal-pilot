package persona

import (
	"testing"
	"time"
)

func TestEvolvePersonaFontsOnlyGrow(t *testing.T) {
	base := Library()[0]
	born := time.Now().UTC().Add(-200 * 24 * time.Hour)
	evolved := EvolvePersona(base, born, time.Now().UTC())
	if len(evolved.FontAllowlist) < len(base.FontAllowlist) {
		t.Fatalf("fonts shrank")
	}
	if evolved.GPURenderer != base.GPURenderer || evolved.HardwareConcurrency != base.HardwareConcurrency {
		t.Fatalf("hardware must stay fixed")
	}
}

func TestAssignPersonaIDDiverseAvoidsSaturation(t *testing.T) {
	used := map[string]int{}
	// Saturate first persona's feature key.
	first := Resolve(AssignPersonaID("seed-x"))
	if first == nil {
		t.Fatal("nil")
	}
	used[FeatureKey(*first)] = 10
	id := AssignPersonaIDDiverse("seed-x", used, 3)
	if id == "" {
		t.Fatal("empty")
	}
	p := Resolve(id)
	if p == nil {
		t.Fatal("resolve")
	}
	if FeatureKey(*p) == FeatureKey(*first) && used[FeatureKey(*first)] >= 3 {
		// Only acceptable if entire library shares key (shouldn't).
		t.Fatalf("expected diverse pick away from saturated key")
	}
}
