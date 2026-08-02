package inputplane

import "strings"

// Mode selects how pointer/keyboard actions are executed.
type Mode string

const (
	ModeAuto Mode = "auto"
	ModeOS   Mode = "os"
	ModeCDP  Mode = "cdp"
)

// Plane is the resolved execution backend.
type Plane string

const (
	PlaneOS  Plane = "os"
	PlaneCDP Plane = "cdp"
)

// ParseMode normalizes user/API input; empty → auto.
func ParseMode(raw string) Mode {
	switch Mode(strings.ToLower(strings.TrimSpace(raw))) {
	case ModeOS:
		return ModeOS
	case ModeCDP:
		return ModeCDP
	default:
		return ModeAuto
	}
}
