package backend_test

import (
	"testing"

	"personal-pilot/backend"
)

func TestAsymmetricShouldExecuteReturnsGateShape(t *testing.T) {
	app := backend.NewApp(t.TempDir())
	out, err := app.AsymmetricShouldExecute("missing-profile")
	if err != nil {
		t.Fatalf("AsymmetricShouldExecute: %v", err)
	}
	if _, ok := out["allowed"]; !ok {
		t.Fatalf("missing allowed field: %+v", out)
	}
	if _, ok := out["inWindow"]; !ok {
		t.Fatalf("missing inWindow field: %+v", out)
	}
}
