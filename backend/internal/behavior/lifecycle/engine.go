package lifecycle

import "time"

type PageType string

const (
	PageArticle       PageType = "article"
	PageSearchResults PageType = "search_results"
	PageListing       PageType = "listing"
	PageProduct       PageType = "product"
	PageForm          PageType = "form"
	PageAuth          PageType = "auth"
	PageDashboard     PageType = "dashboard"
	PageGeneric       PageType = "generic"
)

type State struct {
	ProfileID       string
	CreatedAt       time.Time
	LastActiveAt    time.Time
	SessionCount    uint32
	InterestVector  map[string]float64
	SocialStage     string
	RiskScore       float64
	ExitRegion      string
	PreferredLocale string
}

type SessionStrategy struct {
	MinDuration time.Duration
	MaxDuration time.Duration
	PageBudget  uint32
	Cadence     string
}

type Engine struct {
	Now func() time.Time
}

func NewEngine() Engine {
	return Engine{Now: time.Now}
}

func (e Engine) PlanDailySession(state State) SessionStrategy {
	now := e.now()
	ageDays := now.Sub(state.CreatedAt).Hours() / 24
	if ageDays < 1 {
		return SessionStrategy{MinDuration: 90 * time.Second, MaxDuration: 4 * time.Minute, PageBudget: 3, Cadence: "new_account"}
	}
	if state.RiskScore >= 0.7 {
		return SessionStrategy{MinDuration: 45 * time.Second, MaxDuration: 2 * time.Minute, PageBudget: 2, Cadence: "risk_reduced"}
	}
	return SessionStrategy{MinDuration: 3 * time.Minute, MaxDuration: 12 * time.Minute, PageBudget: 8, Cadence: "steady"}
}

func (e Engine) EvolveInterest(state State, topic string, weight float64) State {
	if state.InterestVector == nil {
		state.InterestVector = map[string]float64{}
	}
	if weight < 0 {
		weight = 0
	}
	state.InterestVector[topic] = state.InterestVector[topic]*0.85 + weight
	state.LastActiveAt = e.now()
	return state
}

func (e Engine) AdvanceSocialStage(state State, successfulInteraction bool) State {
	if !successfulInteraction {
		return state
	}
	switch state.SocialStage {
	case "", "only-read":
		state.SocialStage = "like"
	case "like":
		state.SocialStage = "follow"
	case "follow":
		state.SocialStage = "comment"
	}
	state.LastActiveAt = e.now()
	return state
}

func (e Engine) AssessGeoCoherence(state State, exitRegion, localHour string) float64 {
	score := 1.0
	if state.ExitRegion != "" && exitRegion != "" && state.ExitRegion != exitRegion {
		score -= 0.35
	}
	if state.PreferredLocale != "" && exitRegion != "" && len(state.PreferredLocale) >= 2 && len(exitRegion) >= 2 && state.PreferredLocale[len(state.PreferredLocale)-2:] != exitRegion {
		score -= 0.15
	}
	if localHour == "night" {
		score -= 0.1
	}
	if score < 0 {
		return 0
	}
	return score
}

func (e Engine) now() time.Time {
	if e.Now != nil {
		return e.Now()
	}
	return time.Now()
}
