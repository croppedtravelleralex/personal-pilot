package backend

import (
	"os"
	"path/filepath"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior/lifecycle"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/platformpack"
)

func (a *App) touchProfileLifecycle(profileID string) {
	if a == nil || a.lifecycleStore == nil || profileID == "" {
		return
	}
	state, err := a.lifecycleStore.Load(profileID)
	if err != nil {
		state = lifecycle.State{
			ProfileID:      profileID,
			CreatedAt:      time.Now().UTC(),
			SocialStage:    "only-read",
			InterestVector: map[string]float64{},
		}
	}
	engine := a.lifecycleEngine()
	strategy := engine.PlanDailySession(state)
	state = engine.EvolveInterest(state, "default", 0.1)
	_ = a.lifecycleStore.Save(state)
	_ = a.lifecycleStore.IncrementSession(profileID, time.Now().UTC())
	if a.ctx != nil {
		events.EmitAndLog(a.ctx, "behavior:cadence:planned", map[string]interface{}{
			"profileId":     profileID,
			"cadence":       strategy.Cadence,
			"pageBudget":    strategy.PageBudget,
			"cadenceSource": strategy.CadenceSource,
		})
	}
	logger.New("Lifecycle").Info("lifecycle session touched",
		logger.F("profile_id", profileID),
		logger.F("cadence", strategy.Cadence),
	)
	// AH2: activate daily cycle on first touch of the UTC day.
	a.runDailyLifecycleCycle(profileID)
}

func (a *App) runDailyLifecycleCycle(profileID string) {
	if a == nil || a.lifecycleStore == nil {
		return
	}
	state, err := a.lifecycleStore.Load(profileID)
	if err != nil {
		return
	}
	engine := a.lifecycleEngine()
	result := engine.RunDailyCycle(lifecycle.CycleRequest{State: state})
	_ = a.lifecycleStore.Save(result.State)
	if a.ctx != nil {
		events.EmitAndLog(a.ctx, "behavior:cadence:cycle-completed", map[string]interface{}{
			"profileId": profileID,
			"riskScore": result.State.RiskScore,
		})
	}
}

func (a *App) lifecycleEngine() lifecycle.Engine {
	engine := lifecycle.NewEngine()
	root := a.resolveAppRoot()
	if cfg, err := platformpack.LoadCadence(root, "xhs"); err == nil {
		engine = engine.WithCadence(cfg)
	}
	return engine
}

func (a *App) resolveAppRoot() string {
	if a != nil && strings.TrimSpace(a.appRoot) != "" {
		return filepath.Clean(a.appRoot)
	}
	if wd, err := os.Getwd(); err == nil {
		return wd
	}
	return "."
}
