package offsets

import "testing"

func TestBuiltinOffsetLibrary_Has50Variants(t *testing.T) {
	lib := BuiltinOffsetLibrary()
	if len(lib) != 50 {
		t.Fatalf("expected 50 variants, got %d", len(lib))
	}
}

func TestBuiltinOffsetLibrary_CategoryCounts(t *testing.T) {
	lib := BuiltinOffsetLibrary()
	counts := map[string]int{}
	for _, v := range lib {
		counts[v.Category]++
	}
	expected := map[string]int{
		"open": 10, "browse": 15, "click": 15, "type": 5, "wait": 5,
	}
	for cat, want := range expected {
		if counts[cat] != want {
			t.Errorf("category %s: got %d, want %d", cat, counts[cat], want)
		}
	}
}

func TestBuiltinOffsetLibrary_AllIDsUnique(t *testing.T) {
	lib := BuiltinOffsetLibrary()
	seen := map[string]bool{}
	for _, v := range lib {
		if seen[v.ID] {
			t.Errorf("duplicate ID: %s", v.ID)
		}
		seen[v.ID] = true
	}
}

func TestBuiltinOffsetLibrary_NotEmptyFields(t *testing.T) {
	lib := BuiltinOffsetLibrary()
	for _, v := range lib {
		if v.ID == "" {
			t.Error("variant has empty ID")
		}
		if v.Name == "" {
			t.Errorf("variant %s has empty Name", v.ID)
		}
		if v.Category == "" {
			t.Errorf("variant %s has empty Category", v.ID)
		}
		if v.BehaviorPreset == "" {
			t.Errorf("variant %s has empty BehaviorPreset", v.ID)
		}
	}
}

func TestSelector_RoundRobin(t *testing.T) {
	s := NewSelector()

	// First 10 open selections should cycle through all 10 open variants
	seen := map[string]bool{}
	for i := 0; i < 10; i++ {
		v := s.Select("open")
		if v.Category != "open" {
			t.Fatalf("expected open variant, got %s", v.Category)
		}
		seen[v.ID] = true
	}
	if len(seen) != 10 {
		t.Errorf("expected 10 unique variants, got %d", len(seen))
	}

	// 11th selection should cycle back
	v := s.Select("open")
	if seen[v.ID] {
		t.Logf("round-robin cycled correctly, got %s again", v.ID)
	}
}

func TestSelector_UnknownCategory(t *testing.T) {
	s := NewSelector()
	v := s.Select("nonexistent")
	if v.ID != "" {
		t.Errorf("expected zero value for unknown category, got %+v", v)
	}
}

func TestGetByID(t *testing.T) {
	v, ok := GetByID("open-001")
	if !ok {
		t.Fatal("open-001 not found")
	}
	if v.Name == "" || v.Category != "open" {
		t.Errorf("incorrect open-001: %+v", v)
	}

	_, ok = GetByID("nonexistent")
	if ok {
		t.Error("should not find nonexistent variant")
	}
}

func TestGetByCategory(t *testing.T) {
	variants := GetByCategory("browse")
	if len(variants) != 15 {
		t.Errorf("expected 15 browse variants, got %d", len(variants))
	}
	for _, v := range variants {
		if v.Category != "browse" {
			t.Errorf("expected browse, got %s: %s", v.Category, v.ID)
		}
	}
}

func TestSelectAllForPlan(t *testing.T) {
	s := NewSelector()
	categories := []string{"open", "browse", "click", "click", "wait"}
	result := s.SelectAllForPlan(categories)
	if len(result) != 5 {
		t.Fatalf("expected 5 results, got %d", len(result))
	}
	for i, cat := range categories {
		if result[i].Category != cat {
			t.Errorf("result[%d] category: got %s, want %s", i, result[i].Category, cat)
		}
	}
}
