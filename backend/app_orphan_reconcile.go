package backend

import (
	"context"
	"time"

	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/logger"
)

const orphanReconcileInterval = 2 * time.Minute
const detachedBrowserMaxAge = 10 * time.Minute

// startOrphanReconcileLoop periodically cleans half-dead browser state (docs/54 CP3).
func (a *App) startOrphanReconcileLoop(ctx context.Context) {
	if a == nil {
		return
	}
	go func() {
		ticker := time.NewTicker(orphanReconcileInterval)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				n := a.reconcileOrphanBrowserState()
				if n > 0 && a.ctx != nil {
					events.EmitSystemRecoveryOrphanCleanup(a.ctx, events.SystemRecoveryOrphanCleanupPayload{Count: n})
					logger.New("Recovery").Info("orphan reconcile cleaned profiles",
						logger.F("count", n),
					)
				}
			}
		}
	}()
}

func (a *App) reconcileOrphanBrowserState() int {
	if a == nil || a.browserMgr == nil {
		return 0
	}
	cleaned := 0
	a.browserMgr.Mutex.Lock()
	defer a.browserMgr.Mutex.Unlock()
	for id, profile := range a.browserMgr.Profiles {
		if profile == nil || !profile.Running {
			continue
		}
		port := profile.DebugPort
		if port <= 0 {
			continue
		}
		if canConnectDebugPort(port, 250*time.Millisecond) {
			continue
		}
		// Marked running but CDP dead → orphan/half-dead.
		a.markProfileStoppedLocked(id, profile)
		cleaned++
	}
	return cleaned
}
