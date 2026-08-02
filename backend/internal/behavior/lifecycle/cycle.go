package lifecycle

import "time"

type CycleRequest struct {
	State        State
	PageSequence []PageType
	Topic        string
	TopicWeight  float64
	ExitRegion   string
	LocalHour    string
	Now          time.Time
}

type CycleResult struct {
	State       State
	Strategy    SessionStrategy
	PageModels  []ConsumptionModel
	RiskScore   float64
	CompletedAt time.Time
}

func (e Engine) RunDailyCycle(request CycleRequest) CycleResult {
	if !request.Now.IsZero() {
		e.Now = func() time.Time { return request.Now }
	}
	state := request.State
	if state.CreatedAt.IsZero() {
		state.CreatedAt = e.now()
	}
	strategy := e.PlanDailySession(state)
	models := make([]ConsumptionModel, 0, len(request.PageSequence))
	for _, pageType := range request.PageSequence {
		models = append(models, ModelForPageType(pageType))
	}
	if request.Topic != "" {
		state = e.EvolveInterest(state, request.Topic, request.TopicWeight)
	}
	state = e.AdvanceSocialStage(state, len(models) > 0)
	coherence := e.AssessGeoCoherence(state, request.ExitRegion, request.LocalHour)
	state.RiskScore = AssessRisk(state, coherence)
	state.SessionCount++
	state.LastActiveAt = e.now()
	return CycleResult{State: state, Strategy: strategy, PageModels: models, RiskScore: state.RiskScore, CompletedAt: e.now()}
}
