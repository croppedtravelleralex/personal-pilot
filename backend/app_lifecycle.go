package backend

import (
	"time"

	"personal-pilot/backend/internal/behavior/lifecycle"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/logger"
)

func (a *App) touchProfileLifecycle(profileID string) {
	if a == nil || a.lifecycleStore == nil || profileID == "" {
		return
	}
	state, err := a.lifecycleStore.Load(profileID)
	if err != nil {
		state = lifecycle.State{
			ProfileID:    profileID,
			CreatedAt:    time.Now().UTC(),
			SocialStage:  "only-read",
			InterestVector: map[string]float64{},
		}
	}
	engine := lifecycle.NewEngine()
	strategy := engine.PlanDailySession(state)
	state = engine.EvolveInterest(state, "default", 0.1)
	_ = a.lifecycleStore.Save(state)
	_ = a.lifecycleStore.IncrementSession(profileID, time.Now().UTC())
	if a.ctx != nil {
		events.EmitAndLog(a.ctx, "behavior:cadence:planned", map[string]interface{}{
			"profileId":  profileID,
			"cadence":    strategy.Cadence,
			"pageBudget": strategy.PageBudget,
		})
	}
	logger.New("Lifecycle").Info("lifecycle session touched",
		logger.F("profile_id", profileID),
		logger.F("cadence", strategy.Cadence),
	)
}

func (a *App) runDailyLifecycleCycle(profileID string) {
	if a == nil || a.lifecycleStore == nil {
		return
	}
	state, err := a.lifecycleStore.Load(profileID)
	if err != nil {
		return
	}
	engine := lifecycle.NewEngine()
	result := engine.RunDailyCycle(lifecycle.CycleRequest{State: state})
	_ = a.lifecycleStore.Save(result.State)
	if a.ctx != nil {
		events.EmitAndLog(a.ctx, "behavior:cadence:cycle-completed", map[string]interface{}{
			"profileId": profileID,
			"riskScore": result.State.RiskScore,
		})
	}
}
