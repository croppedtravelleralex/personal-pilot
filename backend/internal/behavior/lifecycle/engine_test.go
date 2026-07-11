package lifecycle

import (
	"testing"
	"time"

	"personal-pilot/backend/internal/platformpack"
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

func TestPlanDailySessionAppliesCadenceBudget(t *testing.T) {
	now := time.Date(2026, 5, 28, 12, 0, 0, 0, time.UTC)
	engine := Engine{Now: func() time.Time { return now }}.WithCadence(platformpack.CadenceConfig{
		Nurture: platformpack.CadenceNurture{
			SessionMinutes:       []int{8, 15},
			ScrollPauseMs:        []int{1200, 2800},
			MaxActionsPerSession: 12,
		},
		Source: "test://cadence.yaml",
	})
	steady := engine.PlanDailySession(State{ProfileID: "p1", CreatedAt: now.Add(-48 * time.Hour)})
	if steady.PageBudget != 8 {
		t.Fatalf("steady budget=%d want 8 (yaml max 12, base 8)", steady.PageBudget)
	}
	if steady.MinDuration != 8*time.Minute || steady.MaxDuration != 15*time.Minute {
		t.Fatalf("steady durations %+v", steady)
	}
	newAcct := engine.PlanDailySession(State{ProfileID: "p2", CreatedAt: now})
	if newAcct.PageBudget != 3 {
		t.Fatalf("new_account budget=%d want 3", newAcct.PageBudget)
	}
	risk := engine.PlanDailySession(State{ProfileID: "p3", CreatedAt: now.Add(-48 * time.Hour), RiskScore: 0.9})
	if risk.PageBudget != 2 || risk.Cadence != "risk_reduced" {
		t.Fatalf("risk strategy=%+v", risk)
	}
}
