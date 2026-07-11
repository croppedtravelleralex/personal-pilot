//go:build !windows

package inputplane

import (
	"fmt"

	"personal-pilot/backend/internal/behavior"
)

// OSAvailable reports whether OS-level input injection is supported on this platform.
func OSAvailable() bool { return false }

// ExecuteClickAt is unsupported on non-Windows platforms.
func ExecuteClickAt(*behavior.CDPExecutor, int, float64, float64) error {
	return fmt.Errorf("os input: platform unsupported")
}

func osClick(*behavior.CDPExecutor, int, string, int, int) error {
	return fmt.Errorf("os input: platform unsupported")
}

func osDoubleClick(*behavior.CDPExecutor, int, string) error {
	return fmt.Errorf("os input: platform unsupported")
}

func osRightClick(*behavior.CDPExecutor, int, string) error {
	return fmt.Errorf("os input: platform unsupported")
}

func osType(*behavior.CDPExecutor, int, string, string, bool, bool) error {
	return fmt.Errorf("os input: platform unsupported")
}
