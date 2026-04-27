package browser

import "testing"

func TestFingerprintFieldPriorityRules_CoversAllLayers(t *testing.T) {
	rules := FingerprintFieldPriorityRules()
	hasL1 := false
	hasL2 := false
	hasL3 := false
	for _, r := range rules {
		switch r.Layer {
		case FPLayerL1:
			hasL1 = true
		case FPLayerL2:
			hasL2 = true
		case FPLayerL3:
			hasL3 = true
		}
	}
	if !hasL1 || !hasL2 || !hasL3 {
		t.Fatalf("Missing layers: L1=%v L2=%v L3=%v", hasL1, hasL2, hasL3)
	}
}

func TestClassifyFingerprintField(t *testing.T) {
	tests := []struct {
		field string
		want  *FingerprintFieldPriorityLayer
	}{
		{"timezone", ptrLayer(FPLayerL1)},
		{"client_hints", ptrLayer(FPLayerL2)},
		{"audio", ptrLayer(FPLayerL3)},
		{"unknown_field", nil},
	}
	for _, tc := range tests {
		got := ClassifyFingerprintField(tc.field)
		if tc.want == nil {
			if got != nil {
				t.Fatalf("ClassifyFingerprintField(%q) = %v, want nil", tc.field, got)
			}
		} else if got == nil || *got != *tc.want {
			t.Fatalf("ClassifyFingerprintField(%q) = %v, want %v", tc.field, got, tc.want)
		}
	}
}

func TestDefaultPerfBudgetForLayer(t *testing.T) {
	tests := []struct {
		layer FingerprintFieldPriorityLayer
		want  FingerprintPerfBudgetTag
	}{
		{FPLayerL1, FPBudgetLight},
		{FPLayerL2, FPBudgetMedium},
		{FPLayerL3, FPBudgetHeavy},
	}
	for _, tc := range tests {
		got := DefaultPerfBudgetForLayer(tc.layer)
		if got != tc.want {
			t.Fatalf("DefaultPerfBudgetForLayer(%v) = %v, want %v", tc.layer, got, tc.want)
		}
	}
}

func ptrLayer(l FingerprintFieldPriorityLayer) *FingerprintFieldPriorityLayer { return &l }
