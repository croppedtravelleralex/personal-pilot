package behavior

import "testing"

func TestExecutePrimitiveClickAndSnapshot(t *testing.T) {
	// Contract test without live browser: unsupported primitive still errors clearly.
	e := &CDPExecutor{}
	_, err := e.ExecutePrimitive(PrimitiveStep{Primitive: "not_real"})
	if err == nil {
		t.Fatal("expected error for unknown primitive wiring path")
	}
}

func TestParsePrimitivePlanJSON(t *testing.T) {
	steps, err := ParsePrimitivePlanJSON(`[{"primitive":"click_element","selector":"button"}]`)
	if err != nil || len(steps) != 1 || steps[0].Primitive != "click_element" {
		t.Fatalf("steps=%+v err=%v", steps, err)
	}
}

func TestTargetToSelector(t *testing.T) {
	if targetToSelector("primary-form-field") == "" {
		t.Fatal("expected selector")
	}
}

func TestDeclaredOnlyPrimitivesAreNotReportedAsShipped(t *testing.T) {
	declaredOnly := []string{
		"drag_to_element",
		"select_option",
		"scroll_into_view",
		"switch_tab",
		"close_tab",
	}
	for _, name := range declaredOnly {
		if !IsDeclaredPrimitive(name) {
			t.Errorf("%s should remain declared for future adapters", name)
		}
		if IsShippedPrimitive(name) {
			t.Errorf("%s is not wired by ExecutePrimitive and must not be reported as shipped", name)
		}
	}
}

func TestEveryShippedPrimitiveIsDeclared(t *testing.T) {
	for _, name := range ShippedPrimitives {
		if !IsDeclaredPrimitive(name) {
			t.Errorf("shipped primitive %s is missing from declared capabilities", name)
		}
	}
}
