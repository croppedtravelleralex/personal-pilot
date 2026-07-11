package backend

import (
	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/behavior/inputplane"
)

func bindOSClickFallback(executor *behavior.CDPExecutor, pid int) {
	if executor == nil || pid <= 0 || !inputplane.OSAvailable() {
		return
	}
	capturedPID := pid
	executor.OSClickAtFallback = func(x, y float64) error {
		return inputplane.ExecuteClickAt(executor, capturedPID, x, y)
	}
}
