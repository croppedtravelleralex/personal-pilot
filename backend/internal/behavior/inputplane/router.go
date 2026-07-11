package inputplane

import "strings"

// ResolvePlane picks OS vs CDP for a workbench action.
func ResolvePlane(mode Mode, actionType string, hasFrame bool, osAvailable bool) Plane {
	if mode == ModeCDP {
		return PlaneCDP
	}
	if hasFrame {
		return PlaneCDP
	}
	if !osAvailable || !isOSCapableAction(actionType) {
		return PlaneCDP
	}
	if mode == ModeOS || mode == ModeAuto {
		return PlaneOS
	}
	return PlaneCDP
}

func isOSCapableAction(actionType string) bool {
	switch strings.ToLower(strings.TrimSpace(actionType)) {
	case "click", "type", "text", "double-click", "right-click", "click-offset":
		return true
	default:
		return false
	}
}
