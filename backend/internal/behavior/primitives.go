package behavior

// DeclaredPrimitives lists control-plane capabilities understood by plans and adapters.
// A declared primitive is not necessarily executable by the current Go CDP runtime.
var DeclaredPrimitives = []string{
	"idle",
	"wait_for_readiness",
	"wait_for_content_stable",
	"scroll_progressive",
	"scroll_to_ratio",
	"pause_on_content",
	"focus_element",
	"blur_element",
	"hover_candidate",
	"type_with_rhythm",
	"clear_with_corrections",
	"persist_session_state",
	"soft_abort_if_budget_exceeded",
	"click_element",
	"double_click_element",
	"right_click_element",
	"drag_to_element",
	"select_option",
	"press_key",
	"press_key_combo",
	"scroll_into_view",
	"wait_for_selector",
	"wait_for_navigation",
	"capture_screenshot",
	"get_page_html",
	"get_element_text",
	"fill_form_field",
	"switch_tab",
	"close_tab",
	"open_url",
	"simulate_natural_browsing",
	"show_mouse_pointer",
	"hide_mouse_pointer",
	"evaluate_script",
	"dom_snapshot",
}

// ShippedPrimitives lists primitives with a concrete ExecutePrimitive branch in
// the current Go CDP runtime. Keep future adapter capabilities in
// DeclaredPrimitives until their runtime implementation and tests land.
var ShippedPrimitives = []string{
	"idle",
	"wait_for_readiness",
	"wait_for_content_stable",
	"scroll_progressive",
	"scroll_to_ratio",
	"pause_on_content",
	"focus_element",
	"blur_element",
	"hover_candidate",
	"type_with_rhythm",
	"clear_with_corrections",
	"persist_session_state",
	"soft_abort_if_budget_exceeded",
	"click_element",
	"double_click_element",
	"right_click_element",
	"press_key",
	"press_key_combo",
	"wait_for_selector",
	"wait_for_navigation",
	"capture_screenshot",
	"get_page_html",
	"get_element_text",
	"fill_form_field",
	"open_url",
	"simulate_natural_browsing",
	"show_mouse_pointer",
	"hide_mouse_pointer",
	"evaluate_script",
	"dom_snapshot",
}

// IsDeclaredPrimitive reports whether name is recognized by the control plane.
func IsDeclaredPrimitive(name string) bool {
	return containsPrimitive(DeclaredPrimitives, name)
}

// IsShippedPrimitive reports whether name is executable by the current Go runtime.
func IsShippedPrimitive(name string) bool {
	return containsPrimitive(ShippedPrimitives, name)
}

func containsPrimitive(items []string, name string) bool {
	for _, item := range items {
		if item == name {
			return true
		}
	}
	return false
}
