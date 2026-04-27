package humanize

import "testing"

func TestNewBehavioralMutationMiddleware(t *testing.T) {
	cfg := DefaultConfig()
	mw := NewBehavioralMutationMiddleware(cfg)
	if mw.Config().Level != LevelMedium {
		t.Fatalf("Level = %v, want LevelMedium", mw.Config().Level)
	}
}

func TestMiddleware_Mutate_Goto(t *testing.T) {
	mw := NewBehavioralMutationMiddleware(DefaultConfig())
	mw.config.Failure.GiveUpChance = 0
	action := LlmAction{Type: ActionGoto, URL: "https://example.com"}
	mutated := mw.Mutate(action, nil)

	if mutated.Type != MutatedGoto {
		t.Fatalf("Type = %v, want MutatedGoto", mutated.Type)
	}
	if mutated.URL != "https://example.com" {
		t.Fatalf("URL = %q", mutated.URL)
	}
	if mutated.PreGapMs == 0 {
		t.Fatal("PreGapMs should be > 0 for Medium level")
	}
}

func TestMiddleware_Mutate_TypeText(t *testing.T) {
	mw := NewBehavioralMutationMiddleware(DefaultConfig())
	mw.config.Failure.GiveUpChance = 0
	action := LlmAction{Type: ActionTypeText, Selector: "#input", Text: "hello"}
	mutated := mw.Mutate(action, nil)

	if mutated.Type != MutatedTypeText {
		t.Fatalf("Type = %v, want MutatedTypeText", mutated.Type)
	}
	if mutated.TypingPlan == nil {
		t.Fatal("TypingPlan should not be nil")
	}
	if len(mutated.TypingPlan.Events) == 0 {
		t.Fatal("TypingPlan.Events should not be empty")
	}
	if mutated.PreGapMs == 0 {
		t.Fatal("PreGapMs should be > 0")
	}
}

func TestMiddleware_Mutate_Click(t *testing.T) {
	mw := NewBehavioralMutationMiddleware(DefaultConfig())
	mw.config.Failure.GiveUpChance = 0
	bounds := &ElementBounds{X1: 100, Y1: 100, X2: 200, Y2: 160, Width: 100, Height: 60}
	action := LlmAction{Type: ActionClick, Selector: "#btn"}
	mutated := mw.Mutate(action, bounds)

	if mutated.Type != MutatedClick {
		t.Fatalf("Type = %v, want MutatedClick", mutated.Type)
	}
	if mutated.ClickTarget == nil {
		t.Fatal("ClickTarget should not be nil")
	}
}

func TestMiddleware_Mutate_Click_NoElement(t *testing.T) {
	mw := NewBehavioralMutationMiddleware(DefaultConfig())
	mw.config.Failure.GiveUpChance = 0
	action := LlmAction{Type: ActionClick, Selector: "#btn"}
	mutated := mw.Mutate(action, nil)

	if mutated.ClickTarget == nil {
		t.Fatal("ClickTarget should not be nil even without element info")
	}
}

func TestMiddleware_Mutate_Wait(t *testing.T) {
	mw := NewBehavioralMutationMiddleware(DefaultConfig())
	mw.config.Failure.GiveUpChance = 0
	action := LlmAction{Type: ActionWait, DurationMs: 1000}
	mutated := mw.Mutate(action, nil)

	if mutated.Type != MutatedWait {
		t.Fatalf("Type = %v, want MutatedWait", mutated.Type)
	}
	// Jitter should be 0-20% of duration
	if mutated.JitterMs > 200 {
		t.Fatalf("JitterMs = %d, want <= 200 (20%% of 1000)", mutated.JitterMs)
	}
}

func TestMiddleware_Mutate_Scroll(t *testing.T) {
	mw := NewBehavioralMutationMiddleware(DefaultConfig())
	mw.config.Failure.GiveUpChance = 0
	dist := uint32(400)
	action := LlmAction{Type: ActionScroll, Direction: ScrollDown, DistancePx: &dist}
	mutated := mw.Mutate(action, nil)

	if mutated.Type != MutatedScroll {
		t.Fatalf("Type = %v, want MutatedScroll", mutated.Type)
	}
	if mutated.ScrollPlan == nil {
		t.Fatal("ScrollPlan should not be nil")
	}
	if len(mutated.ScrollPlan.Steps) < 2 {
		t.Fatalf("ScrollPlan.Steps = %d, want >= 2", len(mutated.ScrollPlan.Steps))
	}
}

func TestMiddleware_DecideRetry_Success(t *testing.T) {
	mw := NewBehavioralMutationMiddleware(DefaultConfig())
	mw.config.Failure.GiveUpChance = 0
	result := ActionResultSuccess()
	d := mw.DecideRetry(&result, 0)
	if d.Action.Type != RecoveryRetry {
		t.Fatalf("Success result should default to Retry, got %v", d.Action.Type)
	}
}

func TestMiddleware_DecideRetry_Failure(t *testing.T) {
	mw := NewBehavioralMutationMiddleware(DefaultConfig())
	mw.config.Failure.GiveUpChance = 0
	result := ActionResultFailure(ErrElementNotFound, "not found")
	d := mw.DecideRetry(&result, 0)
	// For ErrElementNotFound at attempt 0, should be RetryAfter (human hesitation)
	if d.Action.Type != RecoveryRetryAfter {
		t.Fatalf("Action.Type = %v, want RecoveryRetryAfter", d.Action.Type)
	}
}

func TestMiddleware_UpdateConfig(t *testing.T) {
	mw := NewBehavioralMutationMiddleware(DefaultConfig())
	mw.config.Failure.GiveUpChance = 0
	oldLevel := mw.Config().Level

	cfg := ConfigForLevel(LevelHigh)
	mw.UpdateConfig(cfg)

	if mw.Config().Level == oldLevel {
		t.Fatal("Config should have changed after UpdateConfig")
	}
	if mw.Config().Level != LevelHigh {
		t.Fatalf("Level = %v, want LevelHigh", mw.Config().Level)
	}
}

func TestElementBounds_Center(t *testing.T) {
	b := ElementBounds{X1: 100, Y1: 50, X2: 200, Y2: 100, Width: 100, Height: 50}
	if cx := b.CenterX(); cx != 150 {
		t.Fatalf("CenterX = %d, want 150", cx)
	}
	if cy := b.CenterY(); cy != 75 {
		t.Fatalf("CenterY = %d, want 75", cy)
	}
}
