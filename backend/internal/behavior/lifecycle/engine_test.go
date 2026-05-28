package lifecycle

import (
	"testing"
	"time"
)

func TestLifecycleEnginePlansAndProgresses(t *testing.T) {
	now := time.Date(2026, 5, 28, 12, 0, 0, 0, time.UTC)
	engine := Engine{Now: func() time.Time { return now }}
	state := State{ProfileID: "p1", CreatedAt: now.Add(-48 * time.Hour), SocialStage: "only-read", ExitRegion: "US", PreferredLocale: "en-US"}
	strategy := engine.PlanDailySession(state)
	if strategy.PageBudget == 0 || strategy.Cadence == "" {
		t.Fatalf("strategy = %+v", strategy)
	}
	state = engine.EvolveInterest(state, "devtools", 0.8)
	if state.InterestVector["devtools"] == 0 {
		t.Fatal("interest did not evolve")
	}
	state = engine.AdvanceSocialStage(state, true)
	if state.SocialStage != "like" {
		t.Fatalf("stage = %q", state.SocialStage)
	}
	if score := engine.AssessGeoCoherence(state, "CN", "night"); score >= 1 {
		t.Fatalf("geo score = %f, want penalty", score)
	}
	if ModelForPageType(PageProduct).Interaction != "inspect" {
		t.Fatal("product model missing")
	}
}
