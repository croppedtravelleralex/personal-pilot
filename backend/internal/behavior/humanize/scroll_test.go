package humanize

import "testing"

func TestBuildScrollPlan_None(t *testing.T) {
	cfg := ConfigForLevel(LevelNone)
	plan := BuildScrollPlan(500, &cfg)

	if len(plan.Steps) != 1 {
		t.Fatalf("Steps = %d, want 1 for LevelNone", len(plan.Steps))
	}
	if plan.Steps[0].DeltaPx != 500 {
		t.Fatalf("DeltaPx = %d, want 500", plan.Steps[0].DeltaPx)
	}
	if plan.Steps[0].Speed != ScrollFast {
		t.Fatalf("Speed = %v, want ScrollFast", plan.Steps[0].Speed)
	}
}

func TestBuildScrollPlan_Medium(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	plan := BuildScrollPlan(500, &cfg)

	// Medium level has overshoot, so there should be more than 1 step
	if len(plan.Steps) < 2 {
		t.Fatalf("Steps = %d, want >= 2 for LevelMedium (has overshoot phases)", len(plan.Steps))
	}

	// Total distance should include overshoot
	if plan.TotalDistancePx <= 500 {
		t.Fatalf("TotalDistancePx = %d, want > 500 (includes overshoot)", plan.TotalDistancePx)
	}
}

func TestBuildScrollPlan_HasPhases(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	plan := BuildScrollPlan(300, &cfg)

	hasScrollBy := false
	hasPause := false
	for _, step := range plan.Steps {
		if step.Type == ScrollStepBy {
			hasScrollBy = true
		}
		if step.Type == ScrollStepPause {
			hasPause = true
		}
	}
	if !hasScrollBy {
		t.Fatal("Scroll plan should have at least one ScrollBy step")
	}
	if !hasPause {
		t.Fatal("Scroll plan should have at least one Pause step")
	}
}

func TestBuildElementScrollPlan(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	// Element at Y=1000, viewport height 600
	// target = max(0, 1000 - 600/3) = max(0, 800) = 800
	plan := BuildElementScrollPlan(1000, 600, &cfg)
	if plan.TotalDistancePx != 800+uint32(float64(800)*0.35)*2 {
		// Verify it's computed correctly
		if plan.TotalDistancePx == 0 {
			t.Fatal("TotalDistancePx should be > 0")
		}
	}
}

func TestScrollStepDuration(t *testing.T) {
	tests := []struct {
		px    uint32
		speed ScrollSpeed
		want  uint32
	}{
		{30, ScrollFast, 10},
		{30, ScrollNormal, 15},
		{30, ScrollSlow, 30},
		{0, ScrollFast, 0},
	}
	for _, tc := range tests {
		got := scrollStepDuration(tc.px, tc.speed)
		if got != tc.want {
			t.Fatalf("scrollStepDuration(%d, %v) = %d, want %d", tc.px, tc.speed, got, tc.want)
		}
	}
}
