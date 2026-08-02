package persona

import "testing"

func TestLibrarySizeAndSources(t *testing.T) {
	lib := Library()
	if len(lib) < 12 {
		t.Fatalf("library size=%d, want >=12", len(lib))
	}
	seen := map[string]bool{}
	for _, p := range lib {
		if p.ID == "" || p.Source == "" || p.GPUVendor == "" || p.GPURenderer == "" {
			t.Fatalf("incomplete persona: %+v", p)
		}
		if seen[p.ID] {
			t.Fatalf("duplicate id %s", p.ID)
		}
		seen[p.ID] = true
	}
}

func TestAssignPersonaIDStable(t *testing.T) {
	a := AssignPersonaID("seed-alpha")
	b := AssignPersonaID("seed-alpha")
	if a == "" || a != b {
		t.Fatalf("stable assign failed: %q vs %q", a, b)
	}
	c := AssignPersonaID("seed-beta-different")
	if c == "" {
		t.Fatal("empty assign")
	}
	// Different seeds should usually differ; allow rare collision but check Resolve works.
	if Resolve(a) == nil || Resolve(c) == nil {
		t.Fatal("resolve failed")
	}
}

func TestAssignCoversMultiplePersonas(t *testing.T) {
	seen := map[string]bool{}
	for i := 0; i < 200; i++ {
		id := AssignPersonaID(string(rune('a'+(i%26))) + string(rune('0'+(i%10))) + string(rune(i)))
		seen[id] = true
	}
	if len(seen) < 4 {
		t.Fatalf("expected diverse personas, got %d unique", len(seen))
	}
}
