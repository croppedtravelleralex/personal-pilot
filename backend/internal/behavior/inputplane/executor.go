package inputplane

import (
	"fmt"

	"personal-pilot/backend/internal/behavior"
)

// ExecuteClick routes a click through OS input when plane is OS.
func ExecuteClick(plane Plane, executor *behavior.CDPExecutor, pid int, selector string, offsetX, offsetY int) error {
	return ExecuteClickWithSeed(plane, executor, pid, selector, offsetX, offsetY, "")
}

// ExecuteClickWithSeed routes a click through OS input with BioNoise seed.
func ExecuteClickWithSeed(plane Plane, executor *behavior.CDPExecutor, pid int, selector string, offsetX, offsetY int, seed string) error {
	if plane != PlaneOS {
		return fmt.Errorf("internal: ExecuteClick requires plane=os")
	}
	return osClick(executor, pid, selector, offsetX, offsetY, seed)
}

// ExecuteDoubleClick routes a double-click through OS input.
func ExecuteDoubleClick(plane Plane, executor *behavior.CDPExecutor, pid int, selector string) error {
	return ExecuteDoubleClickWithSeed(plane, executor, pid, selector, "")
}

// ExecuteDoubleClickWithSeed routes a double-click with BioNoise seed.
func ExecuteDoubleClickWithSeed(plane Plane, executor *behavior.CDPExecutor, pid int, selector, seed string) error {
	if plane != PlaneOS {
		return fmt.Errorf("internal: ExecuteDoubleClick requires plane=os")
	}
	return osDoubleClick(executor, pid, selector, seed)
}

// ExecuteRightClick routes a right-click through OS input.
func ExecuteRightClick(plane Plane, executor *behavior.CDPExecutor, pid int, selector string) error {
	return ExecuteRightClickWithSeed(plane, executor, pid, selector, "")
}

// ExecuteRightClickWithSeed routes a right-click with BioNoise seed.
func ExecuteRightClickWithSeed(plane Plane, executor *behavior.CDPExecutor, pid int, selector, seed string) error {
	if plane != PlaneOS {
		return fmt.Errorf("internal: ExecuteRightClick requires plane=os")
	}
	return osRightClick(executor, pid, selector, seed)
}

// ExecuteType routes typing through OS keyboard injection.
func ExecuteType(plane Plane, executor *behavior.CDPExecutor, pid int, selector, text string, clearFirst, submitOnEnter bool) error {
	return ExecuteTypeWithSeed(plane, executor, pid, selector, text, clearFirst, submitOnEnter, "")
}

// ExecuteTypeWithSeed routes typing with BioNoise seed on focus click.
func ExecuteTypeWithSeed(plane Plane, executor *behavior.CDPExecutor, pid int, selector, text string, clearFirst, submitOnEnter bool, seed string) error {
	if plane != PlaneOS {
		return fmt.Errorf("internal: ExecuteType requires plane=os")
	}
	return osType(executor, pid, selector, text, clearFirst, submitOnEnter, seed)
}
