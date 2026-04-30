package events

import "testing"

func TestIdentityCatalogExpandsGlobalEventSet(t *testing.T) {
	names := AllRegistryNames()
	if len(names) < 350 {
		t.Fatalf("registry event count = %d, want >= 350", len(names))
	}
	for _, name := range []string{
		"identity:report:completed",
		"fingerprint:audio:captured",
		"cookie:unexpected-cleared",
		"automation:target:hit-test-fail",
	} {
		if _, ok := Registry[name]; !ok {
			t.Fatalf("registry missing %s", name)
		}
	}
	if len(AllEventNames()) != len(names) {
		t.Fatalf("AllEventNames count = %d, registry count = %d", len(AllEventNames()), len(names))
	}
}
