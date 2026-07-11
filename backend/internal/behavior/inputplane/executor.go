package inputplane

import (
	"fmt"

	"personal-pilot/backend/internal/behavior"
)

// ExecuteClick routes a click through OS input when plane is OS.
func ExecuteClick(plane Plane, executor *behavior.CDPExecutor, pid int, selector string, offsetX, offsetY int) error {
	if plane != PlaneOS {
		return fmt.Errorf("internal: ExecuteClick requires plane=os")
	}
	return osClick(executor, pid, selector, offsetX, offsetY)
}

// ExecuteDoubleClick routes a double-click through OS input.
func ExecuteDoubleClick(plane Plane, executor *behavior.CDPExecutor, pid int, selector string) error {
	if plane != PlaneOS {
		return fmt.Errorf("internal: ExecuteDoubleClick requires plane=os")
	}
	return osDoubleClick(executor, pid, selector)
}

// ExecuteRightClick routes a right-click through OS input.
func ExecuteRightClick(plane Plane, executor *behavior.CDPExecutor, pid int, selector string) error {
	if plane != PlaneOS {
		return fmt.Errorf("internal: ExecuteRightClick requires plane=os")
	}
	return osRightClick(executor, pid, selector)
}

// ExecuteType routes typing through OS keyboard injection.
func ExecuteType(plane Plane, executor *behavior.CDPExecutor, pid int, selector, text string, clearFirst, submitOnEnter bool) error {
	if plane != PlaneOS {
		return fmt.Errorf("internal: ExecuteType requires plane=os")
	}
	return osType(executor, pid, selector, text, clearFirst, submitOnEnter)
}
