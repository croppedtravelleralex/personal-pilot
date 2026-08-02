package plugin

import "testing"

func TestRegistryIncludesBuiltIns(t *testing.T) {
	registry := NewRegistry()
	if _, ok := registry.Get("challenge:recaptcha"); !ok {
		t.Fatal("builtin plugin missing")
	}
}
