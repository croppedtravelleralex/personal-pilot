package backend

import (
	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/behavior/inputplane"
)

func bindOSClickFallback(executor *behavior.CDPExecutor, pid int, humanizeSeed string) {
	if executor == nil || pid <= 0 || !inputplane.OSAvailable() {
		return
	}
	capturedPID := pid
	seed := humanizeSeed
	executor.OSClickAtFallback = func(x, y float64) error {
		return inputplane.ExecuteClickAtWithSeed(executor, capturedPID, x, y, seed)
	}
}
